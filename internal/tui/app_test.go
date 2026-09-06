package tui

import (
	"bytes"
	"context"
	"errors"
	"regexp"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/x/exp/teatest"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/ayhid/twins/internal/config"
	"github.com/ayhid/twins/internal/group"
	"github.com/ayhid/twins/internal/trash"
)

var ansi = regexp.MustCompile(`\x1b\[[0-9;]*[A-Za-z]`)

func contains(s string) func([]byte) bool {
	return func(b []byte) bool { return bytes.Contains(ansi.ReplaceAll(b, nil), []byte(s)) }
}

type fakeExec struct{ plans [][]group.Action }

func (f *fakeExec) run(_ context.Context, plan []group.Action, progress func(int, int)) (trash.Result, error) {
	f.plans = append(f.plans, plan)
	n := 0
	for _, a := range plan {
		n += len(a.Remove)
		progress(n, n)
	}
	return trash.Result{Removed: n, Bytes: int64(n) * 100}, nil
}

func fakeScan(groups []group.Group) ScanFunc {
	return func(_ context.Context, _ []string, progress func(Progress)) (ScanResult, error) {
		progress(Progress{Phase: "walk", Done: 512})
		progress(Progress{Phase: "full hash", Done: 1, Total: 2})
		return ScanResult{Files: 42, Groups: groups}, nil
	}
}

func deps(mode Mode, exec *fakeExec) Deps {
	return Deps{
		Mode:       mode,
		Roots:      []string{"/tmp/x"},
		Scan:       fakeScan(twoGroups()),
		Execute:    exec.run,
		Keeper:     oldest,
		DeleteMode: config.ModeTrash,
		Home:       "/",
		Reveal:     func(string) error { return nil },
		Preview:    func(string) error { return nil },
	}
}

func typeKeys(tm *teatest.TestModel, keys ...string) {
	for _, k := range keys {
		tm.Send(key(k))
	}
}

func TestFlow_ScanThenCleanToTrash(t *testing.T) {
	exec := &fakeExec{}
	tm := teatest.NewTestModel(t, newModel(context.Background(), deps(ModeClean, exec)), teatest.WithInitialTermSize(100, 30))

	teatest.WaitFor(t, tm.Output(), contains("2 groups"), teatest.WithDuration(3*time.Second))
	typeKeys(tm, "j", "j", " ") // unmark /a2
	typeKeys(tm, "x")
	teatest.WaitFor(t, tm.Output(), contains("Move to Trash?"), teatest.WithDuration(3*time.Second))
	typeKeys(tm, "y")
	teatest.WaitFor(t, tm.Output(), contains("Moved to Trash: 2 files"), teatest.WithDuration(3*time.Second))
	typeKeys(tm, "q")

	final := tm.FinalModel(t, teatest.WithFinalTimeout(3*time.Second)).(model)
	require.NoError(t, final.err)
	assert.True(t, final.summary.Executed)
	assert.Equal(t, 2, final.summary.Result.Removed)
	require.Len(t, exec.plans, 1)
	plan := exec.plans[0]
	require.Len(t, plan, 2)
	assert.Equal(t, "/a1", plan[0].Keep.Path)
	assert.Len(t, plan[0].Remove, 1)
	assert.Equal(t, "/a3", plan[0].Remove[0].Path)
}

func TestFlow_PermanentRequiresTypedWord(t *testing.T) {
	exec := &fakeExec{}
	d := deps(ModeClean, exec)
	d.DeleteMode = config.ModePermanent
	tm := teatest.NewTestModel(t, newModel(context.Background(), d), teatest.WithInitialTermSize(100, 30))

	teatest.WaitFor(t, tm.Output(), contains("2 groups"), teatest.WithDuration(3*time.Second))
	typeKeys(tm, "x")
	teatest.WaitFor(t, tm.Output(), contains("Delete permanently?"), teatest.WithDuration(3*time.Second))
	typeKeys(tm, "n", "o", "enter") // wrong word: back to browser
	teatest.WaitFor(t, tm.Output(), contains("files marked"), teatest.WithDuration(3*time.Second))
	assert.Empty(t, exec.plans)

	typeKeys(tm, "x")
	teatest.WaitFor(t, tm.Output(), contains("Delete permanently?"), teatest.WithDuration(3*time.Second))
	typeKeys(tm, "p", "e", "r", "m", "a", "n", "e", "n", "t", "enter")
	teatest.WaitFor(t, tm.Output(), contains("Deleted: 3 files"), teatest.WithDuration(3*time.Second))
	typeKeys(tm, "q")
	final := tm.FinalModel(t, teatest.WithFinalTimeout(3*time.Second)).(model)
	assert.True(t, final.summary.Executed)
	require.Len(t, exec.plans, 1)
}

