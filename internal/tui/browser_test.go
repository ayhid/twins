package tui

import (
	"errors"
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/ayhid/twins/internal/trash"
)

func key(s string) tea.KeyMsg {
	switch s {
	case "enter":
		return tea.KeyMsg{Type: tea.KeyEnter}
	case "esc":
		return tea.KeyMsg{Type: tea.KeyEsc}
	case "up":
		return tea.KeyMsg{Type: tea.KeyUp}
	case "down":
		return tea.KeyMsg{Type: tea.KeyDown}
	case " ":
		return tea.KeyMsg{Type: tea.KeySpace}
	}
	return tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune(s)}
}

func testBrowser(mode Mode) browserModel {
	deps := Deps{Mode: mode, Keeper: oldest, Home: "/"}
	return newBrowser(deps, ScanResult{Files: 10, Groups: twoGroups()}).resize(100, 30)
}

func press(m browserModel, keys ...string) (browserModel, tea.Cmd) {
	var cmd tea.Cmd
	for _, k := range keys {
		m, cmd = m.update(key(k))
	}
	return m, cmd
}

func TestBrowser_FirstGroupExpandedByDefault(t *testing.T) {
	m := testBrowser(ModeScan)
	require.Len(t, m.rows, 5, "group1 + 3 files + group2")
	assert.Equal(t, rowGroup, m.rows[0].kind)
	assert.Equal(t, rowFile, m.rows[1].kind)
	assert.Equal(t, rowGroup, m.rows[4].kind)
}

func TestBrowser_NavigationClampsAndWraps(t *testing.T) {
	m := testBrowser(ModeScan)
	m, _ = press(m, "k")
	assert.Equal(t, 0, m.cursor)
	m, _ = press(m, "j", "j", "j", "j", "j", "j")
	assert.Equal(t, 4, m.cursor)
	m, _ = press(m, "g")
	assert.Equal(t, 0, m.cursor)
	m, _ = press(m, "G")
	assert.Equal(t, 4, m.cursor)
}

func TestBrowser_EnterTogglesExpansion(t *testing.T) {
	m := testBrowser(ModeScan)
	m, _ = press(m, "enter")
	assert.Len(t, m.rows, 2)
	m, _ = press(m, "j", "enter")
	assert.Len(t, m.rows, 4)
	m, _ = press(m, "j", "enter") // collapse from inside lands on header
	assert.Len(t, m.rows, 2)
	assert.Equal(t, 1, m.cursor)
}

func TestBrowser_SpaceTogglesFileAndRefusesLastCopy(t *testing.T) {
	m := testBrowser(ModeScan)
	m, _ = press(m, "j", "j") // /a2, marked
	assert.True(t, m.selection.IsMarked(0, "/a2"))
	m, _ = press(m, " ")
	assert.False(t, m.selection.IsMarked(0, "/a2"))

	m, _ = press(m, "j", " ", "k", " ") // unmark a3, then mark a2 again -> ok
	assert.True(t, m.selection.IsMarked(0, "/a2"))
	m, _ = press(m, "k", " ") // mark a1: a3 is the only unmarked -> allowed (a3 remains)
	assert.True(t, m.selection.IsMarked(0, "/a1"))
	m, _ = press(m, "j", "j", " ") // mark a3 -> refused
	assert.False(t, m.selection.IsMarked(0, "/a3"))
	assert.Equal(t, ErrLastCopy.Error(), m.flash)
}

func TestBrowser_SpaceOnHeaderTogglesWholeGroup(t *testing.T) {
	m := testBrowser(ModeScan)
	m, _ = press(m, " ")
	assert.Equal(t, 0, m.selection.GroupMarked(0))
	m, _ = press(m, " ")
	assert.Equal(t, 2, m.selection.GroupMarked(0))
}

func TestBrowser_AutoAndUnmarkAll(t *testing.T) {
	m := testBrowser(ModeScan)
	m, _ = press(m, "u")
	assert.Equal(t, 0, m.selection.MarkedCount())
	m, _ = press(m, "a")
	assert.Equal(t, 3, m.selection.MarkedCount())
}

