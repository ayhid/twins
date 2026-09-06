package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"

	"github.com/ayhid/twins/internal/config"
	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
)

// permanentWord must be typed to confirm irreversible deletion.
const permanentWord = "permanent"

type confirmModel struct {
	deps  Deps
	plan  []group.Action
	files int
	bytes int64
	input textinput.Model
}

func newConfirm(deps Deps, plan []group.Action) confirmModel {
	n := 0
	for _, a := range plan {
		n += len(a.Remove)
	}
	ti := textinput.New()
	ti.Prompt = "> "
	ti.CharLimit = 16
	return confirmModel{deps: deps, plan: plan, files: n, bytes: group.TotalReclaimable(plan), input: ti}
}

func (m confirmModel) needsWord() bool {
	return m.deps.DeleteMode == config.ModePermanent && !m.deps.DryRun
}

// init focuses the confirmation input when a typed word is required.
func (m confirmModel) init() (confirmModel, tea.Cmd) {
	if m.needsWord() {
		cmd := m.input.Focus()
		return m, cmd
	}
	return m, nil
}

func (m confirmModel) update(msg tea.Msg) (confirmModel, tea.Cmd) {
	key, ok := msg.(tea.KeyMsg)
	if !ok {
		return m, nil
	}
	if key.String() == "esc" {
		return m, func() tea.Msg { return backMsg{} }
	}
	if m.needsWord() {
		if key.String() == "enter" {
			if strings.TrimSpace(m.input.Value()) == permanentWord {
				return m, func() tea.Msg { return confirmedMsg{} }
			}
			return m, func() tea.Msg { return backMsg{} }
		}
		var cmd tea.Cmd
		m.input, cmd = m.input.Update(key)
		return m, cmd
	}
	switch key.String() {
	case "y", "Y", "enter":
		return m, func() tea.Msg { return confirmedMsg{} }
	case "n", "N", "q":
		return m, func() tea.Msg { return backMsg{} }
	}
	return m, nil
}

func (m confirmModel) view(width int) string {
	var b strings.Builder
	b.WriteString(styleTitle.Render(m.headline()) + "\n\n")
	b.WriteString(fmt.Sprintf("%d files in %d groups, %s to reclaim\n", m.files, len(m.plan), fsutil.HumanSize(m.bytes)))
	b.WriteString(styleMuted.Render(m.destination()) + "\n\n")
	if m.needsWord() {
		b.WriteString(styleWarn.Render("This cannot be undone.") + "\n")
		b.WriteString(fmt.Sprintf("Type %q and press enter to continue, esc to go back\n", permanentWord))
		b.WriteString(m.input.View() + "\n")
	} else {
		b.WriteString(helpLine("y/enter", "confirm", "n/esc", "back"))
	}
	return styleBox.MaxWidth(width).Render(b.String())
}

func (m confirmModel) headline() string {
	if m.deps.DryRun {
		return "Dry run"
	}
	switch m.deps.DeleteMode {
	case config.ModePermanent:
		return "Delete permanently?"
	case config.ModeLink:
		return "Replace with clones?"
	}
	return "Move to Trash?"
}

func (m confirmModel) destination() string {
	if m.deps.DryRun {
		return "Nothing will be modified; intended actions are journaled."
	}
	switch m.deps.DeleteMode {
	case config.ModePermanent:
		return "Files are unlinked immediately."
	case config.ModeLink:
		return "Each duplicate becomes an APFS clone of the kept file: same path, shared blocks."
	}
	return "Files go to the Trash; use Finder's Put Back to undo."
}
