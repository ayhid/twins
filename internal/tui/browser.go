package tui

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"

	"github.com/ayhid/twins/internal/fsutil"
)

// rowKind distinguishes group headers from file lines.
type rowKind int

const (
	rowGroup rowKind = iota
	rowFile
)

// row is one displayable line of the browser.
type row struct {
	kind rowKind
	gi   int // group index
	fi   int // file index within the group (rowFile only)
}

// browserModel lists groups, lets the user expand them and mark files.
type browserModel struct {
	deps      Deps
	result    ScanResult
	selection Selection
	expanded  map[int]bool
	rows      []row
	cursor    int
	offset    int
	width     int
	height    int
	filter    textinput.Model
	filtering bool
	flash     string
	showHelp  bool
}

func newBrowser(deps Deps, result ScanResult) browserModel {
	ti := textinput.New()
	ti.Prompt = "/ "
	ti.Placeholder = "filter paths"
	m := browserModel{
		deps:      deps,
		result:    result,
		selection: NewSelection(result.Groups, deps.Keeper),
		expanded:  map[int]bool{},
		filter:    ti,
		width:     80,
		height:    24,
	}
	if len(result.Groups) > 0 {
		m.expanded[0] = true
	}
	return m.rebuild()
}

func (m browserModel) resize(width, height int) browserModel {
	m.width, m.height = width, height
	return m.clamp()
}

// rebuild recomputes the visible rows from the filter and expansion state.
func (m browserModel) rebuild() browserModel {
	needle := strings.ToLower(strings.TrimSpace(m.filter.Value()))
	var rows []row
	for gi, g := range m.selection.Groups() {
		if needle != "" && !groupMatches(g.Files, needle) {
			continue
		}
		rows = append(rows, row{kind: rowGroup, gi: gi})
		if m.expanded[gi] {
			for fi := range g.Files {
				rows = append(rows, row{kind: rowFile, gi: gi, fi: fi})
			}
		}
	}
	m.rows = rows
	return m.clamp()
}

func groupMatches(files []fsutil.FileMeta, needle string) bool {
	for _, f := range files {
		if strings.Contains(strings.ToLower(f.Path), needle) {
			return true
		}
	}
	return false
}

func (m browserModel) clamp() browserModel {
	if len(m.rows) == 0 {
		m.cursor, m.offset = 0, 0
		return m
	}
	m.cursor = max(0, min(m.cursor, len(m.rows)-1))
	visible := m.listHeight()
	if m.cursor < m.offset {
		m.offset = m.cursor
	}
	if m.cursor >= m.offset+visible {
		m.offset = m.cursor - visible + 1
	}
	m.offset = max(0, m.offset)
	return m
}

// listHeight is the number of rows that fit between header and footer.
func (m browserModel) listHeight() int {
	reserved := 4 // title, blank, help, flash
	if m.showHelp {
		reserved += 3
	}
	return max(1, m.height-reserved)
}

func (m browserModel) update(msg tea.Msg) (browserModel, tea.Cmd) {
	key, ok := msg.(tea.KeyMsg)
	if !ok {
		return m, nil
	}
	m.flash = ""
	if m.filtering {
		return m.updateFilter(key)
	}
	switch key.String() {
	case "q", "esc":
		return m, tea.Quit
	case "up", "k":
		m.cursor--
	case "down", "j":
		m.cursor++
	case "pgup":
		m.cursor -= m.listHeight()
	case "pgdown":
		m.cursor += m.listHeight()
	case "g", "home":
		m.cursor = 0
	case "G", "end":
		m.cursor = len(m.rows) - 1
	case "enter", "right", "left", "l", "h":
		return m.toggleExpand(), nil
	case " ":
		return m.toggleMark(), nil
	case "a":
		m.selection = m.selection.AutoAll()
	case "u":
		m.selection = m.selection.ClearAll()
	case "o":
		return m, m.external(m.deps.Reveal)
	case "p":
		return m, m.external(m.deps.Preview)
	case "/":
		m.filtering = true
		return m, m.filter.Focus()
	case "?":
		m.showHelp = !m.showHelp
	case "x":
		return m.proceed()
	}
	return m.clamp(), nil
}

func (m browserModel) updateFilter(key tea.KeyMsg) (browserModel, tea.Cmd) {
	switch key.String() {
	case "enter":
		m.filtering = false
		m.filter.Blur()
		return m.rebuild(), nil
	case "esc":
		m.filtering = false
		m.filter.Blur()
		m.filter.SetValue("")
		return m.rebuild(), nil
	}
	var cmd tea.Cmd
	m.filter, cmd = m.filter.Update(key)
	return m.rebuild(), cmd
}

func (m browserModel) current() (row, bool) {
	if len(m.rows) == 0 {
		return row{}, false
	}
	return m.rows[m.cursor], true
}

