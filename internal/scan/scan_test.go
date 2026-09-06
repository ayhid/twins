package scan

import (
	"context"
	"os"
	"path/filepath"
	"sort"
	"sync"
	"testing"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// tree creates files relative to root. Value is the file content.
func tree(t *testing.T, root string, files map[string]string) {
	t.Helper()
	for rel, content := range files {
		p := filepath.Join(root, rel)
		require.NoError(t, os.MkdirAll(filepath.Dir(p), 0o755))
		require.NoError(t, os.WriteFile(p, []byte(content), 0o644))
	}
}

func collect(t *testing.T, opts Options) ([]string, Stats) {
	t.Helper()
	var mu sync.Mutex
	var paths []string
	stats, err := Walk(context.Background(), opts, func(m fsutil.FileMeta) {
		mu.Lock()
		defer mu.Unlock()
		paths = append(paths, m.Path)
	})
	require.NoError(t, err)
	sort.Strings(paths)
	return paths, stats
}

func rel(root string, paths []string) []string {
	out := make([]string, 0, len(paths))
	for _, p := range paths {
		r, _ := filepath.Rel(root, p)
		out = append(out, r)
	}
	return out
}

func TestWalk_FindsRegularFilesRecursively(t *testing.T) {
	root := t.TempDir()
	tree(t, root, map[string]string{
		"a.txt":       "aaaa",
		"sub/b.txt":   "bbbb",
		"sub/c/d.txt": "dddd",
	})

	paths, stats := collect(t, Options{Roots: []string{root}, MinSize: 1})
	assert.Equal(t, []string{"a.txt", "sub/b.txt", "sub/c/d.txt"}, rel(root, paths))
	assert.EqualValues(t, 3, stats.Files)
	assert.EqualValues(t, 3, stats.Candidates)
}

func TestWalk_AppliesMinSizeAndSkipsEmptyByDefault(t *testing.T) {
	root := t.TempDir()
	tree(t, root, map[string]string{
		"empty":   "",
		"small":   "ab",
		"big.bin": "0123456789",
	})

	paths, stats := collect(t, Options{Roots: []string{root}, MinSize: 5})
	assert.Equal(t, []string{"big.bin"}, rel(root, paths))
	assert.EqualValues(t, 3, stats.Files)
	assert.EqualValues(t, 1, stats.Candidates)

	paths, _ = collect(t, Options{Roots: []string{root}, MinSize: 0, IncludeEmpty: true})
	assert.Equal(t, []string{"big.bin", "empty", "small"}, rel(root, paths))

	paths, _ = collect(t, Options{Roots: []string{root}, MinSize: 0})
	assert.Equal(t, []string{"big.bin", "small"}, rel(root, paths), "empty files need opt-in")
}

func TestWalk_SkipsDefaultExcludedDirs(t *testing.T) {
	root := t.TempDir()
	tree(t, root, map[string]string{
		"keep.txt":                  "keep",
		".git/objects/x":            "git",
		"node_modules/pkg/index.js": "js",
		".Trash/old":                "trash",
		"Library/Caches/blob":       "cache",
		"Backups.backupdb/x":        "tm",
	})

	paths, _ := collect(t, Options{Roots: []string{root}, MinSize: 1})
	assert.Equal(t, []string{"keep.txt"}, rel(root, paths))

	paths, _ = collect(t, Options{Roots: []string{root}, MinSize: 1, IncludeNodeModules: true})
	assert.Equal(t, []string{"keep.txt", "node_modules/pkg/index.js"}, rel(root, paths))
}

func TestWalk_TreatsBundlesAsAtomic(t *testing.T) {
	root := t.TempDir()
	tree(t, root, map[string]string{
		"Foo.app/Contents/MacOS/foo": "binary",
		"doc.txt":                    "doc",
	})

	paths, _ := collect(t, Options{Roots: []string{root}, MinSize: 1})
	assert.Equal(t, []string{"doc.txt"}, rel(root, paths))
}

func TestWalk_UserExcludeGlobs(t *testing.T) {
	root := t.TempDir()
	tree(t, root, map[string]string{
		"a.log":       "log",
		"a.txt":       "txt",
		"build/x.o":   "obj",
		"src/build/y": "nested",
	})

	paths, _ := collect(t, Options{
		Roots:   []string{root},
		MinSize: 1,
		Exclude: []string{"*.log", "build"},
	})
	assert.Equal(t, []string{"a.txt"}, rel(root, paths))
}

func TestWalk_NeverFollowsSymlinks(t *testing.T) {
	root := t.TempDir()
	tree(t, root, map[string]string{"real/a.txt": "aaaa"})
	require.NoError(t, os.Symlink(filepath.Join(root, "real"), filepath.Join(root, "linkdir")))
	require.NoError(t, os.Symlink(filepath.Join(root, "real/a.txt"), filepath.Join(root, "linkfile")))

	paths, _ := collect(t, Options{Roots: []string{root}, MinSize: 1})
	assert.Equal(t, []string{"real/a.txt"}, rel(root, paths))
}

func TestWalk_RejectsProtectedRoots(t *testing.T) {
	_, err := Walk(context.Background(), Options{Roots: []string{"/System"}}, func(fsutil.FileMeta) {})
	assert.ErrorIs(t, err, ErrProtectedRoot)
}

func TestWalk_MissingRootIsAnError(t *testing.T) {
	_, err := Walk(context.Background(), Options{Roots: []string{filepath.Join(t.TempDir(), "nope")}}, func(fsutil.FileMeta) {})
	assert.Error(t, err)
}

func TestWalk_HonoursContextCancellation(t *testing.T) {
	root := t.TempDir()
	files := map[string]string{}
	for i := 0; i < 200; i++ {
		files[filepath.Join("d", string(rune('a'+i%26)), "f"+string(rune('a'+i%26))+string(rune('0'+i%10)))] = "x"
	}
	tree(t, root, files)

	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	_, err := Walk(ctx, Options{Roots: []string{root}, MinSize: 1}, func(fsutil.FileMeta) {})
	assert.ErrorIs(t, err, context.Canceled)
}

func TestWalk_DeduplicatesOverlappingRoots(t *testing.T) {
	root := t.TempDir()
	tree(t, root, map[string]string{"sub/a.txt": "aaaa"})

	paths, _ := collect(t, Options{Roots: []string{root, filepath.Join(root, "sub")}, MinSize: 1})
	assert.Equal(t, []string{"sub/a.txt"}, rel(root, paths))
}
