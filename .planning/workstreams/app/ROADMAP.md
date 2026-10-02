# Roadmap: twins — desktop app and automation (workstream `app`)

## Overview

This workstream owns twins.app (Tauri v2 + Svelte), the workflow builder and the background agent.
Every surface is a thin shell over `twins-core`: scanning starts once default Phase 1 exposes the
progress/cancel observer, and every clean goes through the deletion routine from default Phase 2.
Phase 5 ends with a signed-bundle `SMAppService` agent spike that Phase 8 depends on.

Phase numbers are shared across workstreams so requirement IDs and phase directories stay stable.

## Cross-workstream dependencies

| This stream needs | From | For |
|---|---|---|
| default Phase 1 | `default` | Phase 5 scan, progress and cancel |
| default Phase 2 | `default` | Phase 5 clean, confirm and restore |
| default Phase 4 | `default` | Phase 5 config-backed defaults |
| default Phase 6 | `default` | Phase 7 builder writes files `twins run` executes |

## Phases

- [ ] **Phase 5: Desktop App** - twins.app scans, browses, selects, cleans and restores through the core routine; ends with the signed agent spike
- [ ] **Phase 7: Workflow Builder UI** - Users create, preview, run and review workflows in the app with an ordered list editor
- [ ] **Phase 8: Background Agent** - Scheduled and folder-change workflows run with the app closed, one at a time, with notifications and a menu bar item

## Phase Details

### Phase 5: Desktop App
**Goal**: User can find, review and clean duplicates in twins.app with the same guarantees as the CLI, and the signed-bundle background agent approach is proven before workflows depend on it
**Mode:** mvp
**Depends on**: default Phase 1 to start (scan, progress and cancel through the core observer). Clean, confirm and restore (APP-06..APP-11) need default Phase 2; config-backed defaults need default Phase 4
**Requirements**: APP-01, APP-02, APP-03, APP-04, APP-05, APP-06, APP-07, APP-08, APP-09, APP-10, APP-11, APP-13, APP-14
**Success Criteria** (what must be TRUE):
  1. User reaches twins.app by opening it, by running bare `twins` (which prints help if the app is not installed), or by right-clicking a folder in Finder and choosing the twins Quick Action; they pick folders and start a scan, and if Full Disk Access is missing they are guided to grant it and see how many items were skipped
  2. During a scan the user sees progress by stage and can cancel; afterwards a summary shows groups, duplicate files and reclaimable bytes, and the group list stays smooth with tens of thousands of groups, with sort and path filter
  3. User chooses the keeper in each group and the UI cannot express a group with zero kept copies; they can auto-select every group with a keep strategy, adjust by hand, apply folder actions ("keep everything in this folder", "remove copies from this folder") across all groups, and mark protected folders whose files can never be selected for removal
  4. Before anything is removed the user reviews a confirmation (files, bytes, mode); the clean runs through the same core routine as the CLI, then shows per-file results and errors, and any past run can be undone with "Restore this run"
  5. Closing spike: a Developer ID-signed build registers a stub agent through `SMAppService` on real hardware, and the findings Phase 8 depends on are written down: whether the agent gets Full Disk Access, whether it can post a notification whose "Review" action opens the app, how it appears in Login Items, and whether the helper .app fallback is needed
**Plans**: TBD
**UI hint**: yes

### Phase 7: Workflow Builder UI
**Goal**: User can create, preview, run and review workflows in the app without touching files by hand
**Mode:** mvp
**Depends on**: Phase 5, default Phase 6 (workflow model and `twins run`)
**Requirements**: WFL-03, WFL-04, WFL-08, WFL-09
**Success Criteria** (what must be TRUE):
  1. User creates and edits a workflow in an ordered list editor (trigger, scope, filters, keep, outcome), and the result is the same workflow file that `twins run` executes
  2. User previews a workflow as a dry run from the editor and sees what it would report or move to the Trash, with nothing changed
  3. User runs any workflow manually from the app and sees its outcome
  4. User sees each workflow's run history (when, trigger, outcome, files, bytes, errors) in the app and from the CLI
**Plans**: TBD
**UI hint**: yes

### Phase 8: Background Agent
**Goal**: Scheduled and folder-change workflows run on their own while the app is closed, one at a time, and the user learns what they found
**Mode:** mvp
**Depends on**: Phase 7 (and the Phase 5 spike findings)
**Requirements**: AGENT-01, AGENT-02, AGENT-03, AGENT-04, AGENT-05, AGENT-06, WFL-07, APP-12
**Success Criteria** (what must be TRUE):
  1. User turns the background agent on or off from the app; with the app quit, a workflow scheduled daily or weekly runs at its time, and a run missed while the Mac was asleep catches up exactly once
  2. A folder-change workflow runs after activity in the watched folder settles, ignores partial downloads, and is not retriggered by twins' own Trash moves
  3. When several triggers fire together, runs are queued and execute one at a time, so two runs never act on the same files concurrently
  4. After a run that found duplicates the user gets a notification whose "Review" action opens that run in the app, and a menu bar item shows agent status and the last run
  5. User can enable or disable each workflow and pause all automation at once; when the agent cannot read a watched folder, the app says so and how to grant Full Disk Access
**Plans**: TBD
**UI hint**: yes

## Progress

**Execution Order:**
Phases execute in numeric order: 5 → 7 → 8

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 5. Desktop App | 0/TBD | Not started | - |
| 7. Workflow Builder UI | 0/TBD | Not started | - |
| 8. Background Agent | 0/TBD | Not started | - |
