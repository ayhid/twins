package cli

import (
	"context"
	"errors"
	"os"

	"github.com/ayhid/twins/internal/group"
	"github.com/ayhid/twins/internal/trash"
	"github.com/ayhid/twins/internal/tui"
)

var errNotATerminal = errors.New("the interactive menu needs a terminal; try `twins scan --plain` or `twins report`")

// browseMode tells the interactive UI whether removal is allowed.
type browseMode int

const (
	browseModeScan browseMode = iota
	browseModeClean
)

func (a *app) runMenu(ctx context.Context) error {
	if !a.interactive() {
		return usageError{errNotATerminal}
	}
	s, err := a.newSession(nil)
	if err != nil {
		return err
	}
	deps := a.tuiDeps(s, browseModeScan)
	deps.StartAtMenu = true
	return a.runTUI(ctx, deps)
}

func (a *app) runBrowser(ctx context.Context, s session, mode browseMode) error {
	return a.runTUI(ctx, a.tuiDeps(s, mode))
}

func (a *app) runTUI(ctx context.Context, deps tui.Deps) error {
	summary, err := tui.Run(ctx, deps)
	if err != nil {
		return err
	}
	if summary.Executed {
		a.printResult(summary.Result, deps.DeleteMode)
	}
	return nil
}

// tuiDeps adapts the session to the UI's callbacks.
func (a *app) tuiDeps(s session, mode browseMode) tui.Deps {
	home, _ := os.UserHomeDir()
	return tui.Deps{
		Mode:       tui.Mode(mode),
		Roots:      s.roots,
		Keeper:     s.keeper,
		DeleteMode: s.cfg.DeleteMode,
		DryRun:     a.flags.dryRun,
		Home:       home,
		Scan: func(ctx context.Context, roots []string, progress func(tui.Progress)) (tui.ScanResult, error) {
			run := s
			run.roots = roots
			run.scan.Roots = roots
			out, err := run.run(ctx, func(p Progress) {
				progress(tui.Progress{Phase: p.Phase, Done: p.Done, Total: p.Total})
			}, nil)
			if err != nil {
				return tui.ScanResult{}, err
			}
			return tui.ScanResult{Files: out.Stats.Files, Errors: out.Stats.Errors, Groups: out.Groups}, nil
		},
		Execute: func(ctx context.Context, plan []group.Action, progress func(int, int)) (trash.Result, error) {
			return a.execute(ctx, plan, s.cfg.DeleteMode, progress)
		},
	}
}
