// Package trash executes a validated plan: moving duplicates to the
// Trash, deleting them permanently, or replacing them with APFS clones.
package trash

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"golang.org/x/sys/unix"

	"github.com/ayhid/twins/internal/config"
	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/journal"
)

// ErrCrossDevice is returned when a clone would span two volumes.
var ErrCrossDevice = errors.New("cannot clone across volumes")

// Remover disposes of a duplicate given the file to keep.
type Remover interface {
	// Remove disposes of dup. keep is the surviving copy (used by Link).
	Remove(dup, keep fsutil.FileMeta) error
	// Action is the journal action name.
	Action() string
}

// ForMode returns the Remover for a config delete mode.
func ForMode(mode string) (Remover, error) {
	switch mode {
	case config.ModeTrash:
		return Trash{}, nil
	case config.ModePermanent:
		return Permanent{}, nil
	case config.ModeLink:
		return Link{}, nil
	}
	return nil, fmt.Errorf("unknown delete mode %q", mode)
}

// Permanent unlinks the file. Irreversible.
type Permanent struct{}

// Remove implements Remover.
func (Permanent) Remove(dup, _ fsutil.FileMeta) error {
	if err := os.Remove(dup.Path); err != nil {
		return fmt.Errorf("delete: %w", err)
	}
	return nil
}

// Action implements Remover.
func (Permanent) Action() string { return journal.ActionDelete }

// Link replaces the duplicate with an APFS clone of the kept file. The
// data is shared copy-on-write, so the space is freed while the path
// keeps working. The replacement is atomic (clone to temp, rename over).
type Link struct{}

// Remove implements Remover.
func (Link) Remove(dup, keep fsutil.FileMeta) error {
	if dup.Dev != keep.Dev {
		return fmt.Errorf("%s: %w", dup.Path, ErrCrossDevice)
	}
	tmp := filepath.Join(filepath.Dir(dup.Path), ".twins-clone-"+filepath.Base(dup.Path))
	if err := unix.Clonefile(keep.Path, tmp, unix.CLONE_NOFOLLOW); err != nil {
		return fmt.Errorf("clonefile: %w", err)
	}
	if err := os.Rename(tmp, dup.Path); err != nil {
		_ = os.Remove(tmp)
		return fmt.Errorf("replace with clone: %w", err)
	}
	return nil
}

// Action implements Remover.
func (Link) Action() string { return journal.ActionLink }

// Trash moves the file to the user's Trash, where Finder can put it back.
type Trash struct{}

// Remove implements Remover.
func (Trash) Remove(dup, _ fsutil.FileMeta) error { return moveToTrash(dup.Path) }

// Action implements Remover.
func (Trash) Action() string { return journal.ActionTrash }
