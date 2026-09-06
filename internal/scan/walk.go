package scan

import (
	"context"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/charlievieth/fastwalk"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/safety"
)

// Walk scans every root in parallel and calls visit for each candidate
// regular file. Symlinks are never followed, bundles are never entered,
// iCloud placeholders are never read and non-local volumes are skipped.
func Walk(ctx context.Context, opts Options, visit Visit) (Stats, error) {
	roots, err := normaliseRoots(opts)
	if err != nil {
		return Stats{}, err
	}
	w := &walker{opts: opts, rules: compileRules(opts), visit: visit, ctx: ctx}
	conf := &fastwalk.Config{Follow: false, NumWorkers: opts.Workers}
	for _, root := range roots {
		if err := fastwalk.Walk(conf, root, w.walkFn(root)); err != nil {
			return w.counters.snapshot(), err
		}
	}
	return w.counters.snapshot(), ctx.Err()
}

type walker struct {
	opts     Options
	rules    rules
	visit    Visit
	ctx      context.Context
	counters counters
}

func (w *walker) walkFn(root string) fs.WalkDirFunc {
	return func(path string, d fs.DirEntry, err error) error {
		if ctxErr := w.ctx.Err(); ctxErr != nil {
			return ctxErr
		}
		if err != nil {
			return w.nonFatal(path, err, d)
		}
		name := d.Name()
		switch {
		case d.IsDir():
			return w.enterDir(root, path, name)
		case d.Type().IsRegular():
			w.handleFile(path, name)
		default:
			w.counters.skipped.Add(1) // symlink, socket, device...
		}
		return nil
	}
}

func (w *walker) enterDir(root, path, name string) error {
	if path == root {
		w.counters.dirs.Add(1)
		return nil
	}
	if w.rules.skipDir(path, name) {
		w.counters.skipped.Add(1)
		return fs.SkipDir
	}
	if !w.opts.IncludeRemote {
		local, err := fsutil.IsLocalVolume(path)
		if err != nil || !local {
			w.counters.skipped.Add(1)
			return fs.SkipDir
		}
	}
	w.counters.dirs.Add(1)
	return nil
}

func (w *walker) handleFile(path, name string) {
	w.counters.files.Add(1)
	if w.rules.skipFile(path, name) {
		w.counters.skipped.Add(1)
		return
	}
	meta, err := fsutil.Stat(path)
	if err != nil {
		w.report(path, err)
		return
	}
	if !w.isCandidate(meta) {
		w.counters.skipped.Add(1)
		return
	}
	w.counters.candidates.Add(1)
	w.visit(meta)
}

func (w *walker) isCandidate(m fsutil.FileMeta) bool {
	if m.Dataless {
		return false
	}
	if m.Size == 0 {
		return w.opts.IncludeEmpty
	}
	return m.Size >= w.opts.MinSize
}

func (w *walker) nonFatal(path string, err error, d fs.DirEntry) error {
	w.report(path, err)
	if d != nil && d.IsDir() {
		return fs.SkipDir
	}
	return nil
}

func (w *walker) report(path string, err error) {
	w.counters.errors.Add(1)
	if w.opts.OnError != nil {
		w.opts.OnError(path, err)
	}
}

// normaliseRoots makes roots absolute, validates them and drops any root
// nested inside another so files are visited once.
func normaliseRoots(opts Options) ([]string, error) {
	if len(opts.Roots) == 0 {
		return nil, ErrNoRoots
	}
	abs := make([]string, 0, len(opts.Roots))
	for _, r := range opts.Roots {
		a, err := filepath.Abs(r)
		if err != nil {
			return nil, fmt.Errorf("root %q: %w", r, err)
		}
		if err := validateRoot(a, opts.IncludeRemote); err != nil {
			return nil, err
		}
		abs = append(abs, filepath.Clean(a))
	}
	sort.Strings(abs)
	var out []string
	for _, a := range abs {
		if len(out) > 0 && (a == out[len(out)-1] || strings.HasPrefix(a, out[len(out)-1]+"/")) {
			continue
		}
		out = append(out, a)
	}
	return out, nil
}

func validateRoot(abs string, includeRemote bool) error {
	if safety.IsProtected(abs) {
		return fmt.Errorf("%s: %w", abs, ErrProtectedRoot)
	}
	info, err := os.Stat(abs)
	if err != nil {
		return fmt.Errorf("root: %w", err)
	}
	if !info.IsDir() {
		return fmt.Errorf("root %s: %w", abs, errors.New("not a directory"))
	}
	if includeRemote {
		return nil
	}
	local, err := fsutil.IsLocalVolume(abs)
	if err != nil {
		return err
	}
	if !local {
		return fmt.Errorf("%s: %w", abs, ErrRemoteRoot)
	}
	return nil
}
