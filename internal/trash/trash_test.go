package trash

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
	"github.com/ayhid/twins/internal/journal"
	"github.com/ayhid/twins/internal/safety"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func write(t *testing.T, dir, name, content string) fsutil.FileMeta {
	t.Helper()
	p := filepath.Join(dir, name)
	require.NoError(t, os.WriteFile(p, []byte(content), 0o644))
	m, err := fsutil.Stat(p)
	require.NoError(t, err)
	return m
}

func planOf(keep fsutil.FileMeta, remove ...fsutil.FileMeta) []group.Action {
	files := append([]fsutil.FileMeta{keep}, remove...)
	return []group.Action{{
		Group:  group.Group{Size: keep.Size, Files: files},
		Keep:   keep,
		Remove: remove,
	}}
}

// recorder is a fake Remover.
type recorder struct {
	removed []string
	fail    map[string]error
}

func (r *recorder) Remove(dup, _ fsutil.FileMeta) error {
	if err, ok := r.fail[dup.Path]; ok {
		return err
	}
	r.removed = append(r.removed, dup.Path)
	return nil
}

func (r *recorder) Action() string { return "fake" }

func TestExecute_RemovesEveryDuplicateAndReportsBytes(t *testing.T) {
	dir := t.TempDir()
	keep := write(t, dir, "keep", "0123456789")
	a := write(t, dir, "a", "0123456789")
	b := write(t, dir, "b", "0123456789")
	rec := &recorder{}

	var progress []int
	res, err := Execute(context.Background(), planOf(keep, a, b), Options{
		Remover:    rec,
		OnProgress: func(done, total int) { progress = append(progress, done*10+total) },
	})
	require.NoError(t, err)
	assert.Equal(t, []string{a.Path, b.Path}, rec.removed)
	assert.Equal(t, 2, res.Removed)
	assert.EqualValues(t, 20, res.Bytes)
	assert.Empty(t, res.Failures)
	assert.Equal(t, []int{12, 22}, progress)
}

func TestExecute_DryRunTouchesNothingButJournals(t *testing.T) {
	dir := t.TempDir()
	keep := write(t, dir, "keep", "xx")
	a := write(t, dir, "a", "xx")
	logPath := filepath.Join(dir, "ops.log")
	j, err := journal.Open(logPath)
	require.NoError(t, err)
	rec := &recorder{}

	res, err := Execute(context.Background(), planOf(keep, a), Options{Remover: rec, Journal: j, DryRun: true})
	require.NoError(t, err)
	require.NoError(t, j.Close())

	assert.Empty(t, rec.removed)
	assert.True(t, res.DryRun)
	assert.Equal(t, 1, res.Removed)
	_, statErr := os.Stat(a.Path)
	assert.NoError(t, statErr, "file must still exist")
	logged, _ := os.ReadFile(logPath)
	assert.Contains(t, string(logged), `"action":"dry-run"`)
	assert.Contains(t, string(logged), a.Path)
}

func TestExecute_DryRunNeedsNoRemover(t *testing.T) {
	dir := t.TempDir()
	keep := write(t, dir, "keep", "xx")
	a := write(t, dir, "a", "xx")
	_, err := Execute(context.Background(), planOf(keep, a), Options{DryRun: true})
	assert.NoError(t, err)
	_, err = Execute(context.Background(), planOf(keep, a), Options{})
	assert.Error(t, err)
}

func TestExecute_RefusesUnsafePlan(t *testing.T) {
	dir := t.TempDir()
	keep := write(t, dir, "keep", "xx")
	plan := planOf(keep, keep) // keep scheduled for removal
	rec := &recorder{}
	_, err := Execute(context.Background(), plan, Options{Remover: rec})
	assert.ErrorIs(t, err, safety.ErrKeepRemoved)
	assert.Empty(t, rec.removed)
}

