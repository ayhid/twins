// Package report renders a plan as JSON (for scripts) or text (for humans).
package report

import (
	"encoding/json"
	"fmt"
	"io"
	"time"

	"github.com/ayhid/twins/internal/fsutil"
	"github.com/ayhid/twins/internal/group"
)

// Version of the JSON schema. Bump on breaking changes.
const Version = 1

// Meta carries scan-level information into the report.
type Meta struct {
	Roots      []string
	Files      int64
	Candidates int64
	Strategy   string
	DryRun     bool
}

// Report is the machine-readable result of a scan.
type Report struct {
	Version   int      `json:"version"`
	ScannedAt string   `json:"scanned_at"`
	Roots     []string `json:"roots"`
	Strategy  string   `json:"keep_strategy,omitempty"`
	DryRun    bool     `json:"dry_run"`
	Summary   Summary  `json:"summary"`
	Groups    []Group  `json:"groups"`
}

// Summary aggregates the plan.
type Summary struct {
	FilesScanned     int64  `json:"files_scanned"`
	Candidates       int64  `json:"candidates"`
	Groups           int64  `json:"groups"`
	Duplicates       int64  `json:"duplicates"`
	ReclaimableBytes int64  `json:"reclaimable_bytes"`
	Reclaimable      string `json:"reclaimable"`
}

// Group is one set of identical files with the resolved decision.
type Group struct {
	Size             int64    `json:"size"`
	Digest           string   `json:"digest"`
	ReclaimableBytes int64    `json:"reclaimable_bytes"`
	Keep             string   `json:"keep"`
	Remove           []string `json:"remove"`
	Files            []File   `json:"files"`
}

// File is one member of a group.
type File struct {
	Path    string `json:"path"`
	Inode   uint64 `json:"inode"`
	ModTime string `json:"mtime"`
}

// Build converts a plan into a Report.
func Build(actions []group.Action, meta Meta) Report {
	groups := make([]Group, 0, len(actions))
	var duplicates int64
	for _, a := range actions {
		groups = append(groups, toGroup(a))
		duplicates += int64(len(a.Remove))
	}
	reclaimable := group.TotalReclaimable(actions)
	return Report{
		Version:   Version,
		ScannedAt: time.Now().UTC().Format(time.RFC3339),
		Roots:     meta.Roots,
		Strategy:  meta.Strategy,
		DryRun:    meta.DryRun,
		Summary: Summary{
			FilesScanned:     meta.Files,
			Candidates:       meta.Candidates,
			Groups:           int64(len(actions)),
			Duplicates:       duplicates,
			ReclaimableBytes: reclaimable,
			Reclaimable:      fsutil.HumanSize(reclaimable),
		},
		Groups: groups,
	}
}

func toGroup(a group.Action) Group {
	files := make([]File, 0, len(a.Group.Files))
	for _, f := range a.Group.Files {
		files = append(files, File{Path: f.Path, Inode: f.Inode, ModTime: f.ModTime.UTC().Format(time.RFC3339)})
	}
	remove := make([]string, 0, len(a.Remove))
	for _, f := range a.Remove {
		remove = append(remove, f.Path)
	}
	return Group{
		Size:             a.Group.Size,
		Digest:           a.Group.Digest.String(),
		ReclaimableBytes: a.Reclaimable(),
		Keep:             a.Keep.Path,
		Remove:           remove,
		Files:            files,
	}
}

// WriteJSON writes the report as indented JSON followed by a newline.
func WriteJSON(w io.Writer, r Report) error {
	enc := json.NewEncoder(w)
	enc.SetIndent("", "  ")
	if err := enc.Encode(r); err != nil {
		return fmt.Errorf("encode report: %w", err)
	}
	return nil
}

// WriteText writes a human-readable listing: kept file marked with ★.
func WriteText(w io.Writer, r Report) {
	if len(r.Groups) == 0 {
		fmt.Fprintf(w, "No duplicates found (%d files scanned).\n", r.Summary.FilesScanned)
		return
	}
	for i, g := range r.Groups {
		fmt.Fprintf(w, "[%d] %s × %d  (%s reclaimable)\n", i+1, fsutil.HumanSize(g.Size), len(g.Files), fsutil.HumanSize(g.ReclaimableBytes))
		for _, f := range g.Files {
			marker := " "
			if f.Path == g.Keep {
				marker = "★"
			}
			fmt.Fprintf(w, "  %s %s\n", marker, f.Path)
		}
	}
	fmt.Fprintf(w, "\n%d group%s, %d duplicate%s, %s reclaimable (%d files scanned)\n",
		r.Summary.Groups, plural(r.Summary.Groups),
		r.Summary.Duplicates, plural(r.Summary.Duplicates),
		r.Summary.Reclaimable, r.Summary.FilesScanned)
}

func plural(n int64) string {
	if n == 1 {
		return ""
	}
	return "s"
}
