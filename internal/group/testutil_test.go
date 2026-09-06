package group

import (
	"crypto/rand"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/stretchr/testify/require"
)

func randomBytes(t *testing.T, n int) []byte {
	t.Helper()
	b := make([]byte, n)
	_, err := rand.Read(b)
	require.NoError(t, err)
	return b
}

// fixture builds files under a temp dir. index() stats them lazily so
// that touch() is reflected in the metadata.
type fixture struct {
	t     *testing.T
	root  string
	paths []string
}

func newFixture(t *testing.T) *fixture {
	t.Helper()
	return &fixture{t: t, root: t.TempDir()}
}

func (f *fixture) index() *Index {
	f.t.Helper()
	idx := NewIndex()
	for _, p := range f.paths {
		m, err := fsutil.Stat(p)
		require.NoError(f.t, err)
		idx.Add(m)
	}
	return idx
}

func (f *fixture) file(rel string, data []byte) fsutil.FileMeta {
	f.t.Helper()
	p := filepath.Join(f.root, rel)
	require.NoError(f.t, os.MkdirAll(filepath.Dir(p), 0o755))
	require.NoError(f.t, os.WriteFile(p, data, 0o644))
	return f.add(p)
}

func (f *fixture) link(rel, target string) fsutil.FileMeta {
	f.t.Helper()
	p := filepath.Join(f.root, rel)
	require.NoError(f.t, os.MkdirAll(filepath.Dir(p), 0o755))
	require.NoError(f.t, os.Link(filepath.Join(f.root, target), p))
	return f.add(p)
}

func (f *fixture) touch(rel string, at time.Time) {
	f.t.Helper()
	require.NoError(f.t, os.Chtimes(filepath.Join(f.root, rel), at, at))
}

func (f *fixture) add(p string) fsutil.FileMeta {
	f.t.Helper()
	m, err := fsutil.Stat(p)
	require.NoError(f.t, err)
	f.paths = append(f.paths, p)
	return m
}

func (f *fixture) path(rel string) string { return filepath.Join(f.root, rel) }

func paths(files []fsutil.FileMeta) []string {
	out := make([]string, 0, len(files))
	for _, m := range files {
		out = append(out, m.Path)
	}
	return out
}
