package cli

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ayhid/twins/internal/report"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// run executes the CLI with isolated config, cache and journal.
func run(t *testing.T, stdin string, args ...string) (int, string, string) {
	t.Helper()
	iso := t.TempDir()
	t.Setenv("TWINS_CACHE", filepath.Join(iso, "cache.db"))
	t.Setenv("TWINS_JOURNAL", filepath.Join(iso, "ops.log"))
	args = append([]string{"--config", filepath.Join(iso, "config.toml")}, args...)
	var out, errOut bytes.Buffer
	code := Execute(context.Background(), args, strings.NewReader(stdin), &out, &errOut)
	return code, out.String(), errOut.String()
}

// dupTree writes two identical 2 KiB files and one unique file.
func dupTree(t *testing.T) (root, a, b, unique string) {
	t.Helper()
	root = t.TempDir()
	data := bytes.Repeat([]byte("twins!"), 400)
	a, b, unique = filepath.Join(root, "a.bin"), filepath.Join(root, "sub", "b.bin"), filepath.Join(root, "u.bin")
	require.NoError(t, os.MkdirAll(filepath.Dir(b), 0o755))
	require.NoError(t, os.WriteFile(a, data, 0o644))
	require.NoError(t, os.WriteFile(b, data, 0o644))
	require.NoError(t, os.WriteFile(unique, bytes.Repeat([]byte("solo!!"), 400), 0o644))
	return root, a, b, unique
}

func TestVersion(t *testing.T) {
	code, out, _ := run(t, "", "version")
	assert.Equal(t, ExitOK, code)
	assert.Equal(t, "twins dev\n", out)

	code, out, _ = run(t, "", "--version")
	assert.Equal(t, ExitOK, code)
	assert.Equal(t, "twins dev\n", out)
}

func TestScan_JSON(t *testing.T) {
	root, a, b, _ := dupTree(t)
	code, out, errOut := run(t, "", "scan", "--json", "--min-size", "1", root)
	require.Equal(t, ExitOK, code, errOut)

	var r report.Report
	require.NoError(t, json.Unmarshal([]byte(out), &r))
	assert.Equal(t, []string{root}, r.Roots)
	assert.EqualValues(t, 3, r.Summary.FilesScanned)
	assert.EqualValues(t, 1, r.Summary.Groups)
	assert.EqualValues(t, 2400, r.Summary.ReclaimableBytes)
	assert.Equal(t, "oldest", r.Strategy)
	require.Len(t, r.Groups, 1)
	assert.ElementsMatch(t, []string{a, b}, []string{r.Groups[0].Keep, r.Groups[0].Remove[0]})
}

func TestScan_PlainTextAndKeepStrategy(t *testing.T) {
	root, a, b, _ := dupTree(t)
	code, out, _ := run(t, "", "scan", "--plain", "--min-size", "1", "--keep", "shortest-path", root)
	require.Equal(t, ExitOK, code)
	assert.Contains(t, out, "★ "+a)
	assert.Contains(t, out, "  "+b)
	assert.Contains(t, out, "1 group, 1 duplicate")
}

func TestScan_MinSizeFiltersEverything(t *testing.T) {
	root, _, _, _ := dupTree(t)
	code, out, _ := run(t, "", "scan", "--plain", "--min-size", "1MiB", root)
	require.Equal(t, ExitOK, code)
	assert.Contains(t, out, "No duplicates")
}

func TestReport_IsAlwaysJSON(t *testing.T) {
	root, _, _, _ := dupTree(t)
	code, out, _ := run(t, "", "report", "--min-size", "1", root)
	require.Equal(t, ExitOK, code)
	assert.True(t, strings.HasPrefix(out, "{"))
}

func TestScan_RejectsBadStrategyWithUsageExit(t *testing.T) {
	root, _, _, _ := dupTree(t)
	code, _, _ := run(t, "", "scan", "--plain", "--keep", "random", root)
	assert.Equal(t, ExitUsage, code)
}

func TestScan_UnknownFlagIsUsageError(t *testing.T) {
	code, _, errOut := run(t, "", "scan", "--bogus")
	assert.Equal(t, ExitUsage, code)
	assert.Contains(t, errOut, "unknown flag")
}

func TestScan_ProtectedRootFails(t *testing.T) {
	code, _, errOut := run(t, "", "scan", "--plain", "/System")
	assert.Equal(t, ExitError, code)
	assert.Contains(t, errOut, "protected")
}

func TestClean_DryRunTouchesNothing(t *testing.T) {
	root, a, b, _ := dupTree(t)
	code, out, _ := run(t, "", "clean", "--yes", "--dry-run", "--min-size", "1", root)
	require.Equal(t, ExitOK, code)
	assert.Contains(t, out, "Dry run: would move to trash 1 files")
	assert.FileExists(t, a)
	assert.FileExists(t, b)
}

func TestClean_PermanentWithForce(t *testing.T) {
	root, a, b, unique := dupTree(t)
	code, out, errOut := run(t, "", "clean", "--yes", "--permanent", "--force", "--min-size", "1", "--keep", "shortest-path", root)
	require.Equal(t, ExitOK, code, errOut)
	assert.Contains(t, out, "Delete: 1 files, 2.3 KiB freed")
	assert.FileExists(t, a)
	assert.NoFileExists(t, b)
	assert.FileExists(t, unique)

	logged, err := os.ReadFile(os.Getenv("TWINS_JOURNAL"))
	require.NoError(t, err)
	assert.Contains(t, string(logged), `"action":"delete"`)
	assert.Contains(t, string(logged), b)
}

