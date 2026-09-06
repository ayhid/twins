package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
)

var (
	colorAccent = lipgloss.Color("#7C3AED")
	colorMuted  = lipgloss.Color("#6B7280")
	colorKeep   = lipgloss.Color("#10B981")
	colorRemove = lipgloss.Color("#EF4444")
	colorWarn   = lipgloss.Color("#F59E0B")

	styleTitle    = lipgloss.NewStyle().Bold(true).Foreground(colorAccent)
	styleMuted    = lipgloss.NewStyle().Foreground(colorMuted)
	styleKeep     = lipgloss.NewStyle().Foreground(colorKeep)
	styleRemove   = lipgloss.NewStyle().Foreground(colorRemove)
	styleWarn     = lipgloss.NewStyle().Foreground(colorWarn).Bold(true)
	styleCursor   = lipgloss.NewStyle().Reverse(true)
	styleHeader   = lipgloss.NewStyle().Bold(true)
	styleHelpKey  = lipgloss.NewStyle().Foreground(colorAccent)
	styleHelpDesc = lipgloss.NewStyle().Foreground(colorMuted)
	styleBox      = lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(colorAccent).Padding(1, 2)
)

// shorten replaces the home prefix with ~ for display.
func shorten(path, home string) string {
	if home != "" && strings.HasPrefix(path, home+"/") {
		return "~" + path[len(home):]
	}
	return path
}

// truncate cuts s to width, adding an ellipsis, without breaking runes.
func truncate(s string, width int) string {
	if width <= 0 {
		return ""
	}
	r := []rune(s)
	if len(r) <= width {
		return s
	}
	if width == 1 {
		return "…"
	}
	return string(r[:width-1]) + "…"
}

// helpLine renders key/description pairs on one line.
func helpLine(pairs ...string) string {
	var b strings.Builder
	for i := 0; i+1 < len(pairs); i += 2 {
		if i > 0 {
			b.WriteString("  ")
		}
		b.WriteString(styleHelpKey.Render(pairs[i]))
		b.WriteString(" ")
		b.WriteString(styleHelpDesc.Render(pairs[i+1]))
	}
	return b.String()
}
