package group

import (
	"errors"
	"fmt"
	"path/filepath"
	"sort"
	"strings"

	"github.com/ayhid/twins/internal/fsutil"
)

// ErrHashCollision is reported when two files share a digest but differ.
var ErrHashCollision = errors.New("content differs despite identical hash")

// Strategy decides which copy of a group survives.
type Strategy string

// Available strategies.
const (
	KeepOldest       Strategy = "oldest"
	KeepNewest       Strategy = "newest"
	KeepShortestPath Strategy = "shortest-path"
	KeepInDir        Strategy = "in-dir"
)

// Strategies lists every valid strategy name.
var Strategies = []Strategy{KeepOldest, KeepNewest, KeepShortestPath, KeepInDir}

// ParseStrategy validates a user-provided strategy name.
func ParseStrategy(s string) (Strategy, error) {
	for _, known := range Strategies {
		if Strategy(s) == known {
			return known, nil
		}
	}
	return "", fmt.Errorf("unknown keep strategy %q (want one of %v)", s, Strategies)
}

// Keeper applies a Strategy. Dir is only used by KeepInDir.
type Keeper struct {
	Strategy Strategy
	Dir      string
}

// Choose returns the file to keep and the files to remove. Hardlinks of the
// kept file are never removed: they already share its data.
func (k Keeper) Choose(g Group) (fsutil.FileMeta, []fsutil.FileMeta) {
	keep := k.pick(g.Files)
	var remove []fsutil.FileMeta
	for _, f := range g.Files {
		if f.Identity() != keep.Identity() {
			remove = append(remove, f)
		}
	}
	return keep, remove
}

// Pick returns the file to keep among files (which must be non-empty).
func (k Keeper) Pick(files []fsutil.FileMeta) fsutil.FileMeta { return k.pick(files) }

func (k Keeper) pick(files []fsutil.FileMeta) fsutil.FileMeta {
	sorted := append([]fsutil.FileMeta(nil), files...)
	sort.SliceStable(sorted, func(a, b int) bool { return k.less(sorted[a], sorted[b]) })
	return sorted[0]
}

// less is a total order whose minimum is the file to keep.
func (k Keeper) less(a, b fsutil.FileMeta) bool {
	switch k.Strategy {
	case KeepNewest:
		if !a.ModTime.Equal(b.ModTime) {
			return a.ModTime.After(b.ModTime)
		}
	case KeepShortestPath:
		if da, db := depth(a.Path), depth(b.Path); da != db {
			return da < db
		}
		if len(a.Path) != len(b.Path) {
			return len(a.Path) < len(b.Path)
		}
	case KeepInDir:
		if ia, ib := k.inDir(a.Path), k.inDir(b.Path); ia != ib {
			return ia
		}
		fallthrough
	default: // KeepOldest
		if !a.ModTime.Equal(b.ModTime) {
			return a.ModTime.Before(b.ModTime)
		}
	}
	if da, db := depth(a.Path), depth(b.Path); da != db {
		return da < db
	}
	return a.Path < b.Path
}

func (k Keeper) inDir(path string) bool {
	dir := filepath.Clean(k.Dir)
	return path == dir || strings.HasPrefix(path, dir+string(filepath.Separator))
}

func depth(path string) int {
	return strings.Count(filepath.Clean(path), string(filepath.Separator))
}

// Action is the resolved decision for one group.
type Action struct {
	Group  Group
	Keep   fsutil.FileMeta
	Remove []fsutil.FileMeta
}

// Reclaimable is the space actually freed by executing the action:
// one Size per physical identity removed that is not the kept file's.
func (a Action) Reclaimable() int64 {
	seen := map[fsutil.Identity]struct{}{a.Keep.Identity(): {}}
	var n int64
	for _, f := range a.Remove {
		if _, ok := seen[f.Identity()]; ok {
			continue
		}
		seen[f.Identity()] = struct{}{}
		n += a.Group.Size
	}
	return n
}

// Plan applies a Keeper to every group.
func Plan(groups []Group, k Keeper) []Action {
	actions := make([]Action, 0, len(groups))
	for _, g := range groups {
		keep, remove := k.Choose(g)
		actions = append(actions, Action{Group: g, Keep: keep, Remove: remove})
	}
	return actions
}

// TotalReclaimable sums the space freed by a plan.
func TotalReclaimable(actions []Action) int64 {
	var n int64
	for _, a := range actions {
		n += a.Reclaimable()
	}
	return n
}
