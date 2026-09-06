// Package fsutil provides low-level, macOS-aware file metadata helpers.
package fsutil

import (
	"errors"
	"fmt"
	"path/filepath"
	"strings"
	"time"

	"golang.org/x/sys/unix"
)

// ErrNotRegular is returned when a path is not a regular file
// (symlink, directory, device, socket, FIFO...).
var ErrNotRegular = errors.New("not a regular file")

// FileMeta is an immutable snapshot of a file's identity and size.
type FileMeta struct {
	Path     string
	Size     int64
	ModTime  time.Time
	Dev      uint64
	Inode    uint64
	Nlink    uint64
	Dataless bool // iCloud placeholder: reading it would trigger a download.
}

// Identity uniquely identifies the physical file on disk.
// Two paths with the same Identity are hardlinks of the same data.
type Identity struct {
	Dev   uint64
	Inode uint64
}

// Identity returns the (device, inode) pair of the file.
func (m FileMeta) Identity() Identity {
	return Identity{Dev: m.Dev, Inode: m.Inode}
}

// String renders the identity for use as a map or cache key.
func (id Identity) String() string {
	return fmt.Sprintf("%d:%d", id.Dev, id.Inode)
}

// Stat returns the metadata of a regular file without following symlinks.
func Stat(path string) (FileMeta, error) {
	var st unix.Stat_t
	if err := unix.Lstat(path, &st); err != nil {
		return FileMeta{}, fmt.Errorf("lstat %s: %w", path, err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFREG {
		return FileMeta{}, fmt.Errorf("%s: %w", path, ErrNotRegular)
	}
	return FileMeta{
		Path:     path,
		Size:     st.Size,
		ModTime:  time.Unix(st.Mtim.Sec, st.Mtim.Nsec),
		Dev:      uint64(st.Dev),
		Inode:    st.Ino,
		Nlink:    uint64(st.Nlink),
		Dataless: st.Flags&unix.SF_DATALESS != 0,
	}, nil
}

// IsLocalVolume reports whether the filesystem holding path is a local
// (non-network) mount.
func IsLocalVolume(path string) (bool, error) {
	var fs unix.Statfs_t
	if err := unix.Statfs(path, &fs); err != nil {
		return false, fmt.Errorf("statfs %s: %w", path, err)
	}
	return fs.Flags&unix.MNT_LOCAL != 0, nil
}

// bundleExtensions lists macOS package directories that must be treated as
// atomic units and never descended into.
var bundleExtensions = map[string]struct{}{
	".app":           {},
	".framework":     {},
	".bundle":        {},
	".plugin":        {},
	".kext":          {},
	".photoslibrary": {},
	".musiclibrary":  {},
	".tvlibrary":     {},
	".sparsebundle":  {},
	".xcodeproj":     {},
	".xcworkspace":   {},
	".pkg":           {},
	".prefpane":      {},
	".qlgenerator":   {},
	".appex":         {},
	".playground":    {},
}

// IsBundle reports whether a directory name denotes a macOS bundle.
func IsBundle(name string) bool {
	_, ok := bundleExtensions[strings.ToLower(filepath.Ext(name))]
	return ok
}
