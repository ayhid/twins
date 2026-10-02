# Roadmap: twins — release pipeline (workstream `release`)

## Overview

This workstream owns CI, semantic-release channels, Developer ID signing, notarization and the
Homebrew cask. It starts now against the CLI and finishes once app Phase 8 has an app and an
agent to bundle.

Phase numbers are shared across workstreams so requirement IDs and phase directories stay stable.

## Cross-workstream dependencies

| This stream needs | From | For |
|---|---|---|
| app Phase 8 | `app` | signing and casking the app and agent (REL-02, REL-03) |

## Phases

- [ ] **Phase 9: Signed Release** - Signed, notarized builds ship through semantic-release channels and a Homebrew cask

## Phase Details

### Phase 9: Signed Release
**Goal**: User installs a signed, notarized twins with Homebrew, and every push produces a tested build on the right release channel
**Mode:** mvp
**Depends on**: Nothing to start: CI (REL-04), semantic-release channels (REL-01) and signing/notarization (REL-02) are built against the CLI first. Bundling, signing and casking the app and agent (rest of REL-02, REL-03) need app Phase 8
**Requirements**: REL-01, REL-02, REL-03, REL-04
**Success Criteria** (what must be TRUE):
  1. Every push runs tests, lints and a smoke test of the built bundle in CI
  2. Every push to `main` publishes a beta prerelease, and fast-forwarding `stable` publishes a release through semantic-release
  3. The app, CLI and agent binaries in a downloaded build are signed with Developer ID and hardened runtime and are notarized, so Gatekeeper opens them without warnings
  4. `brew install --cask ayhid/tap/twins` (or `twins@beta`) installs the app and puts the `twins` CLI on the PATH, and uninstalling the cask unregisters the agent
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 9

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 9. Signed Release | 0/TBD | Not started | - |
