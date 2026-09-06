// Package safety enforces the guard rails that make twins safe to run:
// protected locations, keep-one invariants and dry-run semantics.
package safety

import (
	"os"
	"path/filepath"
	"strings"
)

// protectedPrefixes are never scanned nor modified, whatever the flags.
var protectedPrefixes = []string{
	"/System",
	"/Library",
	"/usr",
	"/bin",
	"/sbin",
	"/private/etc",
	"/private/var",
	"/etc",
	"/var",
	"/dev",
	"/cores",
	"/Applications",
}

// allowedPrefixes are user-writable temporary areas carved out of the
// protected tree (macOS puts per-user temp dirs under /private/var/folders).
var allowedPrefixes = []string{
	"/private/var/folders",
	"/private/var/tmp",
	"/private/tmp",
	"/var/folders",
	"/var/tmp",
	"/tmp",
}

// IsProtected reports whether path lies inside a hard-coded protected
// location. The path is cleaned but symlinks are not resolved.
func IsProtected(path string) bool {
	abs, err := filepath.Abs(path)
	if err != nil {
		return true
	}
	abs = filepath.Clean(abs)
	if abs == "/" {
		return true
	}
	if hasPrefix(abs, allowedPrefixes) {
		return false
	}
	return hasPrefix(abs, protectedPrefixes)
}

func hasPrefix(abs string, prefixes []string) bool {
	for _, prefix := range prefixes {
		if abs == prefix || strings.HasPrefix(abs, prefix+"/") {
			return true
		}
	}
	return false
}

// IsUserLibrary reports whether path is the current user's ~/Library
// folder or lies inside it. It is excluded unless explicitly opted in.
func IsUserLibrary(path string) bool {
	home, err := os.UserHomeDir()
	if err != nil {
		return false
	}
	lib := filepath.Join(home, "Library")
	abs, err := filepath.Abs(path)
	if err != nil {
		return false
	}
	return abs == lib || strings.HasPrefix(abs, lib+"/")
}
