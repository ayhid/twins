package cli

import (
	"context"
	"fmt"
	"os"
	"sync/atomic"

	"github.com/ayhid/twins/internal/cache"
	"github.com/ayhid/twins/internal/config"
	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
	"github.com/ayhid/twins/internal/scan"
)

// session is the fully-resolved configuration for one run: config file
// values overridden by flags, roots defaulted to $HOME.
type session struct {
	cfg      config.Config
	roots    []string
	minSize  int64
	keeper   group.Keeper
	scan     scan.Options
	find     group.Options
	useCache bool
	verify   bool
}

// Outcome is what a completed scan produces.
type Outcome struct {
	Roots  []string
	Stats  scan.Stats
	Groups []group.Group
	Cache  *cache.Stats
}

// Progress events emitted during a scan.
type Progress struct {
	Phase string // "walk", "partial hash", "full hash", "verify"
	Done  int64
	Total int64
}

func (a *app) newSession(roots []string) (session, error) {
	cfg, _, err := a.loadConfig()
	if err != nil {
		return session{}, err
	}
	cfg = a.overrideConfig(cfg)
	if err := cfg.Validate(); err != nil {
		return session{}, usageError{err}
	}
	minSize, err := cfg.MinSizeBytes()
	if err != nil {
		return session{}, usageError{err}
	}
	if len(roots) == 0 {
		home, err := os.UserHomeDir()
		if err != nil {
			return session{}, fmt.Errorf("home dir: %w", err)
		}
		roots = []string{home}
	}
	strategy, _ := group.ParseStrategy(cfg.Keep)
	return session{
		cfg:     cfg,
		roots:   roots,
		minSize: minSize,
		keeper:  group.Keeper{Strategy: strategy, Dir: cfg.KeepDir},
		scan: scan.Options{
			Roots:              roots,
			MinSize:            minSize,
			IncludeEmpty:       cfg.IncludeEmpty,
			IncludeLibrary:     cfg.IncludeLibrary,
			IncludeNodeModules: cfg.IncludeNodeModules,
			IncludeRemote:      a.flags.includeRemote,
			Exclude:            cfg.Exclude,
			Workers:            cfg.Jobs,
		},
		find:     group.Options{Workers: cfg.Jobs, Verify: a.flags.verify},
		useCache: !a.flags.noCache,
		verify:   a.flags.verify,
	}, nil
}

// overrideConfig applies explicit flags on top of the file values.
func (a *app) overrideConfig(cfg config.Config) config.Config {
	f := a.flags
	out := cfg
	if f.minSize != "" {
		out.MinSize = f.minSize
	}
	if len(f.exclude) > 0 {
		out.Exclude = append(append([]string(nil), cfg.Exclude...), f.exclude...)
	}
	if f.keep != "" {
		out.Keep = f.keep
	}
	if f.keepDir != "" {
		out.KeepDir = f.keepDir
		if f.keep == "" {
			out.Keep = string(group.KeepInDir)
		}
	}
	if f.jobs > 0 {
		out.Jobs = f.jobs
	}
	out.IncludeLibrary = cfg.IncludeLibrary || f.includeLibrary
	out.IncludeNodeModules = cfg.IncludeNodeModules || f.includeNodeModules
	out.IncludeEmpty = cfg.IncludeEmpty || f.includeEmpty
	return out
}

// run walks the roots then finds duplicates, reporting progress. onError
// receives non-fatal per-file problems.
func (s session) run(ctx context.Context, progress func(Progress), onError func(string, error)) (Outcome, error) {
	idx := group.NewIndex()
	scanOpts := s.scan
	scanOpts.OnError = onError
	var walked atomic.Int64
	stats, err := scan.Walk(ctx, scanOpts, func(m fsutil.FileMeta) {
		idx.Add(m)
		if n := walked.Add(1); progress != nil && n%256 == 0 {
			progress(Progress{Phase: "walk", Done: n})
		}
	})
	if err != nil {
		return Outcome{}, err
	}

	findOpts := s.find
	findOpts.OnError = onError
	if progress != nil {
		findOpts.OnProgress = func(p group.Progress) {
			progress(Progress{Phase: p.Stage.String(), Done: p.Done, Total: p.Total})
		}
	}
	store, err := s.openCache()
	if err != nil {
		return Outcome{}, err
	}
	if store != nil {
		findOpts.Hasher = store
		defer store.Close()
	}
	groups, err := group.Find(ctx, idx, findOpts)
	if err != nil {
		return Outcome{}, err
	}
	out := Outcome{Roots: s.roots, Stats: stats, Groups: groups}
	if store != nil {
		st := store.Stats()
		out.Cache = &st
	}
	return out, nil
}

func (s session) openCache() (*cache.Store, error) {
	if !s.useCache {
		return nil, nil
	}
	path, err := cache.DefaultPath()
	if err != nil {
		return nil, err
	}
	store, err := cache.Open(s.cachePath(path), group.DirectHasher{})
	if err != nil {
		// A locked or corrupt cache must never block a scan: fall back to
		// direct hashing and let the caller surface the warning.
		return nil, nil
	}
	return store, nil
}

// cachePath honours TWINS_CACHE so tests never touch the real cache.
func (s session) cachePath(def string) string {
	if p := os.Getenv("TWINS_CACHE"); p != "" {
		return p
	}
	return def
}
