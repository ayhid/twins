// Package cli wires the twins commands together.
package cli

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/signal"
	"syscall"

	"github.com/spf13/cobra"

	"github.com/ayhid/twins/internal/config"
)

// version is injected at build time via -ldflags.
var version = "dev"

// Exit codes.
const (
	ExitOK    = 0
	ExitError = 1
	ExitUsage = 2
)

// Main runs the CLI against the process streams and returns the exit code.
func Main(args []string) int {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	return Execute(ctx, args, os.Stdin, os.Stdout, os.Stderr)
}

// Execute runs the CLI with explicit streams (used by tests).
func Execute(ctx context.Context, args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	app := &app{stdin: stdin, stdout: stdout, stderr: stderr}
	root := app.rootCommand()
	root.SetArgs(args)
	root.SetIn(stdin)
	root.SetOut(stdout)
	root.SetErr(stderr)
	if err := root.ExecuteContext(ctx); err != nil {
		var usage usageError
		if errors.As(err, &usage) {
			return ExitUsage
		}
		fmt.Fprintf(stderr, "twins: %v\n", err)
		return ExitError
	}
	return ExitOK
}

// usageError marks errors caused by invalid invocation.
type usageError struct{ error }

// usage prints the error and the command usage, then returns a usageError.
func (a *app) usage(cmd *cobra.Command, err error) error {
	fmt.Fprintf(a.stderr, "twins: %v\n\n", err)
	_ = cmd.Usage()
	return usageError{err}
}

// app holds the streams and global flags shared by every command.
type app struct {
	stdin  io.Reader
	stdout io.Writer
	stderr io.Writer
	flags  globalFlags
}

// globalFlags mirror config.Config plus per-run switches.
type globalFlags struct {
	configPath         string
	minSize            string
	exclude            []string
	keep               string
	keepDir            string
	jobs               int
	includeLibrary     bool
	includeNodeModules bool
	includeEmpty       bool
	includeRemote      bool
	noCache            bool
	verify             bool
	verbose            bool
	json               bool
	plain              bool
	dryRun             bool
}

func (a *app) rootCommand() *cobra.Command {
	root := &cobra.Command{
		Use:   "twins",
		Short: "Find and remove duplicate files, safely",
		Long: `twins finds files with identical content and helps you reclaim the space.
Nothing is deleted without confirmation; duplicates go to the Trash by default.

Run without arguments for the interactive menu.`,
		SilenceUsage:  true,
		SilenceErrors: true,
		Version:       version,
		RunE: func(cmd *cobra.Command, args []string) error {
			return a.runMenu(cmd.Context())
		},
	}
	root.SetVersionTemplate("twins {{.Version}}\n")
	root.SetFlagErrorFunc(func(cmd *cobra.Command, err error) error {
		fmt.Fprintf(a.stderr, "twins: %v\n\n", err)
		_ = cmd.Usage()
		return usageError{err}
	})

	pf := root.PersistentFlags()
	pf.StringVar(&a.flags.configPath, "config", "", "config file (default ~/Library/Application Support/twins/config.toml)")
	pf.StringVar(&a.flags.minSize, "min-size", "", "ignore files smaller than this, e.g. 1MiB (default from config)")
	pf.StringArrayVarP(&a.flags.exclude, "exclude", "e", nil, "glob to skip; repeatable (matched on name, or path if it contains /)")
	pf.StringVar(&a.flags.keep, "keep", "", "which copy to keep: oldest|newest|shortest-path|in-dir")
	pf.StringVar(&a.flags.keepDir, "keep-in", "", "with --keep in-dir: prefer copies under this directory")
	pf.IntVarP(&a.flags.jobs, "jobs", "j", 0, "parallel hashing workers (default: CPUs, max 8)")
	pf.BoolVar(&a.flags.includeLibrary, "include-library", false, "scan ~/Library too")
	pf.BoolVar(&a.flags.includeNodeModules, "include-node-modules", false, "scan node_modules directories too")
	pf.BoolVar(&a.flags.includeEmpty, "include-empty", false, "consider empty files as duplicates of each other")
	pf.BoolVar(&a.flags.includeRemote, "include-remote", false, "allow scanning network volumes")
	pf.BoolVar(&a.flags.noCache, "no-cache", false, "do not read or write the hash cache")
	pf.BoolVar(&a.flags.verify, "verify", false, "compare duplicates byte by byte after hashing")
	pf.BoolVarP(&a.flags.verbose, "verbose", "v", false, "report skipped files and errors on stderr")
	pf.BoolVar(&a.flags.json, "json", false, "machine-readable JSON output")
	pf.BoolVar(&a.flags.plain, "plain", false, "plain text output instead of the interactive browser")
	pf.BoolVarP(&a.flags.dryRun, "dry-run", "n", false, "show what would happen without touching any file")

	root.AddCommand(a.scanCommand(), a.cleanCommand(), a.reportCommand(), a.configCommand(), a.versionCommand())
	return root
}

func (a *app) loadConfig() (config.Config, string, error) {
	path := a.flags.configPath
	if path == "" {
		p, err := config.DefaultPath()
		if err != nil {
			return config.Config{}, "", err
		}
		path = p
	}
	cfg, err := config.Load(path)
	return cfg, path, err
}

func (a *app) versionCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "version",
		Short: "Print the version",
		RunE: func(cmd *cobra.Command, _ []string) error {
			fmt.Fprintf(a.stdout, "twins %s\n", version)
			return nil
		},
	}
}
