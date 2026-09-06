package group

import (
	"testing"
	"time"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestParseStrategy(t *testing.T) {
	for _, s := range []string{"oldest", "newest", "shortest-path", "in-dir"} {
		got, err := ParseStrategy(s)
		require.NoError(t, err)
		assert.Equal(t, Strategy(s), got)
	}
	_, err := ParseStrategy("random")
	assert.Error(t, err)
}

func TestKeeper_Oldest(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 2048)
	f.file("new", data)
	f.file("old", data)
	f.file("mid", data)
	base := time.Date(2024, 1, 1, 0, 0, 0, 0, time.UTC)
	f.touch("old", base)
	f.touch("mid", base.Add(24*time.Hour))
	f.touch("new", base.Add(48*time.Hour))
	g := groupOf(t, f)

	keep, remove := Keeper{Strategy: KeepOldest}.Choose(g)
	assert.Equal(t, f.path("old"), keep.Path)
	assert.Equal(t, []string{f.path("mid"), f.path("new")}, paths(remove))
}

func TestKeeper_Newest(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 2048)
	f.file("a", data)
	f.file("b", data)
	base := time.Date(2024, 1, 1, 0, 0, 0, 0, time.UTC)
	f.touch("a", base)
	f.touch("b", base.Add(time.Hour))
	g := groupOf(t, f)

	keep, remove := Keeper{Strategy: KeepNewest}.Choose(g)
	assert.Equal(t, f.path("b"), keep.Path)
	assert.Equal(t, []string{f.path("a")}, paths(remove))
}

func TestKeeper_ShortestPathPrefersFewestComponentsThenShortestName(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 2048)
	f.file("deep/er/x", data)
	f.file("longer-name", data)
	f.file("short", data)
	g := groupOf(t, f)

	keep, remove := Keeper{Strategy: KeepShortestPath}.Choose(g)
	assert.Equal(t, f.path("short"), keep.Path)
	assert.Len(t, remove, 2)
}

func TestKeeper_InDirPrefersFilesUnderDirThenFallsBackToOldest(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 2048)
	f.file("Downloads/x", data)
	f.file("Photos/2024/x", data)
	f.file("Photos/2023/x", data)
	base := time.Date(2024, 1, 1, 0, 0, 0, 0, time.UTC)
	f.touch("Downloads/x", base)
	f.touch("Photos/2024/x", base.Add(2*time.Hour))
	f.touch("Photos/2023/x", base.Add(time.Hour))
	g := groupOf(t, f)

	keep, _ := Keeper{Strategy: KeepInDir, Dir: f.path("Photos")}.Choose(g)
	assert.Equal(t, f.path("Photos/2023/x"), keep.Path)

	keep, _ = Keeper{Strategy: KeepInDir, Dir: f.path("Nowhere")}.Choose(g)
	assert.Equal(t, f.path("Downloads/x"), keep.Path, "falls back to oldest")
}

func TestKeeper_OldestTieBreaksDeterministically(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 2048)
	f.file("b", data)
	f.file("a", data)
	at := time.Date(2024, 1, 1, 0, 0, 0, 0, time.UTC)
	f.touch("a", at)
	f.touch("b", at)
	g := groupOf(t, f)

	keep, _ := Keeper{Strategy: KeepOldest}.Choose(g)
	assert.Equal(t, f.path("a"), keep.Path)
}

func TestKeeper_NeverRemovesHardlinksOfTheKeptFile(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 2048)
	f.file("a", data)
	f.link("a-link", "a")
	f.file("copy", data)
	base := time.Date(2024, 1, 1, 0, 0, 0, 0, time.UTC)
	f.touch("a", base)
	f.touch("copy", base.Add(time.Hour))
	g := groupOf(t, f)

	keep, remove := Keeper{Strategy: KeepOldest}.Choose(g)
	assert.Equal(t, f.path("a"), keep.Path)
	assert.Equal(t, []string{f.path("copy")}, paths(remove))
}

func TestPlan_ReclaimableSumsPhysicalBytes(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 2048)
	f.file("a", data)
	f.file("b", data)
	f.file("c", data)
	g := groupOf(t, f)

	plan := Plan([]Group{g}, Keeper{Strategy: KeepOldest})
	require.Len(t, plan, 1)
	assert.Len(t, plan[0].Remove, 2)
	assert.EqualValues(t, 4096, plan[0].Reclaimable())
	assert.EqualValues(t, 4096, TotalReclaimable(plan))
}

func TestAction_Reclaimable_IgnoresLinksOfKeptFile(t *testing.T) {
	keep := fsutil.FileMeta{Path: "/k", Size: 100, Dev: 1, Inode: 1}
	link := fsutil.FileMeta{Path: "/k2", Size: 100, Dev: 1, Inode: 1}
	other := fsutil.FileMeta{Path: "/o", Size: 100, Dev: 1, Inode: 2}
	otherLink := fsutil.FileMeta{Path: "/o2", Size: 100, Dev: 1, Inode: 2}
	a := Action{
		Group:  Group{Size: 100, Files: []fsutil.FileMeta{keep, link, other, otherLink}},
		Keep:   keep,
		Remove: []fsutil.FileMeta{other, otherLink},
	}
	assert.EqualValues(t, 100, a.Reclaimable())
}

func groupOf(t *testing.T, f *fixture) Group {
	t.Helper()
	groups, err := Find(t.Context(), f.index(), Options{})
	require.NoError(t, err)
	require.Len(t, groups, 1)
	return groups[0]
}
