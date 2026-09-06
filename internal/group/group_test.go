package group

import (
	"context"
	"errors"
	"os"
	"sync"
	"sync/atomic"
	"testing"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/hash"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestFind_GroupsIdenticalFilesSortedByReclaimable(t *testing.T) {
	f := newFixture(t)
	big := randomBytes(t, 64*1024)
	small := randomBytes(t, 10*1024)
	f.file("big1", big)
	f.file("big2", big)
	f.file("small1", small)
	f.file("small2", small)
	f.file("small3", small)
	f.file("unique", randomBytes(t, 64*1024))

	groups, err := Find(context.Background(), f.index(), Options{})
	require.NoError(t, err)
	require.Len(t, groups, 2)

	assert.EqualValues(t, 64*1024, groups[0].Size)
	assert.Equal(t, []string{f.path("big1"), f.path("big2")}, paths(groups[0].Files))
	assert.EqualValues(t, 64*1024, groups[0].Reclaimable())

	assert.EqualValues(t, 10*1024, groups[1].Size)
	assert.Len(t, groups[1].Files, 3)
	assert.EqualValues(t, 20*1024, groups[1].Reclaimable())
	assert.False(t, groups[1].Digest.IsZero())
}

func TestFind_SameSizeDifferentContentIsNotADuplicate(t *testing.T) {
	f := newFixture(t)
	f.file("a", randomBytes(t, 4096))
	f.file("b", randomBytes(t, 4096))

	groups, err := Find(context.Background(), f.index(), Options{})
	require.NoError(t, err)
	assert.Empty(t, groups)
}

func TestFind_PartialCollisionIsResolvedByFullHash(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 100*1024)
	mid := append([]byte{}, data...)
	mid[50*1024] ^= 0xff
	f.file("a", data)
	f.file("b", mid)
	f.file("c", data)

	groups, err := Find(context.Background(), f.index(), Options{})
	require.NoError(t, err)
	require.Len(t, groups, 1)
	assert.Equal(t, []string{f.path("a"), f.path("c")}, paths(groups[0].Files))
}

func TestFind_HardlinksAreNotDuplicatesOfEachOther(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 8192)
	f.file("a", data)
	f.link("a-link", "a")

	groups, err := Find(context.Background(), f.index(), Options{})
	require.NoError(t, err)
	assert.Empty(t, groups, "two names for one inode waste no space")
}

func TestFind_HardlinksCountOncePhysically(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 8192)
	f.file("a", data)
	f.link("a-link", "a")
	f.file("copy", data)

	groups, err := Find(context.Background(), f.index(), Options{})
	require.NoError(t, err)
	require.Len(t, groups, 1)
	assert.Len(t, groups[0].Files, 3)
	assert.Equal(t, 2, groups[0].Physical())
	assert.EqualValues(t, 8192, groups[0].Reclaimable())
}

func TestFind_HashesEachIdentityOnce(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 8192)
	f.file("a", data)
	f.link("a-link", "a")
	f.file("copy", data)

	h := &countingHasher{}
	_, err := Find(context.Background(), f.index(), Options{Hasher: h})
	require.NoError(t, err)
	assert.EqualValues(t, 2, h.partial.Load())
	assert.EqualValues(t, 2, h.full.Load())
}

func TestFind_VerifyDropsFilesThatDoNotMatchByteForByte(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 4096)
	other := randomBytes(t, 4096)
	f.file("a", data)
	f.file("b", data)
	f.file("c", other)

	var errs []string
	groups, err := Find(context.Background(), f.index(), Options{
		Hasher:  collidingHasher{},
		Verify:  true,
		OnError: func(path string, _ error) { errs = append(errs, path) },
	})
	require.NoError(t, err)
	require.Len(t, groups, 1)
	assert.Equal(t, []string{f.path("a"), f.path("b")}, paths(groups[0].Files))
	assert.Equal(t, []string{f.path("c")}, errs)
}

func TestFind_WithoutVerifyTrustsTheHash(t *testing.T) {
	f := newFixture(t)
	f.file("a", randomBytes(t, 4096))
	f.file("b", randomBytes(t, 4096))

	groups, err := Find(context.Background(), f.index(), Options{Hasher: collidingHasher{}})
	require.NoError(t, err)
	require.Len(t, groups, 1)
}

func TestFind_UnreadableFileIsReportedAndDropped(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 4096)
	f.file("a", data)
	f.file("b", data)
	f.file("c", data)
	require.NoError(t, os.Chmod(f.path("c"), 0o000))
	t.Cleanup(func() { _ = os.Chmod(f.path("c"), 0o644) })

	var reported []string
	groups, err := Find(context.Background(), f.index(), Options{
		OnError: func(path string, err error) { reported = append(reported, path) },
	})
	require.NoError(t, err)
	require.Len(t, groups, 1)
	assert.Equal(t, []string{f.path("a"), f.path("b")}, paths(groups[0].Files))
	assert.Equal(t, []string{f.path("c")}, reported)
}

func TestFind_ReportsProgressPerStage(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 4096)
	f.file("a", data)
	f.file("b", data)

	var mu sync.Mutex
	var stages []Stage
	_, err := Find(context.Background(), f.index(), Options{
		OnProgress: func(p Progress) {
			mu.Lock()
			defer mu.Unlock()
			stages = append(stages, p.Stage)
		},
	})
	require.NoError(t, err)
	assert.Contains(t, stages, StagePartial)
	assert.Contains(t, stages, StageFull)
}

func TestFind_HonoursContextCancellation(t *testing.T) {
	f := newFixture(t)
	data := randomBytes(t, 4096)
	for _, n := range []string{"a", "b", "c", "d"} {
		f.file(n, data)
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err := Find(ctx, f.index(), Options{})
	assert.ErrorIs(t, err, context.Canceled)
}

func TestIndex_IsSafeForConcurrentAdd(t *testing.T) {
	idx := NewIndex()
	done := make(chan struct{})
	for i := 0; i < 8; i++ {
		go func(i int) {
			for j := 0; j < 100; j++ {
				idx.Add(fsutil.FileMeta{Path: "p", Size: int64(j % 10), Inode: uint64(i*100 + j)})
			}
			done <- struct{}{}
		}(i)
	}
	for i := 0; i < 8; i++ {
		<-done
	}
	assert.Len(t, idx.Candidates(), 10)
	assert.EqualValues(t, 800, idx.Len())
}

// countingHasher counts calls and delegates to the real hasher.
type countingHasher struct{ partial, full atomic.Int64 }

func (h *countingHasher) Partial(m fsutil.FileMeta) (uint64, error) {
	h.partial.Add(1)
	return hash.Partial(m.Path, m.Size)
}

func (h *countingHasher) Full(m fsutil.FileMeta) (hash.Digest, error) {
	h.full.Add(1)
	return hash.Full(m.Path)
}

// collidingHasher returns the same digest for every file, simulating a
// collision so the verify stage is exercised.
type collidingHasher struct{}

func (collidingHasher) Partial(fsutil.FileMeta) (uint64, error) { return 42, nil }
func (collidingHasher) Full(fsutil.FileMeta) (hash.Digest, error) {
	return hash.Digest{1}, nil
}

var _ = errors.New
