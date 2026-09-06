// Package cache persists file fingerprints so repeated scans only hash
// files that changed. Entries are keyed by (device, inode, size, mtime).
package cache

import (
	"encoding/binary"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"sync/atomic"
	"time"

	bolt "go.etcd.io/bbolt"

	"github.com/ayhid/twins/internal/config"
	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/hash"
)

var (
	bucketPartial = []byte("partial")
	bucketFull    = []byte("full")
)

// Hasher is the subset of group.Hasher the store wraps.
type Hasher interface {
	Partial(m fsutil.FileMeta) (uint64, error)
	Full(m fsutil.FileMeta) (hash.Digest, error)
}

// Stats counts cache activity for one session.
type Stats struct {
	Hits   int64
	Misses int64
}

// flushThreshold bounds the number of pending writes held in memory.
const flushThreshold = 4096

// Store is a bbolt-backed Hasher decorator. It is safe for concurrent use.
// Writes are buffered and committed in one transaction (on Close or every
// flushThreshold entries): one fsync per batch instead of one per file.
type Store struct {
	db     *bolt.DB
	inner  Hasher
	hits   atomic.Int64
	misses atomic.Int64

	mu      sync.Mutex
	pending map[string]pendingEntry
}

type pendingEntry struct {
	bucket []byte
	value  []byte
}

// DefaultPath returns ~/Library/Application Support/twins/cache.db.
func DefaultPath() (string, error) {
	dir, err := config.AppDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(dir, "cache.db"), nil
}

// Open opens or creates the cache at path, wrapping inner.
func Open(path string, inner Hasher) (*Store, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return nil, fmt.Errorf("create cache dir: %w", err)
	}
	db, err := bolt.Open(path, 0o644, &bolt.Options{Timeout: 2 * time.Second})
	if err != nil {
		return nil, fmt.Errorf("open cache %s: %w", path, err)
	}
	err = db.Update(func(tx *bolt.Tx) error {
		for _, b := range [][]byte{bucketPartial, bucketFull} {
			if _, err := tx.CreateBucketIfNotExists(b); err != nil {
				return err
			}
		}
		return nil
	})
	if err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("init cache: %w", err)
	}
	return &Store{db: db, inner: inner, pending: make(map[string]pendingEntry)}, nil
}

// Close flushes pending writes and releases the database.
func (s *Store) Close() error {
	flushErr := s.Flush()
	if err := s.db.Close(); err != nil {
		return err
	}
	return flushErr
}

// Flush commits buffered fingerprints in a single transaction.
func (s *Store) Flush() error {
	s.mu.Lock()
	batch := s.pending
	s.pending = make(map[string]pendingEntry)
	s.mu.Unlock()
	if len(batch) == 0 {
		return nil
	}
	err := s.db.Update(func(tx *bolt.Tx) error {
		for k, e := range batch {
			raw := []byte(k[len(e.bucket)+1:])
			if err := tx.Bucket(e.bucket).Put(raw, e.value); err != nil {
				return err
			}
		}
		return nil
	})
	if err != nil {
		return fmt.Errorf("flush cache: %w", err)
	}
	return nil
}

// Stats returns hit/miss counters.
func (s *Store) Stats() Stats {
	return Stats{Hits: s.hits.Load(), Misses: s.misses.Load()}
}

// Clear drops every cached fingerprint, buffered or committed.
func (s *Store) Clear() error {
	s.mu.Lock()
	s.pending = make(map[string]pendingEntry)
	s.mu.Unlock()
	return s.db.Update(func(tx *bolt.Tx) error {
		for _, b := range [][]byte{bucketPartial, bucketFull} {
			if err := tx.DeleteBucket(b); err != nil {
				return err
			}
			if _, err := tx.CreateBucket(b); err != nil {
				return err
			}
		}
		return nil
	})
}

// Partial implements Hasher with caching.
func (s *Store) Partial(m fsutil.FileMeta) (uint64, error) {
	k := key(m)
	if v := s.get(bucketPartial, k); len(v) == 8 {
		s.hits.Add(1)
		return binary.BigEndian.Uint64(v), nil
	}
	s.misses.Add(1)
	h, err := s.inner.Partial(m)
	if err != nil {
		return 0, err
	}
	s.put(bucketPartial, k, binary.BigEndian.AppendUint64(nil, h))
	return h, nil
}

// Full implements Hasher with caching.
func (s *Store) Full(m fsutil.FileMeta) (hash.Digest, error) {
	k := key(m)
	if v := s.get(bucketFull, k); len(v) == len(hash.Digest{}) {
		s.hits.Add(1)
		var d hash.Digest
		copy(d[:], v)
		return d, nil
	}
	s.misses.Add(1)
	d, err := s.inner.Full(m)
	if err != nil {
		return hash.Digest{}, err
	}
	s.put(bucketFull, k, d[:])
	return d, nil
}

func (s *Store) get(bucket, k []byte) []byte {
	s.mu.Lock()
	if e, ok := s.pending[pendingKey(bucket, k)]; ok {
		s.mu.Unlock()
		return e.value
	}
	s.mu.Unlock()
	var out []byte
	_ = s.db.View(func(tx *bolt.Tx) error {
		if v := tx.Bucket(bucket).Get(k); v != nil {
			out = append([]byte(nil), v...)
		}
		return nil
	})
	return out
}

// put buffers the write; Flush commits it.
func (s *Store) put(bucket, k, v []byte) {
	s.mu.Lock()
	s.pending[pendingKey(bucket, k)] = pendingEntry{bucket: bucket, value: append([]byte(nil), v...)}
	full := len(s.pending) >= flushThreshold
	s.mu.Unlock()
	if full {
		_ = s.Flush()
	}
}

// pendingKey namespaces a raw key by bucket. The bucket name is stripped
// again at flush time, so only the raw key is stored in bbolt.
func pendingKey(bucket, k []byte) string {
	return string(bucket) + "\x00" + string(k)
}

// key encodes the identity and the change-detecting fields.
func key(m fsutil.FileMeta) []byte {
	b := make([]byte, 0, 32)
	b = binary.BigEndian.AppendUint64(b, m.Dev)
	b = binary.BigEndian.AppendUint64(b, m.Inode)
	b = binary.BigEndian.AppendUint64(b, uint64(m.Size))
	b = binary.BigEndian.AppendUint64(b, uint64(m.ModTime.UnixNano()))
	return b
}
