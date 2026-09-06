//go:build !(darwin && cgo)

package trash

import (
	"fmt"
	"os/exec"
	"strings"
)

const trashBackend = "osascript"

// moveToTrash asks Finder to delete the item, which moves it to the Trash.
func moveToTrash(path string) error {
	escaped := strings.ReplaceAll(path, `\`, `\\`)
	escaped = strings.ReplaceAll(escaped, `"`, `\"`)
	script := fmt.Sprintf(`tell application "Finder" to delete POSIX file "%s"`, escaped)
	out, err := exec.Command("osascript", "-e", script).CombinedOutput()
	if err != nil {
		return fmt.Errorf("trash via Finder: %s: %w", strings.TrimSpace(string(out)), err)
	}
	return nil
}
