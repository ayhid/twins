package cli

import (
	"fmt"
	"io"
	"time"
)

// plainProgress prints coarse progress lines to stderr for non-TTY runs.
// It is quiet when the output is JSON or piped.
type plainProgress struct {
	w       io.Writer
	quiet   bool
	last    time.Time
	phase   string
	started time.Time
}

func newPlainProgress(w io.Writer, quiet bool) *plainProgress {
	return &plainProgress{w: w, quiet: quiet, started: time.Now()}
}

func (p *plainProgress) update(pr Progress) {
	if p.quiet {
		return
	}
	now := time.Now()
	if pr.Phase == p.phase && now.Sub(p.last) < 500*time.Millisecond {
		return
	}
	p.phase, p.last = pr.Phase, now
	if pr.Total > 0 {
		fmt.Fprintf(p.w, "\r%-14s %d/%d", pr.Phase, pr.Done, pr.Total)
		return
	}
	fmt.Fprintf(p.w, "\r%-14s %d files", pr.Phase, pr.Done)
}

func (p *plainProgress) finish() {
	if p.quiet || p.phase == "" {
		return
	}
	fmt.Fprintf(p.w, "\rdone in %s%20s\n", time.Since(p.started).Round(time.Millisecond), "")
}
