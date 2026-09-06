package tui

import (
	"context"
	"os/exec"

	"github.com/ayhid/twins/internal/group"
	"github.com/ayhid/twins/internal/trash"
)

// Mode selects what the browser allows.
type Mode int

// Modes.
const (
	ModeScan  Mode = iota // browse only
	ModeClean             // browse and remove
)

// Progress is a scan progress event.
type Progress struct {
	Phase string
	Done  int64
	Total int64
}

// ScanResult is what a scan produces.
type ScanResult struct {
	Files  int64
	Errors int64
	Groups []group.Group
}

// ScanFunc runs a scan over roots, reporting progress (possibly from
// several goroutines).
type ScanFunc func(ctx context.Context, roots []string, progress func(Progress)) (ScanResult, error)

// ExecFunc executes a plan, reporting progress.
type ExecFunc func(ctx context.Context, plan []group.Action, progress func(done, total int)) (trash.Result, error)

// Deps is everything the UI needs from the outside world.
type Deps struct {
	Mode        Mode
	Roots       []string
	StartAtMenu bool
	Scan        ScanFunc
	Execute     ExecFunc
	Keeper      group.Keeper
	DeleteMode  string // config.ModeTrash, ModePermanent or ModeLink
	DryRun      bool
	Home        string                  // used to shorten paths as ~
	Reveal      func(path string) error // optional: show the file in Finder
	Preview     func(path string) error // optional: Quick Look the file
}

// Summary is returned when the UI exits.
type Summary struct {
	Executed bool
	Result   trash.Result
}

func (d Deps) withDefaults() Deps {
	out := d
	if out.Reveal == nil {
		out.Reveal = func(path string) error { return exec.Command("open", "-R", path).Start() }
	}
	if out.Preview == nil {
		out.Preview = func(path string) error { return exec.Command("qlmanage", "-p", path).Start() }
	}
	return out
}
