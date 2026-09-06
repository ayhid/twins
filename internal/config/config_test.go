package config

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestDefault_IsValid(t *testing.T) {
	cfg := Default()
	require.NoError(t, cfg.Validate())
	assert.Equal(t, "1MiB", cfg.MinSize)
	assert.Equal(t, "oldest", cfg.Keep)
	assert.Equal(t, "trash", cfg.DeleteMode)
	min, err := cfg.MinSizeBytes()
	require.NoError(t, err)
	assert.EqualValues(t, 1<<20, min)
}

func TestLoad_MissingFileReturnsDefaults(t *testing.T) {
	cfg, err := Load(filepath.Join(t.TempDir(), "none.toml"))
	require.NoError(t, err)
	assert.Equal(t, Default(), cfg)
}

func TestSaveThenLoad_RoundTrips(t *testing.T) {
	path := filepath.Join(t.TempDir(), "sub", "config.toml")
	cfg := Default()
	cfg.MinSize = "10MiB"
	cfg.Exclude = []string{"*.log", "build"}
	cfg.Keep = "in-dir"
	cfg.KeepDir = "/Users/me/Photos"
	cfg.IncludeLibrary = true
	cfg.Jobs = 4

	require.NoError(t, Save(path, cfg))
	back, err := Load(path)
	require.NoError(t, err)
	assert.Equal(t, cfg, back)
}

func TestLoad_RejectsInvalidValues(t *testing.T) {
	path := filepath.Join(t.TempDir(), "config.toml")
	require.NoError(t, os.WriteFile(path, []byte("min_size = \"lots\"\n"), 0o644))
	_, err := Load(path)
	assert.Error(t, err)

	require.NoError(t, os.WriteFile(path, []byte("keep = \"random\"\n"), 0o644))
	_, err = Load(path)
	assert.Error(t, err)

	require.NoError(t, os.WriteFile(path, []byte("delete_mode = \"shred\"\n"), 0o644))
	_, err = Load(path)
	assert.Error(t, err)

	require.NoError(t, os.WriteFile(path, []byte("not toml"), 0o644))
	_, err = Load(path)
	assert.Error(t, err)
}

func TestValidate_InDirRequiresKeepDir(t *testing.T) {
	cfg := Default()
	cfg.Keep = "in-dir"
	assert.Error(t, cfg.Validate())
	cfg.KeepDir = "/tmp"
	assert.NoError(t, cfg.Validate())
}

func TestDefaultPath_LivesInApplicationSupport(t *testing.T) {
	p, err := DefaultPath()
	require.NoError(t, err)
	assert.Contains(t, p, filepath.Join("Library", "Application Support", "twins", "config.toml"))
}
