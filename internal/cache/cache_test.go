package cache

import (
	"os"
	"path/filepath"
	"sync/atomic"
	"testing"
	"time"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/hash"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

type countingHasher struct{ partial, full atomic.Int64 }

func (h *countingHasher) Partial(m fsutil.FileMeta) (uint64, error) {
	h.partial.Add(1)
	return uint64(m.Inode * 7), nil
}

func (h *countingHasher) Full(m fsutil.FileMeta) (hash.Digest, error) {
	h.full.Add(1)
	return hash.Digest{byte(m.Inode)}, nil
}

func openTemp(t *testing.T) (*Store, *countingHasher) {
	t.Helper()
	inner := &countingHasher{}
	s, err := Open(filepath.Join(t.TempDir(), "cache.db"), inner)
	require.NoError(t, err)
	t.Cleanup(func() { _ = s.Close() })
	return s, inner
}

func meta(inode uint64, mtime time.Time) fsutil.FileMeta {
	return fsutil.FileMeta{Path: "/p", Size: 100, Dev: 1, Inode: inode, ModTime: mtime}
}

func TestStore_CachesPartialAndFull(t *testing.T) {
	s, inner := openTemp(t)
	m := meta(1, time.Unix(1000, 0))

	p1, err := s.Partial(m)
	require.NoError(t, err)
	p2, err := s.Partial(m)
	require.NoError(t, err)
	assert.Equal(t, p1, p2)
	assert.EqualValues(t, 1, inner.partial.Load())

	f1, err := s.Full(m)
	require.NoError(t, err)
	f2, err := s.Full(m)
	require.NoError(t, err)
	assert.Equal(t, f1, f2)
	assert.EqualValues(t, 1, inner.full.Load())

	assert.EqualValues(t, 2, s.Stats().Hits)
	assert.EqualValues(t, 2, s.Stats().Misses)
}

func TestStore_InvalidatesWhenMtimeOrSizeChanges(t *testing.T) {
	s, inner := openTemp(t)
	m := meta(1, time.Unix(1000, 0))
	_, err := s.Full(m)
	require.NoError(t, err)

	changed := m
	changed.ModTime = time.Unix(2000, 0)
	_, err = s.Full(changed)
	require.NoError(t, err)

	bigger := m
	bigger.Size = 200
	_, err = s.Full(bigger)
	require.NoError(t, err)

	assert.EqualValues(t, 3, inner.full.Load())
}

func TestStore_PersistsAcrossReopen(t *testing.T) {
	path := filepath.Join(t.TempDir(), "cache.db")
	inner := &countingHasher{}
	m := meta(5, time.Unix(1000, 0))

	s, err := Open(path, inner)
	require.NoError(t, err)
	_, err = s.Full(m)
	require.NoError(t, err)
	require.NoError(t, s.Close())

	s, err = Open(path, inner)
	require.NoError(t, err)
	defer s.Close()
	_, err = s.Full(m)
	require.NoError(t, err)
	assert.EqualValues(t, 1, inner.full.Load())
}

func TestStore_ClearForgetsEverything(t *testing.T) {
	s, inner := openTemp(t)
	m := meta(1, time.Unix(1000, 0))
	_, _ = s.Full(m)
	require.NoError(t, s.Clear())
	_, _ = s.Full(m)
	assert.EqualValues(t, 2, inner.full.Load())
}

func TestStore_IsSafeForConcurrentUse(t *testing.T) {
	s, _ := openTemp(t)
	done := make(chan struct{})
	for w := 0; w < 8; w++ {
		go func(w int) {
			for i := 0; i < 50; i++ {
				_, _ = s.Full(meta(uint64(i), time.Unix(1000, 0)))
				_, _ = s.Partial(meta(uint64(i+w), time.Unix(1000, 0)))
			}
			done <- struct{}{}
		}(w)
	}
	for w := 0; w < 8; w++ {
		<-done
	}
}

func TestOpen_CreatesParentDir(t *testing.T) {
	path := filepath.Join(t.TempDir(), "nested", "dir", "cache.db")
	s, err := Open(path, &countingHasher{})
	require.NoError(t, err)
	require.NoError(t, s.Close())
	_, err = os.Stat(path)
	assert.NoError(t, err)
}

func TestDefaultPath(t *testing.T) {
	p, err := DefaultPath()
	require.NoError(t, err)
	assert.Contains(t, p, filepath.Join("Application Support", "twins", "cache.db"))
}

func TestStore_PendingWritesAreVisibleBeforeFlushAndSurviveIt(t *testing.T) {
	path := filepath.Join(t.TempDir(), "cache.db")
	inner := &countingHasher{}
	s, err := Open(path, inner)
	require.NoError(t, err)
	m := meta(9, time.Unix(1000, 0))

	_, err = s.Full(m)
	require.NoError(t, err)
	_, err = s.Full(m)
	require.NoError(t, err)
	assert.EqualValues(t, 1, inner.full.Load(), "buffered value served before flush")

	require.NoError(t, s.Flush())
	require.NoError(t, s.Close())

	s, err = Open(path, inner)
	require.NoError(t, err)
	defer s.Close()
	_, err = s.Full(m)
	require.NoError(t, err)
	assert.EqualValues(t, 1, inner.full.Load(), "flushed value persisted")
}
