// Package tui is the interactive Bubble Tea front end.
package tui

import (
	"errors"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
)

// ErrLastCopy is returned when marking a file would leave no copy at all.
var ErrLastCopy = errors.New("at least one copy must be kept")

// Selection is the user's removal choices over a set of groups. It is a
// value type: every mutation returns a new Selection and leaves the
// receiver untouched (only the affected group's set is copied).
type Selection struct {
	groups []group.Group
	keeper group.Keeper
	marked []map[string]bool // per group: path → scheduled for removal
}

// NewSelection applies the keeper to every group.
func NewSelection(groups []group.Group, keeper group.Keeper) Selection {
	s := Selection{groups: groups, keeper: keeper, marked: make([]map[string]bool, len(groups))}
	for i := range groups {
		s.marked[i] = autoMark(groups[i], keeper)
	}
	return s
}

func autoMark(g group.Group, keeper group.Keeper) map[string]bool {
	_, remove := keeper.Choose(g)
	m := make(map[string]bool, len(remove))
	for _, f := range remove {
		m[f.Path] = true
	}
	return m
}

// Groups returns the underlying groups.
func (s Selection) Groups() []group.Group { return s.groups }

// IsMarked reports whether the file is scheduled for removal.
func (s Selection) IsMarked(gi int, path string) bool { return s.marked[gi][path] }

// Keep returns the file that survives in group gi: the keeper's pick among
// unmarked files.
func (s Selection) Keep(gi int) fsutil.FileMeta {
	return s.keeper.Pick(s.unmarked(gi))
}

func (s Selection) unmarked(gi int) []fsutil.FileMeta {
	var out []fsutil.FileMeta
	for _, f := range s.groups[gi].Files {
		if !s.marked[gi][f.Path] {
			out = append(out, f)
		}
	}
	return out
}

// Toggle flips one file. Marking the last unmarked file is refused.
func (s Selection) Toggle(gi int, path string) (Selection, error) {
	if s.marked[gi][path] {
		return s.withGroup(gi, without(s.marked[gi], path)), nil
	}
	if len(s.unmarked(gi)) <= 1 {
		return s, ErrLastCopy
	}
	return s.withGroup(gi, with(s.marked[gi], path)), nil
}

// ToggleGroup clears the group if anything is marked, otherwise re-applies
// the keeper.
func (s Selection) ToggleGroup(gi int) Selection {
	if len(s.marked[gi]) > 0 {
		return s.withGroup(gi, map[string]bool{})
	}
	return s.withGroup(gi, autoMark(s.groups[gi], s.keeper))
}

// AutoAll re-applies the keeper to every group.
func (s Selection) AutoAll() Selection { return NewSelection(s.groups, s.keeper) }

// ClearAll unmarks everything.
func (s Selection) ClearAll() Selection {
	out := Selection{groups: s.groups, keeper: s.keeper, marked: make([]map[string]bool, len(s.groups))}
	for i := range out.marked {
		out.marked[i] = map[string]bool{}
	}
	return out
}

// MarkedCount counts files scheduled for removal.
func (s Selection) MarkedCount() int {
	n := 0
	for _, m := range s.marked {
		n += len(m)
	}
	return n
}

// GroupMarked counts marked files in one group.
func (s Selection) GroupMarked(gi int) int { return len(s.marked[gi]) }

// Plan converts the selection into executable actions, skipping groups
// with nothing marked.
func (s Selection) Plan() []group.Action {
	var plan []group.Action
	for gi, g := range s.groups {
		if len(s.marked[gi]) == 0 {
			continue
		}
		var remove []fsutil.FileMeta
		for _, f := range g.Files {
			if s.marked[gi][f.Path] {
				remove = append(remove, f)
			}
		}
		plan = append(plan, group.Action{Group: g, Keep: s.Keep(gi), Remove: remove})
	}
	return plan
}

// Reclaimable is the space the current plan would free.
func (s Selection) Reclaimable() int64 { return group.TotalReclaimable(s.Plan()) }

func (s Selection) withGroup(gi int, m map[string]bool) Selection {
	marked := append([]map[string]bool(nil), s.marked...)
	marked[gi] = m
	return Selection{groups: s.groups, keeper: s.keeper, marked: marked}
}

func with(m map[string]bool, path string) map[string]bool {
	out := make(map[string]bool, len(m)+1)
	for k, v := range m {
		out[k] = v
	}
	out[path] = true
	return out
}

func without(m map[string]bool, path string) map[string]bool {
	out := make(map[string]bool, len(m))
	for k, v := range m {
		if k != path {
			out[k] = v
		}
	}
	return out
}
