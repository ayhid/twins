package safety

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestIsProtected(t *testing.T) {
	cases := map[string]bool{
		"/":                      true,
		"/System":                true,
		"/System/Library/Fonts":  true,
		"/usr/local/bin/twins":   true,
		"/private/var/db":        true,
		"/private/var/folders/x": false,
		"/private/tmp/build":     false,
		"/var/folders/rs/x":      false,
		"/Applications/Foo.app":  true,
		"/Users/me/Documents":    false,
		"/Volumes/Data/Movies":   false,
		"/Systemic":              false,
		"/Users/me/../../System": true,
	}
	for path, want := range cases {
		assert.Equal(t, want, IsProtected(path), path)
	}
}

func TestIsUserLibrary(t *testing.T) {
	home, _ := os.UserHomeDir()
	assert.True(t, IsUserLibrary(filepath.Join(home, "Library")))
	assert.True(t, IsUserLibrary(filepath.Join(home, "Library", "Caches", "x")))
	assert.False(t, IsUserLibrary(filepath.Join(home, "Documents")))
	assert.False(t, IsUserLibrary(filepath.Join(home, "LibraryBackup")))
	assert.False(t, IsUserLibrary(t.TempDir()))
}