func TestClean_PermanentPromptsForTheWord(t *testing.T) {
	root, _, b, _ := dupTree(t)
	code, out, _ := run(t, "nope\n", "clean", "--yes", "--permanent", "--min-size", "1", root)
	require.Equal(t, ExitOK, code)
	assert.Contains(t, out, `Type "permanent"`)
	assert.FileExists(t, b, "declined confirmation must not delete")

	code, _, _ = run(t, "permanent\n", "clean", "--yes", "--permanent", "--min-size", "1", "--keep", "shortest-path", root)
	require.Equal(t, ExitOK, code)
	assert.NoFileExists(t, b)
}

func TestClean_AsksBeforeTrashingWithoutYes(t *testing.T) {
	root, _, b, _ := dupTree(t)
	code, out, _ := run(t, "n\n", "clean", "--plain", "--min-size", "1", root)
	require.Equal(t, ExitOK, code)
	assert.Contains(t, out, "Move to Trash 1 files")
	assert.FileExists(t, b)
}

func TestClean_LinkReplacesWithClone(t *testing.T) {
	root, a, b, _ := dupTree(t)
	code, out, errOut := run(t, "", "clean", "--yes", "--link", "--min-size", "1", "--keep", "shortest-path", root)
	require.Equal(t, ExitOK, code, errOut)
	assert.Contains(t, out, "Replace with clones: 1 files")
	assert.FileExists(t, a)
	assert.FileExists(t, b)
	ca, _ := os.ReadFile(a)
	cb, _ := os.ReadFile(b)
	assert.Equal(t, ca, cb)
}

func TestClean_PermanentAndLinkAreExclusive(t *testing.T) {
	root, _, _, _ := dupTree(t)
	code, _, _ := run(t, "", "clean", "--yes", "--permanent", "--link", root)
	assert.Equal(t, ExitUsage, code)
}

func TestConfig_PathShowInitAndClearCache(t *testing.T) {
	code, out, _ := run(t, "", "config", "path")
	require.Equal(t, ExitOK, code)
	assert.Contains(t, out, "config:")
	assert.Contains(t, out, "cache.db")
	assert.Contains(t, out, "operations.log")

	code, out, _ = run(t, "", "config", "show")
	require.Equal(t, ExitOK, code)
	assert.Contains(t, out, `min_size = "1MiB"`)

	iso := t.TempDir()
	cfg := filepath.Join(iso, "config.toml")
	code, out, _ = run(t, "", "--config", cfg, "config", "init")
	require.Equal(t, ExitOK, code)
	assert.Contains(t, out, "wrote")
	assert.FileExists(t, cfg)
	code, _, errOut := run(t, "", "--config", cfg, "config", "init")
	assert.Equal(t, ExitError, code)
	assert.Contains(t, errOut, "already exists")

	code, out, _ = run(t, "", "config", "clear-cache")
	require.Equal(t, ExitOK, code)
	assert.Contains(t, out, "cache cleared")
}

func TestConfig_FileValuesAreUsedAndFlagsOverride(t *testing.T) {
	root, a, b, _ := dupTree(t)
	iso := t.TempDir()
	cfg := filepath.Join(iso, "config.toml")
	require.NoError(t, os.WriteFile(cfg, []byte("min_size = \"1\"\nkeep = \"shortest-path\"\n"), 0o644))

	code, out, _ := run(t, "", "--config", cfg, "scan", "--plain", root)
	require.Equal(t, ExitOK, code)
	assert.Contains(t, out, "★ "+a)

	code, out, _ = run(t, "", "--config", cfg, "scan", "--plain", "--keep", "in-dir", "--keep-in", filepath.Dir(b), root)
	require.Equal(t, ExitOK, code)
	assert.Contains(t, out, "★ "+b)
}

func TestConfig_InvalidFileIsAnError(t *testing.T) {
	cfg := filepath.Join(t.TempDir(), "config.toml")
	require.NoError(t, os.WriteFile(cfg, []byte("keep = \"random\"\n"), 0o644))
	code, _, errOut := run(t, "", "--config", cfg, "config", "show")
	assert.Equal(t, ExitError, code)
	assert.Contains(t, errOut, "keep")
}

func TestExcludeGlob(t *testing.T) {
	root, _, _, _ := dupTree(t)
	code, out, _ := run(t, "", "scan", "--plain", "--min-size", "1", "-e", "sub", root)
	require.Equal(t, ExitOK, code)
	assert.Contains(t, out, "No duplicates")
}

func TestVerboseReportsUnreadableFiles(t *testing.T) {
	root, a, _, _ := dupTree(t)
	require.NoError(t, os.Chmod(a, 0o000))
	t.Cleanup(func() { _ = os.Chmod(a, 0o644) })
	code, _, errOut := run(t, "", "scan", "--plain", "--verbose", "--min-size", "1", root)
	require.Equal(t, ExitOK, code)
	assert.Contains(t, errOut, "skip "+a)
}