func (m browserModel) toggleExpand() browserModel {
	r, ok := m.current()
	if !ok {
		return m
	}
	expanded := make(map[int]bool, len(m.expanded)+1)
	for k, v := range m.expanded {
		expanded[k] = v
	}
	expanded[r.gi] = !expanded[r.gi]
	m.expanded = expanded
	if r.kind == rowFile { // collapsing from inside: land on the header
		for i := m.cursor; i >= 0; i-- {
			if m.rows[i].kind == rowGroup && m.rows[i].gi == r.gi {
				m.cursor = i
				break
			}
		}
	}
	return m.rebuild()
}

func (m browserModel) toggleMark() browserModel {
	r, ok := m.current()
	if !ok {
		return m
	}
	if r.kind == rowGroup {
		m.selection = m.selection.ToggleGroup(r.gi)
		return m
	}
	path := m.selection.Groups()[r.gi].Files[r.fi].Path
	next, err := m.selection.Toggle(r.gi, path)
	if err != nil {
		m.flash = err.Error()
		return m
	}
	m.selection = next
	return m
}

func (m browserModel) external(fn func(string) error) tea.Cmd {
	r, ok := m.current()
	if !ok || fn == nil {
		return nil
	}
	path := m.pathAt(r)
	return func() tea.Msg {
		_ = fn(path)
		return nil
	}
}

func (m browserModel) pathAt(r row) string {
	g := m.selection.Groups()[r.gi]
	if r.kind == rowFile {
		return g.Files[r.fi].Path
	}
	return m.selection.Keep(r.gi).Path
}

func (m browserModel) proceed() (browserModel, tea.Cmd) {
	if m.deps.Mode != ModeClean {
		m.flash = "browse only: run `twins clean` to remove files"
		return m, nil
	}
	if m.selection.MarkedCount() == 0 {
		m.flash = "nothing marked: press space to mark files, a to auto-select"
		return m, nil
	}
	return m, func() tea.Msg { return proceedMsg{} }
}

// View.

func (m browserModel) view() string {
	var b strings.Builder
	b.WriteString(m.title() + "\n")
	if m.filtering || m.filter.Value() != "" {
		b.WriteString(m.filter.View() + "\n")
	} else {
		b.WriteString("\n")
	}
	end := min(len(m.rows), m.offset+m.listHeight())
	for i := m.offset; i < end; i++ {
		b.WriteString(m.renderRow(m.rows[i], i == m.cursor) + "\n")
	}
	if len(m.rows) == 0 {
		b.WriteString(styleMuted.Render("  No duplicates found.") + "\n")
	}
	b.WriteString(m.footer())
	return b.String()
}

func (m browserModel) title() string {
	groups := m.selection.Groups()
	reclaimable := fsutil.HumanSize(m.selection.Reclaimable())
	return styleTitle.Render("twins") + styleMuted.Render(fmt.Sprintf(
		"  %d groups · %d files marked · %s to reclaim · %d files scanned",
		len(groups), m.selection.MarkedCount(), reclaimable, m.result.Files))
}

func (m browserModel) renderRow(r row, selected bool) string {
	var line string
	if r.kind == rowGroup {
		line = m.renderGroup(r.gi)
	} else {
		line = m.renderFile(r.gi, r.fi)
	}
	line = truncate(line, m.width)
	if selected {
		return styleCursor.Render(line)
	}
	return line
}

func (m browserModel) renderGroup(gi int) string {
	g := m.selection.Groups()[gi]
	arrow := "▸"
	if m.expanded[gi] {
		arrow = "▾"
	}
	name := filepath.Base(m.selection.Keep(gi).Path)
	return fmt.Sprintf(" %s %9s × %-3d %-40s %s  %s", arrow, fsutil.HumanSize(g.Size), len(g.Files),
		truncate(name, 40), fsutil.HumanSize(g.Reclaimable()),
		styleMuted.Render(fmt.Sprintf("[%d/%d marked]", m.selection.GroupMarked(gi), len(g.Files))))
}

func (m browserModel) renderFile(gi, fi int) string {
	f := m.selection.Groups()[gi].Files[fi]
	path := shorten(f.Path, m.deps.Home)
	switch {
	case m.selection.IsMarked(gi, f.Path):
		return "      " + styleRemove.Render("✗ "+path)
	case m.selection.Keep(gi).Path == f.Path:
		return "      " + styleKeep.Render("★ "+path)
	default:
		return "      " + "· " + path
	}
}

func (m browserModel) footer() string {
	var b strings.Builder
	if m.flash != "" {
		b.WriteString(styleWarn.Render(m.flash))
	}
	b.WriteString("\n")
	keys := []string{"space", "mark", "enter", "expand", "a", "auto", "u", "unmark all", "/", "filter"}
	if m.deps.Mode == ModeClean {
		keys = append(keys, "x", "remove marked")
	}
	keys = append(keys, "?", "help", "q", "quit")
	b.WriteString(helpLine(keys...))
	if m.showHelp {
		b.WriteString("\n" + helpLine("↑/↓ j/k", "move", "pgup/pgdn", "page", "g/G", "top/bottom", "o", "reveal in Finder", "p", "Quick Look"))
		b.WriteString("\n" + styleMuted.Render("★ kept   ✗ marked for removal   · kept too (unmarked)"))
	}
	return b.String()
}
