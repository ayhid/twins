package safety

import (
	"testing"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func meta(path string, inode uint64) fsutil.FileMeta {
	return fsutil.FileMeta{Path: path, Size: 10, Dev: 1, Inode: inode}
}

func action(keep fsutil.FileMeta, remove ...fsutil.FileMeta) group.Action {
	files := append([]fsutil.FileMeta{keep}, remove...)
	return group.Action{
		Group:  group.Group{Size: 10, Files: files},
		Keep:   keep,
		Remove: remove,
	}
}

func TestValidate_AcceptsWellFormedPlan(t *testing.T) {
	a := action(meta("/Users/me/a", 1), meta("/Users/me/b", 2))
	require.NoError(t, Validate([]group.Action{a}))
}

func TestValidate_RejectsKeepInsideRemove(t *testing.T) {
	keep := meta("/Users/me/a", 1)
	a := action(keep, meta("/Users/me/b", 2))
	a.Remove = append(a.Remove, keep)
	assert.ErrorIs(t, Validate([]group.Action{a}), ErrKeepRemoved)
}

func TestValidate_RejectsKeepOutsideGroup(t *testing.T) {
	a := action(meta("/Users/me/a", 1), meta("/Users/me/b", 2))
	a.Keep = meta("/Users/me/elsewhere", 9)
	assert.ErrorIs(t, Validate([]group.Action{a}), ErrKeepNotInGroup)
}

func TestValidate_RejectsRemoveOutsideGroup(t *testing.T) {
	a := action(meta("/Users/me/a", 1), meta("/Users/me/b", 2))
	a.Remove = append(a.Remove, meta("/Users/me/stranger", 7))
	assert.ErrorIs(t, Validate([]group.Action{a}), ErrRemoveNotInGroup)
}

func TestValidate_RejectsRemovingAllPhysicalCopies(t *testing.T) {
	keep := meta("/Users/me/a", 1)
	link := meta("/Users/me/a-link", 1)
	a := action(keep, meta("/Users/me/b", 2))
	// Removing every path of the kept inode leaves no copy at all.
	a.Group.Files = append(a.Group.Files, link)
	a.Remove = append(a.Remove, link)
	a.Keep = link
	a.Remove = append(a.Remove, keep)
	assert.ErrorIs(t, Validate([]group.Action{a}), ErrKeepRemoved)
}

func TestValidate_RejectsProtectedPaths(t *testing.T) {
	a := action(meta("/Users/me/a", 1), meta("/System/Library/x", 2))
	assert.ErrorIs(t, Validate([]group.Action{a}), ErrProtectedPath)
}

func TestValidate_ReportsEveryViolation(t *testing.T) {
	bad := action(meta("/Users/me/a", 1), meta("/usr/lib/x", 2))
	bad.Remove = append(bad.Remove, bad.Keep)
	err := Validate([]group.Action{bad})
	assert.ErrorIs(t, err, ErrProtectedPath)
	assert.ErrorIs(t, err, ErrKeepRemoved)
}

func TestValidate_EmptyPlanIsValid(t *testing.T) {
	assert.NoError(t, Validate(nil))
}
