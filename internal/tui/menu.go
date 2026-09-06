package tui

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

type menuItem struct {
	label string
	desc  string
	mode  Mode
	quit  bool
}

var menuItems = []menuItem{
	{label: "Scan", desc: "find duplicates and browse them, nothing is modified", mode: ModeScan},
	{label: "Clean", desc: "find duplicates, choose what to keep, remove the rest", mode: ModeClean},
	{label: "Quit", quit: true},
}

// menuChoiceMsg is emitted when the user starts a scan.
type menuChoiceMsg struct {
	mode  Mode
	roots []string
}

type menuModel struct {
	deps    Deps
	cursor  int
	path    textinput.Model
	editing bool
	err     string
}

func newMenu(deps Deps) menuModel {
	ti := textinput.New()
	ti.Prompt = ""
	ti.CharLimit = 512
	root := deps.Home
	if len(deps.Roots) > 0 {
		root = deps.Roots[0]
	}
	ti.SetValue(root)
	return menuModel{deps: deps, path: ti}
}

func (m menuModel) update(msg tea.Msg) (menuModel, tea.Cmd) {
	key, ok := msg.(tea.KeyMsg)
	if !ok {
		return m, nil
	}
	if m.editing {
		return m.updateEditing(key)
	}
	switch key.String() {
	case "q", "esc":
		return m, tea.Quit
	case "up", "k":
		m.cursor = max(m.cursor-1, 0)
	case "down", "j":
		m.cursor = min(m.cursor+1, len(menuItems)-1)
	case "e", "f":
		m.editing = true
		m.err = ""
		return m, m.path.Focus()
	case "enter":
		return m.choose()
	}
	return m, nil
}

func (m menuModel) updateEditing(key tea.KeyMsg) (menuModel, tea.Cmd) {
	switch key.String() {
	case "enter", "esc":
		m.editing = false
		m.path.Blur()
		return m, nil
	}
	var cmd tea.Cmd
	m.path, cmd = m.path.Update(key)
	return m, cmd
}

func (m menuModel) choose() (menuModel, tea.Cmd) {
	item := menuItems[m.cursor]
	if item.quit {
		return m, tea.Quit
	}
	root := expandHome(strings.TrimSpace(m.path.Value()), m.deps.Home)
	if info, err := os.Stat(root); err != nil || !info.IsDir() {
		m.err = fmt.Sprintf("%s is not a directory", root)
		return m, nil
	}
	choice := menuChoiceMsg{mode: item.mode, roots: []string{root}}
	return m, func() tea.Msg { return choice }
}

func expandHome(path, home string) string {
	if path == "~" {
		return home
	}
	if strings.HasPrefix(path, "~/") {
		return filepath.Join(home, path[2:])
	}
	return path
}

func (m menuModel) view(width, height int) string {
	var b strings.Builder
	b.WriteString(styleTitle.Render("twins") + styleMuted.Render("  find and remove duplicate files") + "\n\n")
	b.WriteString(styleHeader.Render("Folder  ") + m.path.View())
	if !m.editing {
		b.WriteString(styleMuted.Render("   (e to edit)"))
	}
	b.WriteString("\n\n")
	for i, item := range menuItems {
		cursor := "  "
		label := item.label
		if i == m.cursor && !m.editing {
			cursor = styleHelpKey.Render("▸ ")
			label = styleCursor.Render(" " + item.label + " ")
		} else {
			label = " " + label + " "
		}
		b.WriteString(cursor + label)
		if item.desc != "" {
			b.WriteString("  " + styleMuted.Render(item.desc))
		}
		b.WriteString("\n")
	}
	if m.err != "" {
		b.WriteString("\n" + styleWarn.Render(m.err) + "\n")
	}
	b.WriteString("\n" + helpLine("↑/↓", "move", "enter", "select", "e", "edit folder", "q", "quit"))
	return lipgloss.NewStyle().Padding(1, 2).MaxWidth(width).MaxHeight(height).Render(b.String())
}
