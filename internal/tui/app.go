package tui

import (
	"context"
	"fmt"

	tea "github.com/charmbracelet/bubbletea"
)

type screen int

const (
	screenMenu screen = iota
	screenScanning
	screenBrowser
	screenConfirm
	screenExecuting
	screenDone
)

// model is the root Bubble Tea model routing between screens.
type model struct {
	deps   Deps
	ctx    context.Context
	screen screen
	width  int
	height int

	menu     menuModel
	scanning scanModel
	browser  browserModel
	confirm  confirmModel
	exec     execModel

	summary Summary
	err     error
}

// Run starts the interactive UI and blocks until it exits.
func Run(ctx context.Context, deps Deps) (Summary, error) {
	deps = deps.withDefaults()
	m := newModel(ctx, deps)
	final, err := tea.NewProgram(m, tea.WithContext(ctx), tea.WithAltScreen()).Run()
	if err != nil {
		return Summary{}, err
	}
	fm := final.(model)
	return fm.summary, fm.err
}

func newModel(ctx context.Context, deps Deps) model {
	m := model{deps: deps, ctx: ctx, width: 80, height: 24}
	if deps.StartAtMenu {
		m.screen = screenMenu
		m.menu = newMenu(deps)
		return m
	}
	m.screen = screenScanning
	m.scanning = newScan(ctx, deps, deps.Roots)
	return m
}

func (m model) Init() tea.Cmd {
	if m.screen == screenScanning {
		return m.scanning.start()
	}
	return nil
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width, m.height = msg.Width, msg.Height
		m.browser = m.browser.resize(msg.Width, msg.Height)
		return m, nil
	case tea.KeyMsg:
		if msg.String() == "ctrl+c" {
			m.scanning.cancel()
			return m, tea.Quit
		}
	case fatalMsg:
		m.err = msg.err
		return m, tea.Quit
	}
	return m.route(msg)
}

func (m model) route(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch m.screen {
	case screenMenu:
		return m.updateMenu(msg)
	case screenScanning:
		return m.updateScanning(msg)
	case screenBrowser:
		return m.updateBrowser(msg)
	case screenConfirm:
		return m.updateConfirm(msg)
	case screenExecuting:
		return m.updateExecuting(msg)
	default:
		return m.updateDone(msg)
	}
}

func (m model) View() string {
	switch m.screen {
	case screenMenu:
		return m.menu.view(m.width, m.height)
	case screenScanning:
		return m.scanning.view(m.width)
	case screenBrowser:
		return m.browser.view()
	case screenConfirm:
		return m.confirm.view(m.width)
	case screenExecuting:
		return m.exec.view(m.width)
	default:
		return m.exec.viewDone(m.width)
	}
}

// Screen transitions.

func (m model) updateMenu(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd
	m.menu, cmd = m.menu.update(msg)
	if choice, ok := msg.(menuChoiceMsg); ok {
		m.deps.Mode = choice.mode
		m.deps.Roots = choice.roots
		m.screen = screenScanning
		m.scanning = newScan(m.ctx, m.deps, choice.roots)
		return m, m.scanning.start()
	}
	return m, cmd
}

func (m model) updateScanning(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd
	m.scanning, cmd = m.scanning.update(msg)
	if done, ok := msg.(scanDoneMsg); ok {
		if done.err != nil {
			m.err = fmt.Errorf("scan: %w", done.err)
			return m, tea.Quit
		}
		m.screen = screenBrowser
		m.browser = newBrowser(m.deps, done.result).resize(m.width, m.height)
		return m, nil
	}
	return m, cmd
}

func (m model) updateBrowser(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd
	m.browser, cmd = m.browser.update(msg)
	if _, ok := msg.(proceedMsg); ok {
		m.screen = screenConfirm
		var init tea.Cmd
		m.confirm, init = newConfirm(m.deps, m.browser.selection.Plan()).init()
		return m, init
	}
	return m, cmd
}

func (m model) updateConfirm(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd
	m.confirm, cmd = m.confirm.update(msg)
	switch msg.(type) {
	case backMsg:
		m.screen = screenBrowser
		return m, nil
	case confirmedMsg:
		m.screen = screenExecuting
		m.exec = newExec(m.ctx, m.deps, m.confirm.plan)
		return m, m.exec.start()
	}
	return m, cmd
}

func (m model) updateExecuting(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd
	m.exec, cmd = m.exec.update(msg)
	if done, ok := msg.(execDoneMsg); ok {
		if done.err != nil {
			m.err = done.err
			return m, tea.Quit
		}
		m.summary = Summary{Executed: true, Result: done.result}
		m.screen = screenDone
		return m, nil
	}
	return m, cmd
}

func (m model) updateDone(msg tea.Msg) (tea.Model, tea.Cmd) {
	if _, ok := msg.(tea.KeyMsg); ok {
		return m, tea.Quit
	}
	return m, nil
}

// Messages shared across screens.

type fatalMsg struct{ err error }
type proceedMsg struct{}
type backMsg struct{}
type confirmedMsg struct{}
