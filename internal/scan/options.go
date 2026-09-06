// Package scan walks directory trees in parallel and emits candidate files.
package scan

import (
	"errors"
	"sync/atomic"

	"github.com/ayhid/twins/internal/fsutil"
)

// DefaultMinSize is the default lower bound for candidate files (1 MiB).
// Small files are numerous and rarely worth reclaiming.
const DefaultMinSize int64 = 1 << 20

var (
	// ErrProtectedRoot is returned when a root lies in a protected location.
	ErrProtectedRoot = errors.New("root is a protected system location")
	// ErrRemoteRoot is returned when a root is on a network volume.
	ErrRemoteRoot = errors.New("root is on a non-local volume")
	// ErrNoRoots is returned when no root is given.
	ErrNoRoots = errors.New("no root directory to scan")
)

// Options controls a walk. The zero value scans nothing: at least one
// root is required.
type Options struct {
	Roots              []string
	MinSize            int64
	IncludeEmpty       bool
	IncludeLibrary     bool // descend into ~/Library
	IncludeNodeModules bool
	IncludeRemote      bool     // allow non-local volumes
	Exclude            []string // glob patterns matched on base name (or full path if they contain "/")
	Workers            int      // ≤ 0 means runtime default
	// OnError is invoked for non-fatal errors (permission denied, vanished
	// files...). The walk continues. Optional.
	OnError func(path string, err error)
}

// Visit receives each candidate file. It is called concurrently from
// several goroutines and must be safe for parallel use.
type Visit func(fsutil.FileMeta)

// Stats summarises a completed walk.
type Stats struct {
	Files      int64 // regular files seen
	Candidates int64 // files passed to Visit
	Dirs       int64
	Skipped    int64 // files or dirs excluded by rules
	Errors     int64 // non-fatal errors
}

type counters struct {
	files, candidates, dirs, skipped, errors atomic.Int64
}

func (c *counters) snapshot() Stats {
	return Stats{
		Files:      c.files.Load(),
		Candidates: c.candidates.Load(),
		Dirs:       c.dirs.Load(),
		Skipped:    c.skipped.Load(),
		Errors:     c.errors.Load(),
	}
}
