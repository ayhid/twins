package hash

import (
	"bytes"
	"crypto/rand"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func write(t *testing.T, name string, data []byte) string {
	t.Helper()
	p := filepath.Join(t.TempDir(), name)
	require.NoError(t, os.WriteFile(p, data, 0o644))
	return p
}

func random(t *testing.T, n int) []byte {
	t.Helper()
	b := make([]byte, n)
	_, err := rand.Read(b)
	require.NoError(t, err)
	return b
}

func TestPartial_SameForIdenticalContent(t *testing.T) {
	data := random(t, 100*1024)
	a, b := write(t, "a", data), write(t, "b", data)

	ha, err := Partial(a, int64(len(data)))
	require.NoError(t, err)
	hb, err := Partial(b, int64(len(data)))
	require.NoError(t, err)
	assert.Equal(t, ha, hb)
}

func TestPartial_DiffersWhenHeadOrTailDiffers(t *testing.T) {
	data := random(t, 100*1024)
	head := append([]byte{}, data...)
	head[0] ^= 0xff
	tail := append([]byte{}, data...)
	tail[len(tail)-1] ^= 0xff

	base, err := Partial(write(t, "a", data), int64(len(data)))
	require.NoError(t, err)
	h1, err := Partial(write(t, "b", head), int64(len(head)))
	require.NoError(t, err)
	h2, err := Partial(write(t, "c", tail), int64(len(tail)))
	require.NoError(t, err)

	assert.NotEqual(t, base, h1)
	assert.NotEqual(t, base, h2)
}

func TestPartial_IgnoresMiddleOfLargeFiles(t *testing.T) {
	data := random(t, 100*1024)
	mid := append([]byte{}, data...)
	mid[50*1024] ^= 0xff

	h1, err := Partial(write(t, "a", data), int64(len(data)))
	require.NoError(t, err)
	h2, err := Partial(write(t, "b", mid), int64(len(mid)))
	require.NoError(t, err)
	assert.Equal(t, h1, h2, "partial hash only samples head and tail")
}

func TestPartial_SmallFileReadEntirely(t *testing.T) {
	data := random(t, 1000)
	mid := append([]byte{}, data...)
	mid[500] ^= 0xff

	h1, err := Partial(write(t, "a", data), int64(len(data)))
	require.NoError(t, err)
	h2, err := Partial(write(t, "b", mid), int64(len(mid)))
	require.NoError(t, err)
	assert.NotEqual(t, h1, h2)
}

func TestFull_DetectsAnyByteChange(t *testing.T) {
	data := random(t, 3*1024*1024)
	mid := append([]byte{}, data...)
	mid[1500*1024] ^= 0xff

	h1, err := Full(write(t, "a", data))
	require.NoError(t, err)
	h2, err := Full(write(t, "b", mid))
	require.NoError(t, err)
	h3, err := Full(write(t, "c", data))
	require.NoError(t, err)

	assert.NotEqual(t, h1, h2)
	assert.Equal(t, h1, h3)
	assert.Len(t, h1.String(), 64)
}

func TestFull_EmptyFile(t *testing.T) {
	h, err := Full(write(t, "empty", nil))
	require.NoError(t, err)
	assert.NotEqual(t, Digest{}, h)
}

func TestEqual(t *testing.T) {
	data := random(t, 300*1024)
	other := append([]byte{}, data...)
	other[299*1024] ^= 0xff

	a, b, c := write(t, "a", data), write(t, "b", data), write(t, "c", other)

	same, err := Equal(a, b)
	require.NoError(t, err)
	assert.True(t, same)

	same, err = Equal(a, c)
	require.NoError(t, err)
	assert.False(t, same)
}

func TestErrorsOnMissingFile(t *testing.T) {
	missing := filepath.Join(t.TempDir(), "missing")
	_, err := Partial(missing, 10)
	assert.Error(t, err)
	_, err = Full(missing)
	assert.Error(t, err)
	_, err = Equal(missing, missing)
	assert.Error(t, err)
}

func BenchmarkFull_8MiB(b *testing.B) {
	data := bytes.Repeat([]byte("twins"), 8*1024*1024/5)
	p := filepath.Join(b.TempDir(), "big")
	require.NoError(b, os.WriteFile(p, data, 0o644))
	b.SetBytes(int64(len(data)))
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if _, err := Full(p); err != nil {
			b.Fatal(err)
		}
	}
}
