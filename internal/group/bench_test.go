package group_test

import (
	"context"
	"crypto/rand"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
	"github.com/ayhid/twins/internal/scan"
)

// benchTree writes n files of size bytes: half unique, half in pairs.
func benchTree(b *testing.B, n, size int) string {
	b.Helper()
	root := b.TempDir()
	buf := make([]byte, size)
	for i := 0; i < n; i++ {
		if i%2 == 0 {
			if _, err := rand.Read(buf); err != nil {
				b.Fatal(err)
			}
		}
		dir := filepath.Join(root, fmt.Sprintf("d%02d", i%16))
		_ = os.MkdirAll(dir, 0o755)
		if err := os.WriteFile(filepath.Join(dir, fmt.Sprintf("f%05d", i)), buf, 0o644); err != nil {
			b.Fatal(err)
		}
	}
	return root
}

func benchPipeline(b *testing.B, n, size int) {
	root := benchTree(b, n, size)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		idx := group.NewIndex()
		_, err := scan.Walk(context.Background(), scan.Options{Roots: []string{root}, MinSize: 1}, func(m fsutil.FileMeta) { idx.Add(m) })
		if err != nil {
			b.Fatal(err)
		}
		groups, err := group.Find(context.Background(), idx, group.Options{})
		if err != nil {
			b.Fatal(err)
		}
		if len(groups) != n/2 {
			b.Fatalf("got %d groups, want %d", len(groups), n/2)
		}
	}
	b.ReportMetric(float64(n)/b.Elapsed().Seconds()*float64(b.N), "files/s")
}

func BenchmarkPipeline_2000x64KiB(b *testing.B) { benchPipeline(b, 2000, 64*1024) }
func BenchmarkPipeline_200x4MiB(b *testing.B)   { benchPipeline(b, 200, 4*1024*1024) }