func TestBrowser_FilterNarrowsGroups(t *testing.T) {
	m := testBrowser(ModeScan)
	m, _ = press(m, "/", "b", "2", "enter")
	require.Len(t, m.rows, 1)
	assert.Equal(t, 1, m.rows[0].gi)
	m, _ = press(m, "/", "esc")
	assert.Len(t, m.rows, 5, "esc clears the filter")
}

func TestBrowser_ProceedOnlyInCleanMode(t *testing.T) {
	m := testBrowser(ModeScan)
	m, cmd := press(m, "x")
	assert.Nil(t, cmd)
	assert.Contains(t, m.flash, "browse only")

	m = testBrowser(ModeClean)
	m, cmd = press(m, "x")
	require.NotNil(t, cmd)
	assert.IsType(t, proceedMsg{}, cmd())

	m, _ = press(m, "u")
	m, cmd = press(m, "x")
	assert.Nil(t, cmd)
	assert.Contains(t, m.flash, "nothing marked")
}

func TestBrowser_ViewShowsMarkersAndTotals(t *testing.T) {
	m := testBrowser(ModeClean)
	v := m.view()
	assert.Contains(t, v, "2 groups")
	assert.Contains(t, v, "3 files marked")
	assert.Contains(t, v, "300 B to reclaim")
	assert.Contains(t, v, "★ /a1")
	assert.Contains(t, v, "✗ /a2")
	assert.Contains(t, v, "remove marked")
	m, _ = press(m, "?")
	assert.Contains(t, m.view(), "Quick Look")
}

func TestBrowser_RevealAndPreviewUseDeps(t *testing.T) {
	var revealed, previewed string
	deps := Deps{Mode: ModeScan, Keeper: oldest,
		Reveal:  func(p string) error { revealed = p; return nil },
		Preview: func(p string) error { previewed = p; return nil },
	}
	m := newBrowser(deps, ScanResult{Groups: twoGroups()})
	_, cmd := press(m, "o")
	require.NotNil(t, cmd)
	cmd()
	assert.Equal(t, "/a1", revealed, "header row reveals the kept file")
	_, cmd = press(m, "j", "j", "p")
	cmd()
	assert.Equal(t, "/a2", previewed)
}

func TestBrowser_EmptyResult(t *testing.T) {
	m := newBrowser(Deps{Keeper: oldest}, ScanResult{})
	assert.Contains(t, m.view(), "No duplicates")
	m, _ = press(m, "j", " ", "enter", "x")
	assert.Empty(t, m.rows)
}

func TestTruncateAndShorten(t *testing.T) {
	assert.Equal(t, "abc", truncate("abc", 5))
	assert.Equal(t, "ab…", truncate("abcdef", 3))
	assert.Equal(t, "…", truncate("abcdef", 1))
	assert.Equal(t, "", truncate("abc", 0))
	assert.Equal(t, "~/x", shorten("/Users/me/x", "/Users/me"))
	assert.Equal(t, "/Users/meow/x", shorten("/Users/meow/x", "/Users/me"))
	assert.True(t, strings.HasPrefix(helpLine("q", "quit"), "\x1b") || strings.Contains(helpLine("q", "quit"), "q"))
}

func TestExec_ViewsUseTheRightVerbs(t *testing.T) {
	d := Deps{DeleteMode: "trash", DryRun: true, Home: "/"}
	m := newExec(nil, d, nil)
	m.result.DryRun, m.result.Removed = true, 3
	assert.Contains(t, m.viewDone(80), "Would move to Trash 3 files")

	d.DryRun = false
	m = newExec(nil, d, nil)
	m.result.Removed = 2
	m.result.Failures = []trash.Failure{{Path: "/x", Err: errBoom}}
	v := m.viewDone(80)
	assert.Contains(t, v, "Moved to Trash: 2 files")
	assert.Contains(t, v, "1 files could not be removed")
	assert.Contains(t, m.view(80), "Moved to Trash…")

	d.DeleteMode = "permanent"
	assert.Contains(t, newExec(nil, d, nil).viewDone(80), "Deleted: 0 files")
	d.DeleteMode = "link"
	assert.Contains(t, newExec(nil, d, nil).viewDone(80), "Replaced with clones: 0 files")
}

var errBoom = errors.New("boom")