func TestFlow_ScanModeNeverExecutes(t *testing.T) {
	exec := &fakeExec{}
	tm := teatest.NewTestModel(t, newModel(context.Background(), deps(ModeScan, exec)), teatest.WithInitialTermSize(100, 30))
	teatest.WaitFor(t, tm.Output(), contains("2 groups"), teatest.WithDuration(3*time.Second))
	typeKeys(tm, "x")
	teatest.WaitFor(t, tm.Output(), contains("browse only"), teatest.WithDuration(3*time.Second))
	typeKeys(tm, "q")
	final := tm.FinalModel(t, teatest.WithFinalTimeout(3*time.Second)).(model)
	assert.False(t, final.summary.Executed)
	assert.Empty(t, exec.plans)
}

func TestFlow_MenuStartsScan(t *testing.T) {
	exec := &fakeExec{}
	d := deps(ModeScan, exec)
	d.StartAtMenu = true
	d.Home = t.TempDir()
	d.Roots = nil
	tm := teatest.NewTestModel(t, newModel(context.Background(), d), teatest.WithInitialTermSize(100, 30))
	teatest.WaitFor(t, tm.Output(), contains("Folder"), teatest.WithDuration(3*time.Second))
	typeKeys(tm, "j", "enter") // Clean
	teatest.WaitFor(t, tm.Output(), func(b []byte) bool {
		return contains("2 groups")(b) && contains("remove marked")(b)
	}, teatest.WithDuration(3*time.Second))
	typeKeys(tm, "q")
	tm.FinalModel(t, teatest.WithFinalTimeout(3*time.Second))
}

func TestFlow_MenuRejectsMissingFolder(t *testing.T) {
	d := deps(ModeScan, &fakeExec{})
	d.StartAtMenu = true
	d.Roots = []string{"/definitely/not/here"}
	tm := teatest.NewTestModel(t, newModel(context.Background(), d), teatest.WithInitialTermSize(100, 30))
	typeKeys(tm, "enter")
	teatest.WaitFor(t, tm.Output(), contains("is not a directory"), teatest.WithDuration(3*time.Second))
	typeKeys(tm, "q")
	tm.FinalModel(t, teatest.WithFinalTimeout(3*time.Second))
}

func TestFlow_ScanErrorExits(t *testing.T) {
	d := deps(ModeScan, &fakeExec{})
	d.Scan = func(context.Context, []string, func(Progress)) (ScanResult, error) {
		return ScanResult{}, errors.New("boom")
	}
	tm := teatest.NewTestModel(t, newModel(context.Background(), d), teatest.WithInitialTermSize(100, 30))
	final := tm.FinalModel(t, teatest.WithFinalTimeout(3*time.Second)).(model)
	assert.ErrorContains(t, final.err, "boom")
}

func TestFlow_QuitDuringScanCancelsContext(t *testing.T) {
	started := make(chan struct{})
	d := deps(ModeScan, &fakeExec{})
	d.Scan = func(ctx context.Context, _ []string, _ func(Progress)) (ScanResult, error) {
		close(started)
		<-ctx.Done()
		return ScanResult{}, ctx.Err()
	}
	tm := teatest.NewTestModel(t, newModel(context.Background(), d), teatest.WithInitialTermSize(100, 30))
	<-started
	tm.Send(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("q")})
	tm.FinalModel(t, teatest.WithFinalTimeout(3*time.Second))
}

func TestMenu_ExpandHome(t *testing.T) {
	assert.Equal(t, "/home/me", expandHome("~", "/home/me"))
	assert.Equal(t, "/home/me/x", expandHome("~/x", "/home/me"))
	assert.Equal(t, "/abs", expandHome("/abs", "/home/me"))
}
