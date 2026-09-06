// Package config loads and saves user preferences from a TOML file in
// ~/Library/Application Support/twins/.
package config

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"github.com/BurntSushi/toml"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
)

// Delete modes.
const (
	ModeTrash     = "trash"
	ModePermanent = "permanent"
	ModeLink      = "link"
)

var deleteModes = []string{ModeTrash, ModePermanent, ModeLink}

// Config is the persisted user configuration. All fields are optional in
// the file: missing values fall back to Default().
type Config struct {
	MinSize            string   `toml:"min_size"`
	Exclude            []string `toml:"exclude"`
	Keep               string   `toml:"keep"`
	KeepDir            string   `toml:"keep_dir"`
	DeleteMode         string   `toml:"delete_mode"`
	IncludeLibrary     bool     `toml:"include_library"`
	IncludeNodeModules bool     `toml:"include_node_modules"`
	IncludeEmpty       bool     `toml:"include_empty"`
	Jobs               int      `toml:"jobs"`
}

// Default returns the built-in configuration.
func Default() Config {
	return Config{
		MinSize:    "1MiB",
		Keep:       string(group.KeepOldest),
		DeleteMode: ModeTrash,
	}
}

// AppDir returns ~/Library/Application Support/twins.
func AppDir() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", fmt.Errorf("home dir: %w", err)
	}
	return filepath.Join(home, "Library", "Application Support", "twins"), nil
}

// DefaultPath returns the default config file location.
func DefaultPath() (string, error) {
	dir, err := AppDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(dir, "config.toml"), nil
}

// Load reads the file at path, merging it over Default(). A missing file
// is not an error.
func Load(path string) (Config, error) {
	cfg := Default()
	meta, err := toml.DecodeFile(path, &cfg)
	if errors.Is(err, os.ErrNotExist) {
		return cfg, nil
	}
	if err != nil {
		return Config{}, fmt.Errorf("read config %s: %w", path, err)
	}
	if undecoded := meta.Undecoded(); len(undecoded) > 0 {
		return Config{}, fmt.Errorf("config %s: unknown keys %v", path, undecoded)
	}
	if err := cfg.Validate(); err != nil {
		return Config{}, fmt.Errorf("config %s: %w", path, err)
	}
	return cfg, nil
}

// Save writes cfg to path, creating parent directories.
func Save(path string, cfg Config) error {
	if err := cfg.Validate(); err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return fmt.Errorf("create config dir: %w", err)
	}
	f, err := os.Create(path)
	if err != nil {
		return fmt.Errorf("write config: %w", err)
	}
	defer f.Close()
	if err := toml.NewEncoder(f).Encode(cfg); err != nil {
		return fmt.Errorf("encode config: %w", err)
	}
	return nil
}

// Validate checks every field.
func (c Config) Validate() error {
	if _, err := fsutil.ParseSize(c.MinSize); err != nil {
		return fmt.Errorf("min_size: %w", err)
	}
	strategy, err := group.ParseStrategy(c.Keep)
	if err != nil {
		return fmt.Errorf("keep: %w", err)
	}
	if strategy == group.KeepInDir && c.KeepDir == "" {
		return errors.New("keep = \"in-dir\" requires keep_dir")
	}
	for _, m := range deleteModes {
		if c.DeleteMode == m {
			return nil
		}
	}
	return fmt.Errorf("delete_mode: %q is not one of %v", c.DeleteMode, deleteModes)
}

// MinSizeBytes parses MinSize.
func (c Config) MinSizeBytes() (int64, error) {
	return fsutil.ParseSize(c.MinSize)
}
