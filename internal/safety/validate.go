package safety

import (
	"errors"
	"fmt"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
)

// Invariant violations. Validate wraps them with the offending path.
var (
	ErrKeepRemoved      = errors.New("kept file is scheduled for removal")
	ErrKeepNotInGroup   = errors.New("kept file does not belong to the group")
	ErrRemoveNotInGroup = errors.New("file to remove does not belong to the group")
	ErrProtectedPath    = errors.New("path is in a protected location")
)

// Validate checks every action against the invariants that make a plan
// safe to execute: the kept file exists in its group, is never removed
// (directly or through a hardlink), every removal belongs to the group
// and no removal touches a protected location. All violations are
// reported, joined into a single error.
func Validate(actions []group.Action) error {
	var errs []error
	for _, a := range actions {
		errs = append(errs, validateAction(a)...)
	}
	return errors.Join(errs...)
}

func validateAction(a group.Action) []error {
	var errs []error
	members := make(map[string]fsutil.Identity, len(a.Group.Files))
	for _, f := range a.Group.Files {
		members[f.Path] = f.Identity()
	}
	if _, ok := members[a.Keep.Path]; !ok {
		errs = append(errs, fmt.Errorf("%s: %w", a.Keep.Path, ErrKeepNotInGroup))
	}
	removedIdentities := make(map[fsutil.Identity]int)
	for _, r := range a.Remove {
		if _, ok := members[r.Path]; !ok {
			errs = append(errs, fmt.Errorf("%s: %w", r.Path, ErrRemoveNotInGroup))
		}
		if r.Path == a.Keep.Path {
			errs = append(errs, fmt.Errorf("%s: %w", r.Path, ErrKeepRemoved))
		}
		if IsProtected(r.Path) {
			errs = append(errs, fmt.Errorf("%s: %w", r.Path, ErrProtectedPath))
		}
		removedIdentities[r.Identity()]++
	}
	if lostAllLinks(a, members, removedIdentities) {
		errs = append(errs, fmt.Errorf("%s: %w (all hardlinks removed)", a.Keep.Path, ErrKeepRemoved))
	}
	return errs
}

// lostAllLinks reports whether every path sharing the kept file's inode is
// scheduled for removal, which would destroy the data.
func lostAllLinks(a group.Action, members map[string]fsutil.Identity, removed map[fsutil.Identity]int) bool {
	total := 0
	for _, id := range members {
		if id == a.Keep.Identity() {
			total++
		}
	}
	return total > 0 && removed[a.Keep.Identity()] >= total
}
