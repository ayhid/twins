package group

import (
	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/hash"
)

// Hasher abstracts content fingerprinting so a persistent cache can be
// layered on top of the raw implementation.
type Hasher interface {
	Partial(m fsutil.FileMeta) (uint64, error)
	Full(m fsutil.FileMeta) (hash.Digest, error)
}

// DirectHasher reads the file every time.
type DirectHasher struct{}

// Partial implements Hasher.
func (DirectHasher) Partial(m fsutil.FileMeta) (uint64, error) {
	return hash.Partial(m.Path, m.Size)
}

// Full implements Hasher.
func (DirectHasher) Full(m fsutil.FileMeta) (hash.Digest, error) {
	return hash.Full(m.Path)
}
