package group

import (
	"sort"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/hash"
)

// Group is an immutable set of files with identical content.
type Group struct {
	Size   int64
	Digest hash.Digest
	Files  []fsutil.FileMeta // sorted by path
}

// Physical counts distinct on-disk copies (hardlinks count once).
func (g Group) Physical() int { return physical(g.Files) }

// Reclaimable is the space freed by keeping a single physical copy.
func (g Group) Reclaimable() int64 {
	return g.Size * int64(max(g.Physical()-1, 0))
}

func newGroup(size int64, digest hash.Digest, files []fsutil.FileMeta) Group {
	sorted := append([]fsutil.FileMeta(nil), files...)
	sort.Slice(sorted, func(a, b int) bool { return sorted[a].Path < sorted[b].Path })
	return Group{Size: size, Digest: digest, Files: sorted}
}

// sortGroups orders groups by reclaimable space, then size, then first path.
func sortGroups(groups []Group) []Group {
	out := append([]Group(nil), groups...)
	sort.SliceStable(out, func(a, b int) bool {
		ra, rb := out[a].Reclaimable(), out[b].Reclaimable()
		if ra != rb {
			return ra > rb
		}
		if out[a].Size != out[b].Size {
			return out[a].Size > out[b].Size
		}
		return out[a].Files[0].Path < out[b].Files[0].Path
	})
	return out
}
