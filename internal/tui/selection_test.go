package tui

import (
	"testing"
	"time"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func file(path string, inode uint64, age int) fsutil.FileMeta {
	return fsutil.FileMeta{Path: path, Size: 100, Dev: 1, Inode: inode,
		ModTime: time.Date(2024, 1, 1, 0, 0, 0, 0, time.UTC).Add(time.Duration(age) * time.Hour)}
}

func twoGroups() []group.Group {
	return []group.Group{
		{Size: 100, Files: []fsutil.FileMeta{file("/a1", 1, 0), file("/a2", 2, 1), file("/a3", 3, 2)}},
		{Size: 100, Files: []fsutil.FileMeta{file("/b1", 4, 5), file("/b2", 5, 1)}},
	}
}

var oldest = group.Keeper{Strategy: group.KeepOldest}

func TestNewSelection_AppliesKeeper(t *testing.T) {
	s := NewSelection(twoGroups(), oldest)
	assert.Equal(t, "/a1", s.Keep(0).Path)
	assert.True(t, s.IsMarked(0, "/a2"))
	assert.True(t, s.IsMarked(0, "/a3"))
	assert.False(t, s.IsMarked(0, "/a1"))
	assert.Equal(t, "/b2", s.Keep(1).Path)
	assert.Equal(t, 3, s.MarkedCount())
	assert.EqualValues(t, 300, s.Reclaimable())
}

func TestToggle_IsImmutable(t *testing.T) {
	s := NewSelection(twoGroups(), oldest)
	s2, err := s.Toggle(0, "/a2")
	require.NoError(t, err)
	assert.True(t, s.IsMarked(0, "/a2"), "original untouched")
	assert.False(t, s2.IsMarked(0, "/a2"))
	assert.Equal(t, 2, s2.MarkedCount())
}

func TestToggle_MarkingKeptFileMovesTheStar(t *testing.T) {
	s := NewSelection(twoGroups(), oldest)
	s, err := s.Toggle(0, "/a2") // unmark a2
	require.NoError(t, err)
	s, err = s.Toggle(0, "/a1") // mark the old keep
	require.NoError(t, err)
	assert.Equal(t, "/a2", s.Keep(0).Path)
	assert.True(t, s.IsMarked(0, "/a1"))
}

func TestToggle_RefusesLastCopy(t *testing.T) {
	s := NewSelection(twoGroups(), oldest)
	_, err := s.Toggle(1, "/b2")
	assert.ErrorIs(t, err, ErrLastCopy)
}

func TestToggleGroup_ClearsThenReapplies(t *testing.T) {
	s := NewSelection(twoGroups(), oldest)
	s = s.ToggleGroup(0)
	assert.Equal(t, 0, s.GroupMarked(0))
	assert.Equal(t, 1, s.MarkedCount())
	s = s.ToggleGroup(0)
	assert.Equal(t, 2, s.GroupMarked(0))
}

func TestClearAllAndAutoAll(t *testing.T) {
	s := NewSelection(twoGroups(), oldest).ClearAll()
	assert.Equal(t, 0, s.MarkedCount())
	assert.Empty(t, s.Plan())
	s = s.AutoAll()
	assert.Equal(t, 3, s.MarkedCount())
}

func TestPlan_SkipsUntouchedGroupsAndKeepsInvariant(t *testing.T) {
	s := NewSelection(twoGroups(), oldest).ToggleGroup(1)
	plan := s.Plan()
	require.Len(t, plan, 1)
	assert.Equal(t, "/a1", plan[0].Keep.Path)
	assert.Len(t, plan[0].Remove, 2)
	for _, r := range plan[0].Remove {
		assert.NotEqual(t, plan[0].Keep.Path, r.Path)
	}
}
