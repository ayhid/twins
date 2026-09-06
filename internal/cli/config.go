package cli

import (
	"fmt"
	"os"

	"github.com/BurntSushi/toml"
	"github.com/spf13/cobra"

	"github.com/ayhid/twins/internal/cache"
	"github.com/ayhid/twins/internal/config"
	"github.com/ayhid/twins/internal/journal"
)

func (a *app) configCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "config",
		Short: "Show or manage the configuration and cache",
		RunE:  func(cmd *cobra.Command, _ []string) error { return a.configShow() },
	}
	cmd.AddCommand(
		&cobra.Command{Use: "show", Short: "Print the effective configuration", RunE: func(*cobra.Command, []string) error { return a.configShow() }},
		&cobra.Command{Use: "path", Short: "Print the config, cache and journal paths", RunE: func(*cobra.Command, []string) error { return a.configPath() }},
		&cobra.Command{Use: "init", Short: "Write a config file with the defaults", RunE: func(*cobra.Command, []string) error { return a.configInit() }},
		&cobra.Command{Use: "clear-cache", Short: "Delete the hash cache", RunE: func(*cobra.Command, []string) error { return a.configClearCache() }},
	)
	return cmd
}

func (a *app) configShow() error {
	cfg, path, err := a.loadConfig()
	if err != nil {
		return err
	}
	fmt.Fprintf(a.stdout, "# %s\n", path)
	return toml.NewEncoder(a.stdout).Encode(cfg)
}

func (a *app) configPath() error {
	_, cfgPath, err := a.loadConfig()
	if err != nil {
		return err
	}
	cachePath, err := cache.DefaultPath()
	if err != nil {
		return err
	}
	logPath, err := journal.DefaultPath()
	if err != nil {
		return err
	}
	fmt.Fprintf(a.stdout, "config:  %s\ncache:   %s\njournal: %s\n", cfgPath, cachePath, logPath)
	return nil
}

func (a *app) configInit() error {
	_, path, err := a.loadConfig()
	if err != nil {
		return err
	}
	if _, err := os.Stat(path); err == nil {
		return fmt.Errorf("%s already exists", path)
	}
	if err := config.Save(path, config.Default()); err != nil {
		return err
	}
	fmt.Fprintf(a.stdout, "wrote %s\n", path)
	return nil
}

func (a *app) configClearCache() error {
	path, err := cache.DefaultPath()
	if err != nil {
		return err
	}
	if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("clear cache: %w", err)
	}
	fmt.Fprintf(a.stdout, "cache cleared\n")
	return nil
}
