package tui

import (
	"context"
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/progress"
	tea "github.com/charmbracelet/bubbletea"

	"github.com/ayhid/twins/internal/config"
	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
	"github.com/ayhid/twins/internal/trash"
)

type execProgressMsg struct{ done, total int }

type execDoneMsg struct {
	result trash.Result
	err    error
}

type execModel struct {
	deps   Deps
	ctx    context.Context
	plan   []group.Action
	bar    progress.Model
	done   int
	total  int
	msgs   chan tea.Msg
	result trash.Result
}

func newExec(ctx context.Context, deps Deps, plan []group.Action) execModel {
	total := 0
	for _, a := range plan {
		total += len(a.Remove)
	}
	bar := progress.New(progress.WithDefaultGradient(), progress.WithoutPercentage())
	return execModel{deps: deps, ctx: ctx, plan: plan, bar: bar, total: total, msgs: make(chan tea.Msg, 256)}
}

func (m execModel) start() tea.Cmd {
	msgs, ctx, deps, plan := m.msgs, m.ctx, m.deps, m.plan
	go func() {
		result, err := deps.Execute(ctx, plan, func(done, total int) {
			select {
			case msgs <- execProgressMsg{done, total}:
			default:
			}
		})
		msgs <- execDoneMsg{result: result, err: err}
	}()
	return pump(msgs)
}

func (m execModel) update(msg tea.Msg) (execModel, tea.Cmd) {
	switch msg := msg.(type) {
	case execProgressMsg:
		m.done, m.total = msg.done, msg.total
		return m, pump(m.msgs)
	case execDoneMsg:
		m.result = msg.result
		m.done = m.total
		return m, nil
	case progress.FrameMsg:
		bar, cmd := m.bar.Update(msg)
		m.bar = bar.(progress.Model)
		return m, cmd
	}
	return m, nil
}

func (m execModel) percent() float64 {
	if m.total == 0 {
		return 1
	}
	return float64(m.done) / float64(m.total)
}

func (m execModel) view(width int) string {
	m.bar.Width = max(20, min(60, width-10))
	return fmt.Sprintf("\n  %s\n\n  %s  %d/%d\n", styleTitle.Render(m.verb()+"…"), m.bar.ViewAs(m.percent()), m.done, m.total)
}

func (m execModel) viewDone(width int) string {
	var b strings.Builder
	r := m.result
	if r.DryRun {
		b.WriteString(styleTitle.Render("Dry run complete") + "\n\n")
		b.WriteString(fmt.Sprintf("Would %s %d files and free %s.\n", m.infinitive(), r.Removed, fsutil.HumanSize(r.Bytes)))
	} else {
		b.WriteString(styleTitle.Render("Done") + "\n\n")
		b.WriteString(fmt.Sprintf("%s: %d files, %s freed.\n", m.verb(), r.Removed, fsutil.HumanSize(r.Bytes)))
	}
	if len(r.Failures) > 0 {
		b.WriteString("\n" + styleWarn.Render(fmt.Sprintf("%d files could not be removed:", len(r.Failures))) + "\n")
		for i, f := range r.Failures {
			if i == 10 {
				b.WriteString(styleMuted.Render(fmt.Sprintf("  … and %d more (see the journal)", len(r.Failures)-10)) + "\n")
				break
			}
			b.WriteString(fmt.Sprintf("  %s: %v\n", shorten(f.Path, m.deps.Home), f.Err))
		}
	}
	b.WriteString("\n" + styleMuted.Render("press any key to exit"))
	return styleBox.MaxWidth(width).Render(b.String())
}

// infinitive is the action as a verb phrase for the dry-run summary.
func (m execModel) infinitive() string {
	switch m.deps.DeleteMode {
	case config.ModePermanent:
		return "delete"
	case config.ModeLink:
		return "replace with clones"
	}
	return "move to Trash"
}

func (m execModel) verb() string {
	switch m.deps.DeleteMode {
	case config.ModePermanent:
		return "Deleted"
	case config.ModeLink:
		return "Replaced with clones"
	}
	return "Moved to Trash"
}
