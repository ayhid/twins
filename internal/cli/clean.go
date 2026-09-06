package cli

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ayhid/twins/internal/config"
	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
	"github.com/ayhid/twins/internal/journal"
	"github.com/ayhid/twins/internal/trash"
)

type cleanFlags struct {
	yes       bool
	permanent bool
	link      bool
	force     bool
}

func (a *app) cleanCommand() *cobra.Command {
	var cf cleanFlags
	cmd := &cobra.Command{
		Use:   "clean [path...]",
		Short: "Find duplicates and remove them (Trash by default)",
		Long: `Scan for duplicates, keep one copy per group according to --keep, and
dispose of the others. Files go to the Trash unless --permanent or --link
is given. Nothing happens without confirmation unless --yes is set.

--permanent always asks you to type "permanent" to confirm, even with
--yes; pass --force to skip that prompt in scripts.`,
		Args: cobra.ArbitraryArgs,
		RunE: func(cmd *cobra.Command, args []string) error {
			if cf.permanent && cf.link {
				return a.usage(cmd, errors.New("--permanent and --link are mutually exclusive"))
			}
			s, err := a.newSession(args)
			if err != nil {
				return err
			}
			s.cfg.DeleteMode = deleteMode(s.cfg.DeleteMode, cf)
			if a.interactive() && !cf.yes {
				return a.runBrowser(cmd.Context(), s, browseModeClean)
			}
			return a.cleanPlain(cmd.Context(), s, cf)
		},
	}
	f := cmd.Flags()
	f.BoolVarP(&cf.yes, "yes", "y", false, "do not ask for confirmation (Trash and link modes)")
	f.BoolVar(&cf.permanent, "permanent", false, "delete permanently instead of moving to the Trash")
	f.BoolVar(&cf.link, "link", false, "replace duplicates with APFS clones of the kept file")
	f.BoolVar(&cf.force, "force", false, "with --permanent: skip the typed confirmation")
	return cmd
}

func deleteMode(configured string, cf cleanFlags) string {
	switch {
	case cf.permanent:
		return config.ModePermanent
	case cf.link:
		return config.ModeLink
	}
	return configured
}

func (a *app) cleanPlain(ctx context.Context, s session, cf cleanFlags) error {
	out, err := a.scanPlain(ctx, s)
	if err != nil {
		return err
	}
	plan := group.Plan(out.Groups, s.keeper)
	if err := a.printReport(out, s, a.flags.dryRun); err != nil {
		return err
	}
	if len(plan) == 0 {
		return nil
	}
	if !a.flags.dryRun {
		ok, err := a.confirmPlain(plan, s.cfg.DeleteMode, cf)
		if err != nil || !ok {
			return err
		}
	}
	res, err := a.execute(ctx, plan, s.cfg.DeleteMode, nil)
	if err != nil {
		return err
	}
	a.printResult(res, s.cfg.DeleteMode)
	return nil
}

// confirmPlain asks on stdin. Permanent deletion requires typing the word.
func (a *app) confirmPlain(plan []group.Action, mode string, cf cleanFlags) (bool, error) {
	total := fsutil.HumanSize(group.TotalReclaimable(plan))
	n := 0
	for _, act := range plan {
		n += len(act.Remove)
	}
	if mode == config.ModePermanent && !cf.force {
		fmt.Fprintf(a.stdout, "\nPERMANENTLY delete %d files (%s)? This cannot be undone.\nType \"permanent\" to continue: ", n, total)
		return a.readLine() == "permanent", nil
	}
	if cf.yes {
		return true, nil
	}
	fmt.Fprintf(a.stdout, "\n%s %d files (%s)? [y/N] ", verb(mode), n, total)
	answer := strings.ToLower(a.readLine())
	return answer == "y" || answer == "yes", nil
}

func (a *app) readLine() string {
	sc := bufio.NewScanner(a.stdin)
	if sc.Scan() {
		return strings.TrimSpace(sc.Text())
	}
	return ""
}

func verb(mode string) string {
	switch mode {
	case config.ModePermanent:
		return "Delete"
	case config.ModeLink:
		return "Replace with clones"
	}
	return "Move to Trash"
}

// execute runs the plan with the journal open. onProgress is optional.
func (a *app) execute(ctx context.Context, plan []group.Action, mode string, onProgress func(int, int)) (trash.Result, error) {
	remover, err := trash.ForMode(mode)
	if err != nil {
		return trash.Result{}, err
	}
	j, err := a.openJournal()
	if err != nil {
		return trash.Result{}, err
	}
	defer j.Close()
	return trash.Execute(ctx, plan, trash.Options{
		Remover:    remover,
		Journal:    j,
		DryRun:     a.flags.dryRun,
		OnProgress: onProgress,
	})
}

func (a *app) openJournal() (*journal.Journal, error) {
	path, err := journal.DefaultPath()
	if err != nil {
		return nil, err
	}
	if p := os.Getenv("TWINS_JOURNAL"); p != "" {
		path = p
	}
	j, err := journal.Open(path)
	if err != nil {
		fmt.Fprintf(a.stderr, "warning: journal unavailable: %v\n", err)
		return journal.Discard(), nil
	}
	return j, nil
}

func (a *app) printResult(res trash.Result, mode string) {
	if res.DryRun {
		fmt.Fprintf(a.stdout, "\nDry run: would %s %d files, freeing %s.\n",
			strings.ToLower(verb(mode)), res.Removed, fsutil.HumanSize(res.Bytes))
		return
	}
	fmt.Fprintf(a.stdout, "\n%s: %d files, %s freed.\n", verb(mode), res.Removed, fsutil.HumanSize(res.Bytes))
	for _, f := range res.Failures {
		fmt.Fprintf(a.stderr, "failed %s: %v\n", f.Path, f.Err)
	}
	if len(res.Failures) > 0 {
		fmt.Fprintf(a.stderr, "%d files could not be removed.\n", len(res.Failures))
	}
}
