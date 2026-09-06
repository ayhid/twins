// Package journal appends an auditable JSON-lines log of every operation
// twins performs (or would perform in dry-run mode).
package journal

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// Action names recorded in the journal.
const (
	ActionTrash  = "trash"
	ActionDelete = "delete"
	ActionLink   = "link"
	ActionDryRun = "dry-run"
)

// Entry is one journal line.
type Entry struct {
	Time   time.Time `json:"time"`
	Action string    `json:"action"`
	Path   string    `json:"path"`
	Size   int64     `json:"size"`
	Digest string    `json:"digest,omitempty"`
	Keep   string    `json:"keep,omitempty"`
	Error  string    `json:"error,omitempty"`
}

// Journal is a thread-safe appender. Use Discard for a no-op journal.
type Journal struct {
	mu sync.Mutex
	f  *os.File
}

// DefaultPath returns ~/Library/Logs/twins/operations.log.
func DefaultPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", fmt.Errorf("home dir: %w", err)
	}
	return filepath.Join(home, "Library", "Logs", "twins", "operations.log"), nil
}

// Open opens (or creates) the journal file in append mode.
func Open(path string) (*Journal, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return nil, fmt.Errorf("create journal dir: %w", err)
	}
	f, err := os.OpenFile(path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644)
	if err != nil {
		return nil, fmt.Errorf("open journal: %w", err)
	}
	return &Journal{f: f}, nil
}

// Discard returns a journal that records nothing.
func Discard() *Journal { return &Journal{} }

// Write appends one entry, stamping the time if unset.
func (j *Journal) Write(e Entry) error {
	if j.f == nil {
		return nil
	}
	if e.Time.IsZero() {
		e.Time = time.Now()
	}
	line, err := json.Marshal(e)
	if err != nil {
		return fmt.Errorf("encode journal entry: %w", err)
	}
	j.mu.Lock()
	defer j.mu.Unlock()
	if _, err := j.f.Write(append(line, '\n')); err != nil {
		return fmt.Errorf("write journal: %w", err)
	}
	return nil
}

// Close flushes and closes the file.
func (j *Journal) Close() error {
	if j.f == nil {
		return nil
	}
	return j.f.Close()
}
