package tui

import (
	"context"
	"fmt"
	"strings"
	"sync/atomic"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
)

type scanProgressMsg Progress

type scanDoneMsg struct {
	result ScanResult
	err    error
}

// scanModel runs the scan in the background and shows its progress.
type scanModel struct {
	deps     Deps
	roots    []string
	spinner  spinner.Model
	progress Progress
	msgs     chan tea.Msg
	cancelFn context.CancelFunc
	ctx      context.Context
}

func newScan(parent context.Context, deps Deps, roots []string) scanModel {
	ctx, cancel := context.WithCancel(parent)
	sp := spinner.New(spinner.WithSpinner(spinner.Dot))
	sp.Style = styleHelpKey
	return scanModel{deps: deps, roots: roots, spinner: sp, msgs: make(chan tea.Msg, 256), ctx: ctx, cancelFn: cancel}
}

func (m scanModel) cancel() {
	if m.cancelFn != nil {
		m.cancelFn()
	}
}

// start launches the scan goroutine and the message pump.
func (m scanModel) start() tea.Cmd {
	msgs, ctx, deps, roots := m.msgs, m.ctx, m.deps, m.roots
	go func() {
		var last atomic.Int64
		result, err := deps.Scan(ctx, roots, func(p Progress) {
			// Drop intermediate events when the UI lags: only the latest matters.
			if p.Phase != "walk" || p.Done-last.Load() >= 256 {
				last.Store(p.Done)
				select {
				case msgs <- scanProgressMsg(p):
				default:
				}
			}
		})
		msgs <- scanDoneMsg{result: result, err: err}
	}()
	return tea.Batch(m.spinner.Tick, pump(msgs))
}

// pump waits for the next background message.
func pump(msgs chan tea.Msg) tea.Cmd {
	return func() tea.Msg { return <-msgs }
}

func (m scanModel) update(msg tea.Msg) (scanModel, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		if s := msg.String(); s == "q" || s == "esc" {
			m.cancel()
			return m, tea.Quit
		}
	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return m, cmd
	case scanProgressMsg:
		m.progress = Progress(msg)
		return m, pump(m.msgs)
	case scanDoneMsg:
		return m, nil
	}
	return m, nil
}

func (m scanModel) view(width int) string {
	var b strings.Builder
	b.WriteString("\n  " + styleTitle.Render("twins") + styleMuted.Render("  scanning "+strings.Join(shortenAll(m.roots, m.deps.Home), ", ")) + "\n\n")
	b.WriteString("  " + m.spinner.View() + " " + phaseLine(m.progress) + "\n\n")
	b.WriteString("  " + helpLine("q", "cancel"))
	return truncateLines(b.String(), width)
}

func phaseLine(p Progress) string {
	switch {
	case p.Phase == "":
		return "starting…"
	case p.Total > 0:
		return fmt.Sprintf("%s  %d/%d", p.Phase, p.Done, p.Total)
	default:
		return fmt.Sprintf("walking  %d candidate files", p.Done)
	}
}

func shortenAll(paths []string, home string) []string {
	out := make([]string, 0, len(paths))
	for _, p := range paths {
		out = append(out, shorten(p, home))
	}
	return out
}

func truncateLines(s string, width int) string {
	lines := strings.Split(s, "\n")
	for i, l := range lines {
		lines[i] = truncate(l, width)
	}
	return strings.Join(lines, "\n")
}
