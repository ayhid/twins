// Package hash computes cheap partial fingerprints and full content digests.
package hash

import (
	"encoding/hex"
	"fmt"
	"io"
	"os"
	"sync"

	"github.com/cespare/xxhash/v2"
	"github.com/zeebo/blake3"
)

const (
	// sampleSize is the number of bytes read at each end of a file for Partial.
	sampleSize = 16 * 1024
	// bufferSize is the streaming buffer used by Full.
	bufferSize = 256 * 1024
)

// Digest is a 256-bit BLAKE3 content hash.
type Digest [32]byte

// String renders the digest as lowercase hex.
func (d Digest) String() string { return hex.EncodeToString(d[:]) }

// IsZero reports whether the digest has not been computed.
func (d Digest) IsZero() bool { return d == Digest{} }

var buffers = sync.Pool{
	New: func() any {
		b := make([]byte, bufferSize)
		return &b
	},
}

// Partial fingerprints the first and last sampleSize bytes of the file.
// Files no larger than two samples are read entirely. It is designed to
// discard non-duplicates with at most two reads.
func Partial(path string, size int64) (uint64, error) {
	f, err := os.Open(path)
	if err != nil {
		return 0, fmt.Errorf("open: %w", err)
	}
	defer f.Close()

	h := xxhash.New()
	if size <= 2*sampleSize {
		if _, err := io.Copy(h, f); err != nil {
			return 0, fmt.Errorf("read %s: %w", path, err)
		}
		return h.Sum64(), nil
	}

	buf := make([]byte, sampleSize)
	if _, err := io.ReadFull(f, buf); err != nil {
		return 0, fmt.Errorf("read head %s: %w", path, err)
	}
	_, _ = h.Write(buf)
	if _, err := f.ReadAt(buf, size-sampleSize); err != nil && err != io.EOF {
		return 0, fmt.Errorf("read tail %s: %w", path, err)
	}
	_, _ = h.Write(buf)
	return h.Sum64(), nil
}

// Full computes the BLAKE3 digest of the whole file using a pooled buffer.
func Full(path string) (Digest, error) {
	f, err := os.Open(path)
	if err != nil {
		return Digest{}, fmt.Errorf("open: %w", err)
	}
	defer f.Close()

	buf := buffers.Get().(*[]byte)
	defer buffers.Put(buf)

	h := blake3.New()
	if _, err := io.CopyBuffer(h, f, *buf); err != nil {
		return Digest{}, fmt.Errorf("read %s: %w", path, err)
	}
	var d Digest
	copy(d[:], h.Sum(nil))
	return d, nil
}
