package trash

import (
	"context"
	"fmt"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
	"github.com/ayhid/twins/internal/journal"
	"github.com/ayhid/twins/internal/safety"
)

// Backend names the Trash implementation compiled in.
func Backend() string { return trashBackend }

// Options controls Execute.
type Options struct {
	Remover    Remover
	Journal    *journal.Journal // nil means Discard
	DryRun     bool
	OnProgress func(done, total int) // optional
}

// Failure records one file that could not be removed.
type Failure struct {
	Path string
	Err  error
}

// Result summarises an execution.
type Result struct {
	Removed  int
	Bytes    int64
	Failures []Failure
	DryRun   bool
}

// Execute validates the plan, then disposes of every duplicate in order.
// A failure on one file is recorded and does not stop the others. In
// dry-run mode nothing is touched but every intended action is journaled.
func Execute(ctx context.Context, actions []group.Action, opts Options) (Result, error) {
	if err := safety.Validate(actions); err != nil {
		return Result{}, fmt.Errorf("refusing to execute unsafe plan: %w", err)
	}
	if opts.Remover == nil && !opts.DryRun {
		return Result{}, fmt.Errorf("no remover configured")
	}
	j := opts.Journal
	if j == nil {
		j = journal.Discard()
	}
	total := countRemovals(actions)
	res := Result{DryRun: opts.DryRun}
	done := 0
	for _, a := range actions {
		for _, dup := range a.Remove {
			if err := ctx.Err(); err != nil {
				return res, err
			}
			res = res.with(execOne(a, dup, opts, j))
			done++
			if opts.OnProgress != nil {
				opts.OnProgress(done, total)
			}
		}
	}
	return res, nil
}

type outcome struct {
	bytes   int64
	failure *Failure
}

func execOne(a group.Action, dup fsutil.FileMeta, opts Options, j *journal.Journal) outcome {
	entry := journal.Entry{
		Path:   dup.Path,
		Size:   a.Group.Size,
		Digest: a.Group.Digest.String(),
		Keep:   a.Keep.Path,
	}
	if opts.DryRun {
		entry.Action = journal.ActionDryRun
		_ = j.Write(entry)
		return outcome{bytes: a.Group.Size}
	}
	entry.Action = opts.Remover.Action()
	if err := opts.Remover.Remove(dup, a.Keep); err != nil {
		entry.Error = err.Error()
		_ = j.Write(entry)
		return outcome{failure: &Failure{Path: dup.Path, Err: err}}
	}
	_ = j.Write(entry)
	return outcome{bytes: a.Group.Size}
}

func (r Result) with(o outcome) Result {
	if o.failure != nil {
		return Result{Removed: r.Removed, Bytes: r.Bytes, DryRun: r.DryRun,
			Failures: append(append([]Failure(nil), r.Failures...), *o.failure)}
	}
	return Result{Removed: r.Removed + 1, Bytes: r.Bytes + o.bytes, Failures: r.Failures, DryRun: r.DryRun}
}

func countRemovals(actions []group.Action) int {
	n := 0
	for _, a := range actions {
		n += len(a.Remove)
	}
	return n
}