func TestExecute_RecordsFailuresAndContinues(t *testing.T) {
	dir := t.TempDir()
	keep := write(t, dir, "keep", "xx")
	a := write(t, dir, "a", "xx")
	b := write(t, dir, "b", "xx")
	boom := errors.New("boom")
	rec := &recorder{fail: map[string]error{a.Path: boom}}

	res, err := Execute(context.Background(), planOf(keep, a, b), Options{Remover: rec})
	require.NoError(t, err)
	assert.Equal(t, []string{b.Path}, rec.removed)
	assert.Equal(t, 1, res.Removed)
	require.Len(t, res.Failures, 1)
	assert.Equal(t, a.Path, res.Failures[0].Path)
	assert.ErrorIs(t, res.Failures[0].Err, boom)
}

func TestExecute_StopsOnCancelledContext(t *testing.T) {
	dir := t.TempDir()
	keep := write(t, dir, "keep", "xx")
	a := write(t, dir, "a", "xx")
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	_, err := Execute(ctx, planOf(keep, a), Options{Remover: &recorder{}})
	assert.ErrorIs(t, err, context.Canceled)
}

func TestPermanent_DeletesFile(t *testing.T) {
	dir := t.TempDir()
	keep := write(t, dir, "keep", "xx")
	a := write(t, dir, "a", "xx")
	require.NoError(t, Permanent{}.Remove(a, keep))
	_, err := os.Stat(a.Path)
	assert.True(t, os.IsNotExist(err))
	assert.Error(t, Permanent{}.Remove(a, keep), "second removal fails")
}

func TestLink_ReplacesDuplicateWithCloneOfKeptFile(t *testing.T) {
	dir := t.TempDir()
	keep := write(t, dir, "keep", "shared content")
	dup := write(t, dir, "dup", "shared content")

	require.NoError(t, Link{}.Remove(dup, keep))

	content, err := os.ReadFile(dup.Path)
	require.NoError(t, err)
	assert.Equal(t, "shared content", string(content))
	after, err := fsutil.Stat(dup.Path)
	require.NoError(t, err)
	assert.NotEqual(t, dup.Inode, after.Inode, "path now points to a fresh clone")
	assert.NotEqual(t, keep.Inode, after.Inode, "clone is not a hardlink")
	entries, _ := os.ReadDir(dir)
	assert.Len(t, entries, 2, "no temp file left behind")
}

func TestLink_RefusesCrossDevice(t *testing.T) {
	keep := fsutil.FileMeta{Path: "/a", Dev: 1}
	dup := fsutil.FileMeta{Path: "/b", Dev: 2}
	assert.ErrorIs(t, Link{}.Remove(dup, keep), ErrCrossDevice)
}

func TestForMode(t *testing.T) {
	for mode, want := range map[string]string{"trash": "trash", "permanent": "delete", "link": "link"} {
		r, err := ForMode(mode)
		require.NoError(t, err)
		assert.Equal(t, want, r.Action())
	}
	_, err := ForMode("shred")
	assert.Error(t, err)
}

// TestTrash_MovesToUserTrash touches the real ~/.Trash, so it only runs
// when TWINS_TEST_TRASH=1 is set.
func TestTrash_MovesToUserTrash(t *testing.T) {
	if os.Getenv("TWINS_TEST_TRASH") != "1" {
		t.Skip("set TWINS_TEST_TRASH=1 to exercise the real Trash")
	}
	home, _ := os.UserHomeDir()
	dir, err := os.MkdirTemp(home, ".twins-test-")
	require.NoError(t, err)
	t.Cleanup(func() { _ = os.RemoveAll(dir) })
	keep := write(t, dir, "keep", "xx")
	a := write(t, dir, "twins-trash-test", "xx")

	require.NoError(t, Trash{}.Remove(a, keep))
	_, err = os.Stat(a.Path)
	assert.True(t, os.IsNotExist(err))
	assert.NotEmpty(t, Backend())
}
