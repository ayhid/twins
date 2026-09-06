package fsutil

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func writeFile(t *testing.T, dir, name, content string) string {
	t.Helper()
	p := filepath.Join(dir, name)
	require.NoError(t, os.WriteFile(p, []byte(content), 0o644))
	return p
}

func TestStat_ReturnsSizeDeviceAndInode(t *testing.T) {
	dir := t.TempDir()
	p := writeFile(t, dir, "a.txt", "hello")

	m, err := Stat(p)
	require.NoError(t, err)

	assert.Equal(t, p, m.Path)
	assert.EqualValues(t, 5, m.Size)
	assert.NotZero(t, m.Inode)
	assert.NotZero(t, m.Dev)
	assert.False(t, m.ModTime.IsZero())
	assert.False(t, m.Dataless)
}

func TestStat_HardlinksShareIdentity(t *testing.T) {
	dir := t.TempDir()
	a := writeFile(t, dir, "a.txt", "hello")
	b := filepath.Join(dir, "b.txt")
	require.NoError(t, os.Link(a, b))

	ma, err := Stat(a)
	require.NoError(t, err)
	mb, err := Stat(b)
	require.NoError(t, err)

	assert.Equal(t, ma.Identity(), mb.Identity())
	assert.EqualValues(t, 2, ma.Nlink)
}

func TestStat_DoesNotFollowSymlinks(t *testing.T) {
	dir := t.TempDir()
	a := writeFile(t, dir, "a.txt", "hello")
	link := filepath.Join(dir, "link")
	require.NoError(t, os.Symlink(a, link))

	_, err := Stat(link)
	assert.ErrorIs(t, err, ErrNotRegular)
}

func TestStat_MissingFile(t *testing.T) {
	_, err := Stat(filepath.Join(t.TempDir(), "nope"))
	assert.Error(t, err)
}

func TestIsLocalVolume(t *testing.T) {
	local, err := IsLocalVolume(t.TempDir())
	require.NoError(t, err)
	assert.True(t, local)
}

func TestIsBundle(t *testing.T) {
	cases := map[string]bool{
		"Foo.app":              true,
		"Photos.photoslibrary": true,
		"backup.sparsebundle":  true,
		"Lib.framework":        true,
		"notes.txt":            false,
		"app":                  false,
		"archive.tar.gz":       false,
	}
	for name, want := range cases {
		assert.Equal(t, want, IsBundle(name), name)
	}
}

func TestHumanSize(t *testing.T) {
	cases := map[int64]string{
		0:                 "0 B",
		512:               "512 B",
		1024:              "1.0 KiB",
		1536:              "1.5 KiB",
		1048576:           "1.0 MiB",
		45_800_000_000:    "42.7 GiB",
		1_099_511_627_776: "1.0 TiB",
	}
	for n, want := range cases {
		assert.Equal(t, want, HumanSize(n), "%d", n)
	}
}

func TestParseSize(t *testing.T) {
	cases := map[string]int64{
		"0":      0,
		"512":    512,
		"10K":    10 * 1024,
		"1.5MiB": 1536 * 1024,
		"2 GB":   2 << 30,
		"1tib":   1 << 40,
		"100 b":  100,
	}
	for in, want := range cases {
		got, err := ParseSize(in)
		require.NoError(t, err, in)
		assert.Equal(t, want, got, in)
	}
	for _, bad := range []string{"", "abc", "10X", "-5", "MB"} {
		_, err := ParseSize(bad)
		assert.Error(t, err, bad)
	}
}
