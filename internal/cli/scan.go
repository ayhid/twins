package cli

import (
	"context"
	"fmt"
	"os"

	"github.com/charmbracelet/x/term"
	"github.com/spf13/cobra"

	"github.com/ayhid/twins/internal/group"
	"github.com/ayhid/twins/internal/report"
)

func (a *app) scanCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "scan [path...]",
		Short: "Find duplicates and browse them (default: your home folder)",
		Args:  cobra.ArbitraryArgs,
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := a.newSession(args)
			if err != nil {
				return err
			}
			if a.interactive() {
				return a.runBrowser(cmd.Context(), s, browseModeScan)
			}
			out, err := a.scanPlain(cmd.Context(), s)
			if err != nil {
				return err
			}
			return a.printReport(out, s, false)
		},
	}
}

func (a *app) reportCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "report [path...]",
		Short: "Scan and print a JSON report (never modifies anything)",
		Args:  cobra.ArbitraryArgs,
		RunE: func(cmd *cobra.Command, args []string) error {
			a.flags.json = true
			s, err := a.newSession(args)
			if err != nil {
				return err
			}
			out, err := a.scanPlain(cmd.Context(), s)
			if err != nil {
				return err
			}
			return a.printReport(out, s, false)
		},
	}
}

// interactive reports whether the browser UI should be used.
func (a *app) interactive() bool {
	if a.flags.json || a.flags.plain {
		return false
	}
	f, ok := a.stdout.(*os.File)
	return ok && term.IsTerminal(f.Fd())
}

// scanPlain runs a scan with textual progress on stderr.
func (a *app) scanPlain(ctx context.Context, s session) (Outcome, error) {
	p := newPlainProgress(a.stderr, a.flags.json || !a.isTerminal(a.stderr))
	out, err := s.run(ctx, p.update, a.errorReporter())
	p.finish()
	return out, err
}

func (a *app) isTerminal(w any) bool {
	f, ok := w.(*os.File)
	return ok && term.IsTerminal(f.Fd())
}

func (a *app) errorReporter() func(string, error) {
	if !a.flags.verbose {
		return nil
	}
	return func(path string, err error) {
		fmt.Fprintf(a.stderr, "skip %s: %v\n", path, err)
	}
}

func (a *app) printReport(out Outcome, s session, dryRun bool) error {
	plan := group.Plan(out.Groups, s.keeper)
	r := report.Build(plan, report.Meta{
		Roots:      out.Roots,
		Files:      out.Stats.Files,
		Candidates: out.Stats.Candidates,
		Strategy:   string(s.keeper.Strategy),
		DryRun:     dryRun,
	})
	if a.flags.json {
		return report.WriteJSON(a.stdout, r)
	}
	report.WriteText(a.stdout, r)
	if out.Stats.Errors > 0 && !a.flags.verbose {
		fmt.Fprintf(a.stderr, "%d files could not be read (use --verbose to list them)\n", out.Stats.Errors)
	}
	return nil
}
