package journal

import (
	"bufio"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestOpen_CreatesParentDirsAndAppends(t *testing.T) {
	path := filepath.Join(t.TempDir(), "logs", "operations.log")

	j, err := Open(path)
	require.NoError(t, err)
	require.NoError(t, j.Write(Entry{Action: ActionTrash, Path: "/a", Size: 10}))
	require.NoError(t, j.Close())

	j, err = Open(path)
	require.NoError(t, err)
	require.NoError(t, j.Write(Entry{Action: ActionDryRun, Path: "/b", Size: 20, Error: "boom"}))
	require.NoError(t, j.Close())

	f, err := os.Open(path)
	require.NoError(t, err)
	defer f.Close()
	var entries []Entry
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		var e Entry
		require.NoError(t, json.Unmarshal(sc.Bytes(), &e))
		entries = append(entries, e)
	}
	require.Len(t, entries, 2)
	assert.Equal(t, "/a", entries[0].Path)
	assert.Equal(t, ActionTrash, entries[0].Action)
	assert.WithinDuration(t, time.Now(), entries[0].Time, time.Minute)
	assert.Equal(t, "boom", entries[1].Error)
}

func TestDiscard_WritesNothing(t *testing.T) {
	j := Discard()
	assert.NoError(t, j.Write(Entry{Action: ActionDelete, Path: "/x"}))
	assert.NoError(t, j.Close())
}

func TestDefaultPath(t *testing.T) {
	p, err := DefaultPath()
	require.NoError(t, err)
	assert.Contains(t, p, filepath.Join("Library", "Logs", "twins", "operations.log"))
}
