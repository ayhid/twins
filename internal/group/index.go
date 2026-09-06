// Package group turns a stream of candidate files into groups of exact
// duplicates through a size → partial hash → full hash pipeline.
package group

import (
	"sync"

	"github.com/ayhid/twins/internal/fsutil"
)

// Index collects candidate files by size. It is the only mutable, shared
// structure in the pipeline and is safe for concurrent Add calls.
type Index struct {
	mu     sync.Mutex
	bySize map[int64][]fsutil.FileMeta
	count  int64
}

// NewIndex returns an empty index.
func NewIndex() *Index {
	return &Index{bySize: make(map[int64][]fsutil.FileMeta)}
}

// Add records a candidate. It satisfies scan.Visit.
func (i *Index) Add(m fsutil.FileMeta) {
	i.mu.Lock()
	defer i.mu.Unlock()
	i.bySize[m.Size] = append(i.bySize[m.Size], m)
	i.count++
}

// Len returns the number of files added.
func (i *Index) Len() int64 {
	i.mu.Lock()
	defer i.mu.Unlock()
	return i.count
}

// Candidates returns copies of every size bucket holding at least two
// distinct physical files. Size buckets with a single file are dropped
// here, which typically discards the vast majority of the index.
func (i *Index) Candidates() [][]fsutil.FileMeta {
	i.mu.Lock()
	defer i.mu.Unlock()
	var out [][]fsutil.FileMeta
	for _, files := range i.bySize {
		if physical(files) < 2 {
			continue
		}
		out = append(out, append([]fsutil.FileMeta(nil), files...))
	}
	return out
}

// physical counts distinct (device, inode) identities.
func physical(files []fsutil.FileMeta) int {
	seen := make(map[fsutil.Identity]struct{}, len(files))
	for _, f := range files {
		seen[f.Identity()] = struct{}{}
	}
	return len(seen)
}
