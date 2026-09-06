package report

import (
	"bytes"
	"encoding/json"
	"testing"
	"time"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
	"github.com/ayhid/twins/internal/hash"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func samplePlan() []group.Action {
	at := time.Date(2026, 9, 6, 12, 0, 0, 0, time.UTC)
	a := fsutil.FileMeta{Path: "/Users/me/a.jpg", Size: 2048, Dev: 1, Inode: 1, ModTime: at}
	b := fsutil.FileMeta{Path: "/Users/me/b.jpg", Size: 2048, Dev: 1, Inode: 2, ModTime: at}
	g := group.Group{Size: 2048, Digest: hash.Digest{0xab}, Files: []fsutil.FileMeta{a, b}}
	return []group.Action{{Group: g, Keep: a, Remove: []fsutil.FileMeta{b}}}
}

func TestBuild_SummarisesPlan(t *testing.T) {
	r := Build(samplePlan(), Meta{Roots: []string{"/Users/me"}, Files: 10, Candidates: 4, Strategy: "oldest"})

	assert.Equal(t, Version, r.Version)
	assert.Equal(t, []string{"/Users/me"}, r.Roots)
	assert.EqualValues(t, 10, r.Summary.FilesScanned)
	assert.EqualValues(t, 4, r.Summary.Candidates)
	assert.EqualValues(t, 1, r.Summary.Groups)
	assert.EqualValues(t, 1, r.Summary.Duplicates)
	assert.EqualValues(t, 2048, r.Summary.ReclaimableBytes)
	assert.Equal(t, "2.0 KiB", r.Summary.Reclaimable)
	require.Len(t, r.Groups, 1)
	assert.Equal(t, "ab"+string(bytes.Repeat([]byte("0"), 62)), r.Groups[0].Digest)
	assert.Equal(t, "/Users/me/a.jpg", r.Groups[0].Keep)
	assert.Equal(t, []string{"/Users/me/b.jpg"}, r.Groups[0].Remove)
	require.Len(t, r.Groups[0].Files, 2)
	assert.Equal(t, "2026-09-06T12:00:00Z", r.Groups[0].Files[0].ModTime)
}

func TestWriteJSON_IsStableAndParseable(t *testing.T) {
	r := Build(samplePlan(), Meta{Roots: []string{"/Users/me"}})
	r.ScannedAt = "2026-09-06T12:00:00Z"

	var buf bytes.Buffer
	require.NoError(t, WriteJSON(&buf, r))

	var back Report
	require.NoError(t, json.Unmarshal(buf.Bytes(), &back))
	assert.Equal(t, r, back)
	assert.True(t, bytes.HasSuffix(buf.Bytes(), []byte("\n")))
}

func TestWriteText_ShowsGroupsAndTotals(t *testing.T) {
	r := Build(samplePlan(), Meta{Roots: []string{"/Users/me"}, Files: 10})
	var buf bytes.Buffer
	WriteText(&buf, r)
	out := buf.String()

	assert.Contains(t, out, "1 group")
	assert.Contains(t, out, "2.0 KiB")
	assert.Contains(t, out, "★ /Users/me/a.jpg")
	assert.Contains(t, out, "  /Users/me/b.jpg")
}

func TestWriteText_EmptyPlan(t *testing.T) {
	var buf bytes.Buffer
	WriteText(&buf, Build(nil, Meta{Files: 3}))
	assert.Contains(t, buf.String(), "No duplicates")
}
