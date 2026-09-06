package group

import (
	"context"
	"runtime"
	"sync"
	"sync/atomic"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/hash"
)

// Stage identifies a pipeline step for progress reporting.
type Stage int

// Pipeline stages, in order.
const (
	StagePartial Stage = iota + 1
	StageFull
	StageVerify
)

// String names the stage for display.
func (s Stage) String() string {
	switch s {
	case StagePartial:
		return "partial hash"
	case StageFull:
		return "full hash"
	case StageVerify:
		return "verify"
	}
	return "unknown"
}

// Progress is a snapshot of one stage's advancement.
type Progress struct {
	Stage Stage
	Done  int64
	Total int64
}

// Options tunes Find. The zero value uses DirectHasher and all CPUs.
// OnProgress is called concurrently from worker goroutines; OnError is
// only called from the calling goroutine.
type Options struct {
	Hasher     Hasher
	Workers    int
	Verify     bool // compare byte by byte after hashing
	OnProgress func(Progress)
	OnError    func(path string, err error)
}

// maxWorkers bounds hashing parallelism: beyond this, SSDs saturate.
const maxWorkers = 8

func (o Options) hasher() Hasher {
	if o.Hasher == nil {
		return DirectHasher{}
	}
	return o.Hasher
}

func (o Options) workers() int {
	if o.Workers > 0 {
		return o.Workers
	}
	return min(runtime.NumCPU(), maxWorkers)
}

func (o Options) report(path string, err error) {
	if o.OnError != nil {
		o.OnError(path, err)
	}
}

// Find runs the pipeline over the index and returns duplicate groups
// sorted by reclaimable space. Files that cannot be read are reported
// through OnError and dropped.
func Find(ctx context.Context, idx *Index, opts Options) ([]Group, error) {
	buckets := idx.Candidates()
	partial, err := refine(ctx, buckets, opts, StagePartial, func(m fsutil.FileMeta) (uint64, error) {
		return opts.hasher().Partial(m)
	})
	if err != nil {
		return nil, err
	}
	full, err := refine(ctx, unkey(partial), opts, StageFull, func(m fsutil.FileMeta) (hash.Digest, error) {
		return opts.hasher().Full(m)
	})
	if err != nil {
		return nil, err
	}
	groups := make([]Group, 0, len(full))
	for _, b := range full {
		files := b.files
		if opts.Verify {
			files = verify(files, opts)
			if physical(files) < 2 {
				continue
			}
		}
		groups = append(groups, newGroup(files[0].Size, b.key, files))
	}
	return sortGroups(groups), ctx.Err()
}

// bucket is a set of files sharing the same key at a given stage.
type bucket[K comparable] struct {
	key   K
	files []fsutil.FileMeta
}

func unkey[K comparable](buckets []bucket[K]) [][]fsutil.FileMeta {
	out := make([][]fsutil.FileMeta, 0, len(buckets))
	for _, b := range buckets {
		out = append(out, b.files)
	}
	return out
}

// refine splits every bucket by a key computed once per physical identity
// and keeps only sub-buckets that still hold two or more physical files.
func refine[K comparable](
	ctx context.Context,
	buckets [][]fsutil.FileMeta,
	opts Options,
	stage Stage,
	key func(fsutil.FileMeta) (K, error),
) ([]bucket[K], error) {
	reps := representatives(buckets)
	keys, err := computeKeys(ctx, reps, opts, stage, key)
	if err != nil {
		return nil, err
	}
	var out []bucket[K]
	for _, files := range buckets {
		byKey := make(map[K][]fsutil.FileMeta)
		var order []K
		for _, f := range files {
			k, ok := keys[f.Identity()]
			if !ok {
				continue // hashing failed, already reported
			}
			if _, seen := byKey[k]; !seen {
				order = append(order, k)
			}
			byKey[k] = append(byKey[k], f)
		}
		for _, k := range order {
			if sub := byKey[k]; physical(sub) >= 2 {
				out = append(out, bucket[K]{key: k, files: sub})
			}
		}
	}
	return out, nil
}

// representatives picks one file per physical identity across all buckets.
func representatives(buckets [][]fsutil.FileMeta) []fsutil.FileMeta {
	seen := make(map[fsutil.Identity]struct{})
	var reps []fsutil.FileMeta
	for _, bucket := range buckets {
		for _, f := range bucket {
			if _, ok := seen[f.Identity()]; ok {
				continue
			}
			seen[f.Identity()] = struct{}{}
			reps = append(reps, f)
		}
	}
	return reps
}

// computeKeys hashes representatives in parallel with bounded workers.
func computeKeys[K comparable](
	ctx context.Context,
	reps []fsutil.FileMeta,
	opts Options,
	stage Stage,
	key func(fsutil.FileMeta) (K, error),
) (map[fsutil.Identity]K, error) {
	type result struct {
		id  fsutil.Identity
		key K
		err error
	}
	results := make([]result, len(reps))
	jobs := make(chan int)
	var done atomic.Int64
	var wg sync.WaitGroup
	total := int64(len(reps))

	for w := 0; w < opts.workers(); w++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for i := range jobs {
				k, err := key(reps[i])
				results[i] = result{id: reps[i].Identity(), key: k, err: err}
				if opts.OnProgress != nil {
					opts.OnProgress(Progress{Stage: stage, Done: done.Add(1), Total: total})
				}
			}
		}()
	}
	var ctxErr error
feed:
	for i := range reps {
		select {
		case <-ctx.Done():
			ctxErr = ctx.Err()
			break feed
		case jobs <- i:
		}
	}
	close(jobs)
	wg.Wait()
	if ctxErr != nil {
		return nil, ctxErr
	}

	keys := make(map[fsutil.Identity]K, len(reps))
	for i, r := range results {
		if r.err != nil {
			opts.report(reps[i].Path, r.err)
			continue
		}
		keys[r.id] = r.key
	}
	return keys, nil
}

// verify keeps only files byte-identical to the first one in the bucket.
func verify(files []fsutil.FileMeta, opts Options) []fsutil.FileMeta {
	ref := files[0]
	out := []fsutil.FileMeta{ref}
	for _, f := range files[1:] {
		if f.Identity() == ref.Identity() {
			out = append(out, f)
			continue
		}
		same, err := hash.Equal(ref.Path, f.Path)
		if err != nil {
			opts.report(f.Path, err)
			continue
		}
		if !same {
			opts.report(f.Path, ErrHashCollision)
			continue
		}
		out = append(out, f)
	}
	return out
}
