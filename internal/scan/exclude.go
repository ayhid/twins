package scan

import (
	"path/filepath"
	"strings"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/safety"
)

// alwaysExcludedDirs are directory names skipped at any depth.
var alwaysExcludedDirs = map[string]struct{}{
	".git":                    {},
	".hg":                     {},
	".svn":                    {},
	".Trash":                  {},
	".Trashes":                {},
	"Caches":                  {},
	".cache":                  {},
	"Backups.backupdb":        {},
	".Spotlight-V100":         {},
	".fseventsd":              {},
	".DocumentRevisions-V100": {},
	".TemporaryItems":         {},
	".MobileBackups":          {},
	"__pycache__":             {},
}

// optionalExcludedDirs are skipped unless the matching option is set.
const nodeModules = "node_modules"

// rules is the immutable, pre-compiled exclusion set for one walk.
type rules struct {
	includeLibrary     bool
	includeNodeModules bool
	baseGlobs          []string // patterns without a separator
	pathGlobs          []string // patterns with a separator, matched on the full path
}

func compileRules(opts Options) rules {
	r := rules{includeLibrary: opts.IncludeLibrary, includeNodeModules: opts.IncludeNodeModules}
	for _, g := range opts.Exclude {
		if strings.Contains(g, "/") {
			r.pathGlobs = append(r.pathGlobs, g)
		} else {
			r.baseGlobs = append(r.baseGlobs, g)
		}
	}
	return r
}

// skipDir reports whether a directory must not be descended into.
func (r rules) skipDir(path, name string) bool {
	if _, ok := alwaysExcludedDirs[name]; ok {
		return true
	}
	if name == nodeModules && !r.includeNodeModules {
		return true
	}
	if fsutil.IsBundle(name) {
		return true
	}
	if safety.IsProtected(path) {
		return true
	}
	if !r.includeLibrary && safety.IsUserLibrary(path) {
		return true
	}
	return r.matchesGlob(path, name)
}

// skipFile reports whether a file is excluded by user globs.
func (r rules) skipFile(path, name string) bool {
	return r.matchesGlob(path, name)
}

func (r rules) matchesGlob(path, name string) bool {
	for _, g := range r.baseGlobs {
		if ok, _ := filepath.Match(g, name); ok {
			return true
		}
	}
	for _, g := range r.pathGlobs {
		if ok, _ := filepath.Match(g, path); ok {
			return true
		}
	}
	return false
}
