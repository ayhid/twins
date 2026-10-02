# Stack Research

**Domain:** macOS duplicate-file finder. Rust engine + CLI, Tauri v2 + Svelte desktop app, a workflow automation builder with a background agent, signed and notarized Homebrew cask.
**Researched:** 2026-10-02
**Confidence:** MEDIUM-HIGH. Versions come straight from the crates.io and npm registry APIs (pulled 2026-10-02). Behaviour claims were checked against the published crate source where it mattered: trash 5.2.9, tauri-bundler 2.10.1, redb 4.3.0, auto-launch 0.6.0, notify 8.2.0, rustix 1.1.5 and libc 0.2.189. Three areas remain unverified on real hardware and are marked as spikes: notifications from a bare agent executable, Full Disk Access inheritance for the agent, and Put Back for batch trashing.

Scope: this file covers only the **additions** for milestone steps 2–5 plus workflows. The existing scan and hash stack (walkdir, rayon, blake3, xxhash-rust, globset, clap, serde/serde_json, thiserror, libc) stays as it is and was not re-researched.

---

## Headline decisions (read this first)

1. **Raise the workspace MSRV from 1.85 to 1.90.** `tauri 2.12.1` declares `rust-version = 1.90`, so the workspace cannot build the app crate at 1.85. The bump also makes `std::fs::File::lock`/`try_lock` available (stable since 1.89), which removes the need for `fs2`/`fs4`/`fd-lock` for the journal and agent locks. The local toolchain is 1.98.1.
2. **Trash: call `NSFileManager trashItemAtURL:resultingItemURL:error:` directly through `objc2-foundation`, not through the `trash` crate.** The `trash` crate's macOS default is `DeleteMethod::Finder`, which spawns `osascript`, needs the Automation permission and plays the Finder sound. Its `NsFileManager` path also passes `None` for `resultingItemURL`, so it discards the trashed location the journal needs for undo.
3. **Put Back is not reliable through `NSFileManager` for batches.** The `trash` crate's own docs table marks it ✗. `sindresorhus/macos-trash` fixed it in Jan 2026 (v3.0+) by writing the missing `ptbL`/`ptbN` records into `~/.Trash/.DS_Store` after trashing. twins treats the **journal** (original path → `resultingItemURL`) as the undo contract. Restoring Finder Put Back in full is a separate, optional step, covered under "Stack Patterns by Variant".
4. **Cache: SQLite (rusqlite 0.40, `bundled`, WAL), not redb.** redb 4.x opens a database file with an exclusive per-process writer lock by default. Its multi-process mode sits behind an `experimental-multiprocess` flag that is not released yet (redb 5.0 changelog). The CLI, the app and the background agent are three processes that will open the cache at the same time, and SQLite WAL with `busy_timeout` handles that.
5. **Background agent: a headless `twins-agent` binary shipped in `Contents/MacOS` via Tauri `externalBin`, registered with `SMAppService.agent(plistName:)` (macOS 13+) through `objc2-service-management`.** Do not use `tauri-plugin-autostart` for this. It wraps `auto-launch 0.6`, which can do SMAppService, but the plugin only exposes `LaunchAgent` (a plist written into `~/Library/LaunchAgents`) and `AppleScript` modes, and either way it relaunches the GUI app rather than a headless agent.
6. **Frontend: Svelte 5.57 + SvelteKit 3.0 (SPA via adapter-static 4) + Vite 8 + TypeScript 6.0.x, with `@xyflow/svelte` 1.7 for the canvas and `virtua` for the virtualized group list.** Pin TypeScript to 6.x: SvelteKit 3 declares a `typescript ^6.0.0` peer and `svelte-check` declares `^5 || ^6`, so the Go-native TypeScript 7.0 does not satisfy them.
7. **Release: semantic-release 25 (stable line) + `@semantic-release/exec` for the version and cask steps; tauri-action v1 or `tauri build` on a macOS runner for build, signing and notarization; your own tap `ayhid/homebrew-tap`.** Homebrew disabled casks that fail Gatekeeper on 2026-09-01 and removed `--no-quarantine`, so signing and notarization are now required.

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| Rust toolchain | MSRV **1.90**, edition 2024 | Whole workspace | Required by tauri 2.12. Brings std file locking (1.89). | HIGH (crates.io `rust_version` field) |
| `objc2` + `objc2-foundation` | 0.6.4 / 0.3.2 | `NSFileManager.trashItemAtURL` with `resultingItemURL`; `NSURL`, `NSError` | Safe, maintained Apple bindings (40M+ downloads). They are what `trash`, `quicklook` and Tauri's own deps already use, so twins adds no second binding stack. Gives direct access to `resultingItemURL` for the journal. | HIGH |
| `rustix` (feature `fs`) | 1.1.5 | APFS clone: `fclonefileat(src_fd, dst_dir_fd, name, CloneFlags::NOFOLLOW)`, then `renameat` over the duplicate | A **safe** wrapper, so no `unsafe` block (the workspace lints `unsafe_code`). The fd-based form lets twins open the kept file, re-check its (dev, inode, size, mtime) on the open fd, and clone from that same fd. That closes the TOCTOU window the Go path-based `Clonefile` had. | HIGH (verified in rustix source: `#[cfg(apple)] pub fn fclonefileat`) |
| `rusqlite` (feature `bundled`) | 0.40.2 | Hash cache `cache.db`, and run history / agent state `state.db` | Multi-process safe with WAL plus `busy_timeout`, which matters because CLI, app and agent run concurrently. Bundled SQLite means no dependency on the system libsqlite version. A single `WITHOUT ROWID` table keyed by `(dev, ino)` with `size, mtime_ns, partial, full` columns. Warm-scan lookups for 16k files take milliseconds inside one read transaction. | HIGH |
| `toml` | 1.1.6 (TOML spec 1.1) | `config.toml` and workflow files | De-facto serde TOML (238M downloads). 1.x is stable and declares MSRV 1.85, edition 2024. | HIGH |
| `tauri` | 2.12.1 (+ `tauri-build` 2.7.1, `@tauri-apps/cli` 2.12.1, `@tauri-apps/api` 2.12.1) | Desktop shell over `twins-core` | The project already chose it. Keep the Rust crate, CLI and JS API on the same minor version. | HIGH |
| `tauri::ipc::Channel<T>` | built into tauri 2 | Live scan progress and clean progress streaming | The Tauri docs say events are "not designed for low latency or high throughput" and that Channels "are designed to be fast and deliver ordered data". Use one channel per scan session and throttle to roughly 10–20 messages per second from Rust. | HIGH |
| `notify` + `notify-debouncer-full` | 8.2.0 / 0.7.0 | FSEvents folder-change trigger in `twins-agent` | `macos_fsevent` is notify's **default** backend on macOS, using `fsevent-sys`. The debouncer coalesces bursts and tracks renames through `file-id`. The debouncer needs `notify ^8.2.0` and MSRV 1.85. | HIGH |
| `croner` (features `jiff`, `serde`; `default-features = false`) | 4.0.1 | Schedule trigger: cron expression → next fire time | Supports DST and handles `L`/`#`/`W`. 4.x has an optional jiff backend, so it matches the jiff timestamps below. The 4.0.0 major is a month old and 4.0.1 shipped on 2026-10-02, so pin `~4.0`. | MEDIUM |
| `jiff` | 0.2.37 | Timestamps (journal `ts`, run records), time zones, next-run maths | The modern, tz-correct datetime crate, from the author of ripgrep. It avoids pulling in both chrono and time. | HIGH |
| `objc2-service-management` | 0.3.2 | `SMAppService.agentServiceWithPlistName(...)` register/unregister/status from the app | The Apple-sanctioned way (macOS 13+) to run a bundled LaunchAgent. It appears in System Settings → Login Items as "Allow in the Background", attributed to the app. | HIGH (API), MEDIUM (edge cases, see spike) |
| Svelte | 5.57.1 | UI | Runes. Required by `@xyflow/svelte` 1.x (`svelte ^5.25`). | HIGH |
| SvelteKit + `@sveltejs/adapter-static` | 3.0.0 / 4.0.0 | SPA routing in the webview (`ssr = false`, `fallback: 'index.html'`) | This is the path Tauri documents for Svelte. Kit 3 (released 2026-10-01) moves config into the Vite plugin, renames `$lib` to `#lib` and needs Vite 8. Starting on 3 avoids a migration later. See the variant below if a fresh major is too risky. | MEDIUM |
| Vite + `@sveltejs/vite-plugin-svelte` | 8.3.2 / 7.3.1 | Frontend build | Required by Kit 3. | HIGH |
| TypeScript | **6.0.3** (not 7.x) | Type checking | Kit 3 and svelte-check peers stop at `^6`. | HIGH |
| `@xyflow/svelte` (Svelte Flow) | 1.7.0 | Node-graph workflow canvas | The Svelte port of React Flow, maintained by xyflow, the same team that maintains the library n8n-style editors are built on. 1.x is Svelte-5 native. Custom nodes and handles, `isValidConnection` for typed ports, minimap, controls. | HIGH |
| `virtua` | 0.52.10 | Virtualized group browser (thousands of groups, variable-height expandable rows) | Native `virtua/svelte` export with a `svelte >=5.0` peer. Measures dynamic row heights. TanStack's Svelte adapter still has an open "Svelte 5 support" issue (#866). | MEDIUM |
| semantic-release | 25.0.9 | `main` → `x.y.z-beta.n` prerelease, `stable` → release | The user chose it. Use the 25 line; 26 is in beta. It needs Node `^22.14 \|\| >=24.10`. | HIGH |
| tauri-action | v1.0.0 (2026-06-29) | CI build, sign, notarize, upload DMG to the GitHub release | Wraps `tauri build`. Notarization runs automatically when the API-key env vars are set. | HIGH |

### Supporting Libraries

| Library | Version | Purpose | When to Use | Confidence |
|---------|---------|---------|-------------|------------|
| `objc2-user-notifications` | 0.3.2 | `UNUserNotificationCenter` from `twins-agent` (run results, "Review" action) | Agent notifications. **Spike first:** a bare executable in `Contents/MacOS` may not get a bundle identity. If it does not, ship the agent as a nested `LSUIElement` helper `.app` instead. | MEDIUM |
| `tauri-plugin-notification` | 2.5.1 (JS 2.5.1) | Notifications from the GUI app | In-app notifications only. On macOS it goes through `notify-rust` → `mac-notification-sys`, which uses the **deprecated** `NSUserNotification`. That is fine for the app, but do not build the agent on it. | MEDIUM |
| `tauri-plugin-opener` | 2.7.0 (JS 2.7.0) | `revealItemInDir(path)` for "Reveal in Finder"; `openPath` | Group browser actions. Permission `opener:allow-reveal-item-in-dir`. Replaces the old `shell.open`. | HIGH |
| `quicklook` (reference) / `objc2-quick-look-ui` | 0.2.2 / 0.3.2 | Native `QLPreviewPanel` with a data source for Quick Look | Quick Look in the browser. `quicklook` (and `tauri-plugin-quicklook` 0.2.5 on top of it) are tiny projects (about 1.5k and 300 downloads). Read them as a reference and write a ~150-line in-house command on `objc2-quick-look-ui`, or vendor the crate. Fallback: `qlmanage -p <file>`, which works but opens a detached debug panel. | MEDIUM-LOW |
| `tauri-plugin-dialog` | 2.8.1 | Folder pickers for scan roots, keep-dir, workflow scope | App. | HIGH |
| `tauri-plugin-single-instance` | 2.5.2 | Route a second launch (e.g. agent notification click → `open -b`) into the running app | App. Register it **first** among plugins. | HIGH |
| `tauri-plugin-deep-link` | 2.6.1 | `twins://run/<id>` so an agent notification opens the app on that run's results | App, if notification actions open specific runs. | MEDIUM |
| `tauri-plugin-window-state` | 2.5.0 | Remember window size and position | App polish. Optional. | HIGH |
| `tauri-specta` + `specta` | **=2.0.0-rc.25** (pin exact) | Generate TypeScript bindings for commands, events and DTOs from Rust | App IPC typing. It is still a release candidate but is the de-facto Tauri v2 binding generator (590k downloads). Pin with `=` because RCs break between releases. Fallback: `ts-rs` 12.0.1 (types only) with hand-written `invoke` wrappers. | MEDIUM |
| `schemars` | 1.2.2 | JSON Schema for `config.toml` and workflow files | Lets Taplo / "Even Better TOML" validate and autocomplete hand-edited workflows via a `#:schema` header. Also gives the UI a validation source. | HIGH |
| `tracing` + `tracing-subscriber` + `tracing-appender` | 0.1.44 / 0.3.23 / 0.2.5 | Structured diagnostics; the agent logs to `~/Library/Logs/twins/agent.log` (rolling) | Agent and app. Keep this separate from the operations journal, which is a product artefact and not a debug log. | HIGH |
| `dirs` | 7.0.0 | `data_dir()` → `~/Library/Application Support`, `home_dir()` | Config, cache and workflow paths. macOS has no "logs dir" helper, so build `~/Library/Logs/twins` from `home_dir()`. | HIGH |
| `uuid` (v7) or `ulid` | 1.26.1 / 3.0.0 | Run IDs and workflow node IDs (sortable) | Journal `run_id`, workflow node IDs. Pick one; uuid v7 is the more common choice. | HIGH |
| `petgraph` | 0.8.3 | DAG validation (cycle check, toposort) of workflow graphs | Only if the graph grows past a linear trigger → scope → filter → outcome chain. Otherwise a 30-line hand-written toposort is enough. | HIGH (lib) / LOW (need) |
| `@dagrejs/dagre` | 3.1.1 | Auto-layout when a workflow file has no positions, or when the list view creates nodes | Builder. elkjs 0.12 is the heavier alternative. | MEDIUM |
| `cronstrue` | 3.27.0 | Human-readable cron ("At 03:00, every day") in the schedule node and list view | Builder UX. | HIGH |
| shadcn-svelte + bits-ui + Tailwind CSS | 1.7.0 / 2.19.4 / 4.3.3 (`@tailwindcss/vite`) | UI components (dialogs, menus, tables, command palette) | App. Copy-in components, Svelte 5 native, accessible. | MEDIUM (taste) |
| `svelte-sonner` | 1.2.1 | In-app toasts | App. | MEDIUM |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| pnpm 12.x + Node 24 LTS (22.17+ minimum) | Frontend and release tooling | SvelteKit 3 needs Node `>=22.17` and semantic-release 25 needs `^22.14 \|\| >=24.10`. Node 24 satisfies both on CI. Local is 22.22, which is fine. |
| `@tauri-apps/cli` 2.12.1 (npm devDependency) | `pnpm tauri dev/build` | Pin the CLI in `package.json` so CI and local builds use the same bundler. Don't rely on a global `cargo install tauri-cli`. |
| svelte-check 4.7.6, vitest 5.0.3, `@testing-library/svelte` 5.4.2 | Frontend type checking and unit tests | Tauri's WebDriver (`tauri-driver`) does **not** support macOS, so there are no E2E-in-webview tests. Keep logic in `twins-core` and test it there. Mock `invoke` in vitest. |
| Biome 2.5 (optional) | Lint and format TS/Svelte | Optional. Prettier + eslint-plugin-svelte is the conservative choice. |
| `xcrun notarytool`, `codesign`, `xcrun stapler` | Signing and notarization (called by the Tauri bundler) | Use an App Store Connect **API key** (`APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_PATH`) rather than an Apple ID with an app-specific password: keys don't expire with password rotation and don't need 2FA. Certificate: `APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`. |
| Taplo / Even Better TOML | Editing workflow and config TOML | Pair it with the schemars-generated schema. |
| `cargo-edit` (`cargo set-version`) or `sed` | Write the release version into `[workspace.package] version` | Called from `@semantic-release/exec` `prepareCmd`. Leave `version` **out of** `tauri.conf.json` so Tauri reads it from Cargo.toml. |

---

## Installation

```toml
# Cargo.toml — [workspace.package]
rust-version = "1.90"

# [workspace.dependencies] additions
objc2 = "0.6.4"
objc2-foundation = { version = "0.3.2", default-features = false, features = ["std", "NSError", "NSFileManager", "NSString", "NSURL"] }
objc2-service-management = "0.3.2"
objc2-user-notifications = "0.3.2"
objc2-quick-look-ui = "0.3.2"
rustix = { version = "1.1.5", features = ["fs"] }
rusqlite = { version = "0.40.2", features = ["bundled"] }
toml = "1.1.6"
schemars = "1.2.2"
jiff = { version = "0.2.37", features = ["serde"] }
croner = { version = "~4.0.1", default-features = false, features = ["jiff", "serde"] }
notify = "8.2.0"                       # default feature = macos_fsevent
notify-debouncer-full = "0.7.0"
dirs = "7.0.0"
uuid = { version = "1.26.1", features = ["v7", "serde"] }
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.23", features = ["env-filter", "fmt"] }
tracing-appender = "0.2.5"

# crates/twins-app (src-tauri)
tauri = { version = "2.12.1", features = ["tray-icon"] }
tauri-build = "2.7.1"                  # [build-dependencies]
tauri-plugin-opener = "2.7.0"
tauri-plugin-dialog = "2.8.1"
tauri-plugin-notification = "2.5.1"
tauri-plugin-single-instance = "2.5.2"
tauri-plugin-deep-link = "2.6.1"
tauri-plugin-window-state = "2.5.0"
specta = "=2.0.0-rc.25"
tauri-specta = { version = "=2.0.0-rc.25", features = ["derive", "typescript"] }
```

```bash
# Frontend (crates/twins-app/ui or app/)
pnpm add svelte@^5.57.1 @xyflow/svelte@^1.7.0 virtua@^0.52.10 @dagrejs/dagre@^3.1.1 cronstrue@^3.27.0 \
  @tauri-apps/api@~2.12.1 @tauri-apps/plugin-opener@~2.7.0 @tauri-apps/plugin-dialog@~2.8.1 \
  @tauri-apps/plugin-notification@~2.5.1 @tauri-apps/plugin-deep-link@~2.6.1 \
  bits-ui@^2.19.4 svelte-sonner@^1.2.1
pnpm add -D @sveltejs/kit@^3.0.0 @sveltejs/adapter-static@^4.0.0 @sveltejs/vite-plugin-svelte@^7.3.1 \
  vite@^8.3.2 typescript@~6.0.3 svelte-check@^4.7.6 @tauri-apps/cli@~2.12.1 \
  tailwindcss@^4.3.3 @tailwindcss/vite@^4.3.3 vitest@^5.0.3 @testing-library/svelte@^5.4.2

# Release tooling (repo root package.json, devDependencies)
pnpm add -D semantic-release@^25.0.9 @semantic-release/exec@^7.1.0 @semantic-release/github@^12.0.10 \
  @semantic-release/commit-analyzer@^13.0.1 @semantic-release/release-notes-generator@^14.1.1 \
  conventional-changelog-conventionalcommits@^10.4.0
```

```jsonc
// .releaserc.json — branch model (release branch listed first)
{
  "branches": [
    "stable",
    { "name": "main", "prerelease": "beta", "channel": "beta" }
  ],
  "plugins": [
    ["@semantic-release/commit-analyzer", { "preset": "conventionalcommits" }],
    ["@semantic-release/release-notes-generator", { "preset": "conventionalcommits" }],
    ["@semantic-release/exec", { "prepareCmd": "scripts/set-version.sh ${nextRelease.version}" }],
    "@semantic-release/github"
  ]
}
```

Recommended pipeline: **job 1** runs semantic-release, which computes the version and creates the tag and GitHub release (a prerelease on `main`). **Job 2** runs on a macOS runner from that tag: it builds the universal app, then signs, notarizes and staples it via tauri-action/`tauri build`, and uploads the DMG to the release. **Job 3** runs on `stable` only: it renders `Casks/twins.rb` (version + sha256) and pushes it to `ayhid/homebrew-tap`. Do not commit version bumps back to `main` with `@semantic-release/git`. The tag is the source of truth, and commit-backs fight branch protection and trigger re-runs.

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Direct `objc2-foundation` `trashItemAtURL` | `trash` 5.2.9 crate | Only if twins went cross-platform (it won't). Even then, set `DeleteMethod::NsFileManager` explicitly, and accept that you lose `resultingItemURL`. |
| Direct `trashItemAtURL` + journal undo | Finder via AppleScript (`tell application "Finder" to delete {…}`) | If full Finder Put Back for every file turns into a hard requirement and the `.DS_Store` repair approach is rejected. Costs: Automation (TCC) prompt, Finder must be running, sound, slower. Can be acceptable for interactive app cleans but is bad for the background agent. |
| `rusqlite` (SQLite WAL) | `redb` 4.3.0 | Single-process access only. redb is pure Rust, ACID and the closest analogue to the Go v0's bbolt. Revisit once redb's multi-process mode leaves the experimental flag. |
| `rusqlite` | `fjall` 3.1 (LSM) | Write-heavy workloads with huge key counts. Overkill here, and it has the same single-process concern. |
| `rustix::fs::fclonefileat` | `libc::clonefile` (0.2.189) | Equivalent behaviour but needs `unsafe` and a CString. Fine inside one `#[allow(unsafe_code)]` function if you'd rather not add rustix. |
| `croner` 4 | `cron` 0.17 | If you'd rather stay on chrono and a long-lived 0.x API. Its cron dialect is less complete (no `L`/`#`). |
| Hand-merged config (`Option` fields: file < flags) | `figment` 0.10.19 | figment's last release was 2024-05. Layering two sources (file, flags) doesn't justify a framework. |
| SvelteKit 3 SPA | SvelteKit 2.70.3 + adapter-static 3.0.10 | If Kit 3 (one day old) shows ecosystem breakage when step 4 starts. Kit 2 also accepts Vite 8, svelte-plugin 7 and TS 6. `npx sv migrate sveltekit-3` exists for later. |
| SvelteKit SPA | Plain Svelte 5 + Vite (no Kit) | If the app ends up with only two or three views. You lose file-based routing but drop the Kit config surface. |
| `@xyflow/svelte` | Svelvet 11.0.5 | Don't. Last published 2025-02 and far less maintained. |
| `virtua` | `@tanstack/svelte-virtual` 3.13.39 | If you're already on TanStack Table. Watch issue #866 for Svelte 5 runes support. |
| `SMAppService.agent` + headless `twins-agent` | GUI app as a menu-bar resident launched at login (`tauri-plugin-autostart`) | Faster to ship: one binary, and in-app notifications just work. But workflows stop when the user quits the app, and it keeps a webview process around. Acceptable only as a stop-gap. |
| `SMAppService.agent` | Plists written to `~/Library/LaunchAgents` + `launchctl bootstrap gui/$UID` | macOS < 13 (out of scope), or if you need per-workflow `StartCalendarInterval`/`WatchPaths` jobs. SMAppService plists must be bundled at build time, so they can't be created per workflow. That is why twins runs one agent with its own scheduler. |
| tauri-action v1 | Plain `pnpm tauri build --target universal-apple-darwin` on `macos-15`/`macos-26` runners | Equivalent; tauri-action just adds release upload. Either is fine. |
| `@semantic-release/exec` scripts | `semantic-release-cargo` 2.4.x | It's built for publishing crates to crates.io. twins doesn't publish crates, and a workspace-version `sed`/`cargo set-version` is simpler. |
| Own tap `ayhid/homebrew-tap` | Submitting to `homebrew/cask` | Later, once the app has notability. homebrew/cask has review overhead and notability thresholds. |
| App Store Connect API key for notarization | Apple ID + app-specific password (`APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID`) | Only if you cannot create an API key. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `trash` crate defaults on macOS | The default `DeleteMethod::Finder` shells out to `osascript`, triggers the Automation TCC prompt, plays the trash sound and is slow. The `NsFileManager` mode drops `resultingItemURL`. | `objc2-foundation` `NSFileManager::trashItemAtURL_resultingItemURL_error` with a real out-param |
| `rename()` into `~/.Trash` | Writes no Put Back metadata, doesn't handle per-volume `.Trashes/<uid>` and has collision bugs | `trashItemAtURL` |
| Relying on Finder **Put Back** as the undo path | NSFileManager writes Put Back records reliably only for the first item of a batch. The bug has been open since 2015 (rdar 23153124) and the trash crate documents Put Back as ✗. | Journal `original_path` + `trashed_to`, plus `twins undo <run-id>`. Optionally repair `.DS_Store` records (see variants). |
| `reflink-copy` / `reflink` crates for link mode | Both **silently fall back to a full copy** when cloning fails (that is their design). In link mode that turns a space saving into a no-op or doubles the bytes, and hides cross-volume errors. | `rustix::fs::fclonefileat` with an explicit cross-device check, then atomic rename |
| `redb` (now), `sled` 0.34.7 | redb: exclusive per-process writer (multi-process is still experimental). sled: unmaintained beta since 2021, with an unstable on-disk format. | `rusqlite` + WAL |
| `serde_yaml` 0.9.34+deprecated, `serde_yml` | serde_yaml is officially deprecated and serde_yml has had quality and governance concerns. YAML is the wrong format for this project anyway. | TOML for workflows (and JSON for the `report` output) |
| `fs2`, `fd-lock`, `fs4` | Redundant once MSRV is 1.90: `std::fs::File::lock`/`try_lock` are stable since 1.89 | std file locks |
| `tauri-plugin-autostart` for the agent | Writes its own plist into `~/Library/LaunchAgents` (or uses AppleScript login items) and launches the **GUI** app. It doesn't expose SMAppService even though `auto-launch 0.6` supports it. | `objc2-service-management` `SMAppService` |
| `tauri-plugin-shell` for opening/revealing | Shell `open` was moved to the opener plugin in v2. The shell plugin is for spawning processes and widens the capability surface. | `tauri-plugin-opener` (`revealItemInDir`, `openPath`) |
| `tauri-plugin-store` for settings | Would create a second source of truth next to `config.toml`, which the CLI and agent must also read | Tauri commands that read and write `config.toml` through `twins-core::config` |
| `tauri-plugin-fs` exposed to the webview | Gives the webview direct filesystem power in a tool whose core value is "never lose data". All file operations must go through the validated core path. | Narrow, purpose-built Tauri commands |
| `tauri-plugin-updater` | The Homebrew cask owns updates (`brew upgrade`). Two update channels fight each other and complicate notarized bundle replacement. | Cask with `auto_updates false` (the default) |
| Tauri events for progress | Events are JSON-string, not ordered or high-throughput by design | `tauri::ipc::Channel<T>` |
| TypeScript 7.x | SvelteKit 3 and svelte-check peers cap at TS 6 | `typescript@~6.0.3` |
| `mac-notification-sys` / `notify-rust` in the agent | `NSUserNotification` has been deprecated since macOS 11 and fakes the bundle ID | `objc2-user-notifications` (`UNUserNotificationCenter`) under the app's bundle identity |
| `apple-codesign` (`rcodesign`) 0.29 | Last release 2024-11. Only useful for signing on Linux, and twins builds on macOS runners anyway. | Native `codesign`/`notarytool` via the Tauri bundler |
| App Sandbox entitlement | Breaks arbitrary-path scanning, the Trash on other volumes and FDA flows. It isn't needed for Developer ID distribution. | Hardened runtime (Tauri enables it by default when signing), no sandbox |

---

## Stack Patterns by Variant

**If Finder "Put Back" must work for every trashed file (not just the first in a batch):**
- After a batch of `trashItemAtURL` calls, group the results by trash folder (`resultingItemURL.parent`). Then add any missing `ptbN` (original filename) and `ptbL` (original directory, no leading slash) string records to that folder's `.DS_Store`.
- This is the approach of `sindresorhus/macos-trash` v3.0+ (Jan 2026, built on its Swift `DSStore` package). In Rust, evaluate `ds_parser` 0.4.0 (a parser and writer from 2026-07 with low adoption) or port the DSStore B-tree writer.
- Because `.DS_Store` is an undocumented Finder-owned format and Finder writes it concurrently: do it best-effort, never fail a clean on it, and keep the journal as the real undo. **Phase-2 spike.**

**If the agent cannot post notifications as a bare `Contents/MacOS/twins-agent`:**
- Package it as a nested helper `Contents/Library/LoginItems/TwinsAgent.app` (`LSUIElement = true`, its own bundle ID). Put it in place via `bundle.macOS.files`. The bundler's nested-code walker signs `.app` bundles found under nested code folders.
- Register it with `SMAppService.loginItem(identifier:)`, or keep using `.agent` with `BundleProgram` pointing into the helper.

**If SvelteKit 3 is unstable when step 4 starts:**
- Use `@sveltejs/kit@~2.70.3` + `@sveltejs/adapter-static@^3.0.10`. Every other version in this file stays the same.

**If Intel support is dropped:**
- Build `aarch64-apple-darwin` only. That halves CI time, and Apple silicon requires a valid signature anyway. macOS 26 (Tahoe) is the last release with Intel support, so keep `universal-apple-darwin` for now.

---

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `tauri@2.12.1` | Rust ≥ 1.90 | **Breaks the current `rust-version = "1.85"`.** Bump the workspace. |
| `tauri@2.12.x` | `tauri-plugin-*@2.4–2.13` (all depend on `tauri ^2.12`), `@tauri-apps/api@2.12.1`, `@tauri-apps/cli@2.12.1` | Keep the JS plugin packages on the same minor as their crates (opener 2.7.0 ↔ 2.7.0, etc.). |
| `tauri-specta@=2.0.0-rc.25` | `specta@=2.0.0-rc.25`, `tauri ^2` | Pin both exactly. They move in lockstep. |
| `@sveltejs/kit@3.0.0` | `vite ^8.0.12`, `svelte ^5.57.1`, `typescript ^6`, `@sveltejs/vite-plugin-svelte ^7`, Node ≥ 22.17 | Config lives in `vite.config.ts`. `$lib` → `#lib/*.js`. `$app/environment` → `$app/env`. |
| `@sveltejs/adapter-static@4.0.0` | `@sveltejs/kit ^3` | Use 3.0.10 with Kit 2. |
| `@xyflow/svelte@1.7.0` | `svelte ^5.25` | Runes API. Svelte 4 is not supported. |
| `notify-debouncer-full@0.7.0` | `notify ^8.2.0`, MSRV 1.85 | |
| `croner@4.0.x` | `jiff ^0.2.35` (feature `jiff`) | `default-features = false` drops chrono. |
| `objc2-foundation@0.3.2` / `objc2-*@0.3.2` | `objc2 ^0.6` | All `objc2-*` framework crates share the 0.3.2 generation, so keep them aligned. |
| `rusqlite@0.40.2` (`bundled`) | Compiles SQLite from C via the `cc` crate | Adds about 1 MB and some C compile time. That is fine on macOS runners (Xcode CLT present). |
| `semantic-release@25.0.9` | Node `^22.14.0 \|\| >=24.10.0`; `@semantic-release/exec@7` (peer `>=24.1`) | 26.0.0 is beta, so stay on 25. |
| Tauri `version` (from Cargo, e.g. `1.4.0-beta.3`) | `CFBundleShortVersionString` / `CFBundleVersion` | The bundler writes the full semver, prerelease included, into both keys unless `bundle.macOS.bundleVersion` is set. Set `bundleVersion` to a monotonically increasing build number (CI run number) so Launch Services and Gatekeeper ordering stay sane across betas. |
| Bundle layout | `bundle.externalBin` → `Contents/MacOS/<name>` (sidecar must be named `<name>-<target-triple>`, e.g. `twins-agent-universal-apple-darwin`); `bundle.macOS.files` → arbitrary paths under `Contents/` (use for `Library/LaunchAgents/<id>.agent.plist`) | Verified in the tauri-bundler 2.10.1 source: external binaries are **signed inside-out before the app**, then the app is notarized and stapled automatically when notarization env vars are present. |
| Homebrew cask | Must pass Gatekeeper (Homebrew policy, enforced 2026-09-01) | The cask can expose the CLI with `binary "#{appdir}/Twins.app/Contents/MacOS/twins"`, so `brew install --cask twins` installs both the app and the `twins` command. |
| macOS minimum | 13.0 (`bundle.macOS.minimumSystemVersion`) | Required by `SMAppService`. The Tauri default is 10.13, so set it explicitly. |

---

## Spikes to schedule (stack-level unknowns)

| Spike | Phase | Question | Fallback |
|-------|-------|----------|----------|
| Batch trash + Put Back on macOS 26 | Step 2 (deletion) | How many of N consecutive `trashItemAtURL` calls in one process get `ptbL`/`ptbN`? Is the `.DS_Store` repair safe while Finder is open? Does `trashItemAtURL` need a particular thread? | Journal-based `twins undo` as the contract, Put Back best-effort |
| Agent identity | End of step 4, before workflows | From a signed and notarized bundle, can `Contents/MacOS/twins-agent`, registered via `SMAppService.agent`, (a) post `UNUserNotificationCenter` notifications and (b) inherit or obtain Full Disk Access? | Nested helper `.app` |
| Quick Look panel from a Tauri window | Step 4 | Does `QLPreviewPanel.sharedPreviewPanel` with a custom data source work without being in the AppKit responder chain of the WKWebView window? | `qlmanage -p` |
| SvelteKit 3 + Tauri | Start of step 4 | Does `adapter-static` 4 SPA output plus the Tauri dev server work cleanly? | Kit 2.70.x |

---

## Sources

Registry and source evidence (official, HIGH):
- crates.io API (`/api/v1/crates/<name>`), queried 2026-10-02: every Rust version and MSRV in this file (tauri 2.12.1 `rust_version` 1.90; toml 1.1.6 MSRV 1.85; notify 8.2.0; notify-debouncer-full 0.7.0; rusqlite 0.40.2; redb 4.3.0; croner 4.0.1; objc2 0.6.4; objc2-* 0.3.2; rustix 1.1.5; specta/tauri-specta 2.0.0-rc.25; trash 5.2.9; and others)
- npm registry (`npm view`), queried 2026-10-02: svelte 5.57.1, @sveltejs/kit 3.0.0 (peer deps), adapter-static 4.0.0, vite 8.3.2, @xyflow/svelte 1.7.0, virtua 0.52.10, semantic-release 25.0.9 (engines), typescript 7.0.2 / 6.0.3, @tauri-apps/* 2.x
- `trash` 5.2.9 source, `src/macos/mod.rs`: DeleteMethod table (Put Back ✓ Finder / ✗ NsFileManager; Finder is the default; `resultingItemURL` passed as `None`)
- `sindresorhus/macos-trash` `Sources/trash/main.swift` and commit "Fix 'Put Back' not working for all files when trashing multiple files" (2026-01-25); issue #4 (closed 2026-02-25)
- `redb` 4.3.0 source, `src/db.rs`: `ConcurrencyMode::ExclusiveWriter` default, multi-process only behind `experimental-multiprocess`; redb CHANGELOG (5.0.0 unreleased)
- `auto-launch` 0.6.0 and `tauri-plugin-autostart` 2.7.0 source: SMAppService mode exists in auto-launch but the plugin only exposes LaunchAgent/AppleScript
- `tauri-plugin-notification` 2.5.1 → `notify-rust` 4.18.1 → `mac-notification-sys` 0.6.15 (`NSUserNotification`)
- `tauri-bundler` 2.10.1 source, `src/bundle/macos/app.rs`: externalBin copied and signed inside-out, automatic notarize and staple, nested `.app`/`.xpc` signing; Info.plist version keys
- `rustix` 1.1.5 `src/fs/at.rs`: `#[cfg(apple)] fclonefileat`; `libc` 0.2.189 apple: `clonefile`, `renamex_np`, `RENAME_SWAP`
- Tauri docs: https://v2.tauri.app/develop/calling-frontend/ (Channels vs events), https://v2.tauri.app/distribute/sign/macos/ (signing and notarization env vars), https://v2.tauri.app/distribute/macos-application-bundle/ (`bundle.macOS.files`, entitlements, minimumSystemVersion), https://v2.tauri.app/plugin/opener/ (`allow-reveal-item-in-dir`)
- Rust 1.89.0 release notes (File::lock family stabilized): https://blog.rust-lang.org/2025/08/07/Rust-1.89.0/

Secondary (MEDIUM; cross-checked against registry data where possible):
- Homebrew `--no-quarantine` removal and Gatekeeper enforcement: https://github.com/Homebrew/brew/issues/20755
- SvelteKit 3 migration guide: https://svelte.dev/docs/kit/migrating-to-sveltekit-3 ; InfoQ coverage: https://www.infoq.com/news/2026/09/sveltekit-3-vite/
- macOS 15+ built-in `/usr/bin/trash` (man page "First appeared in macOS 15.0", checked locally on macOS 26.6.2). macos-trash README notes third-party tools exist partly because the built-in one doesn't fix Put Back: https://github.com/sindresorhus/macos-trash
- TanStack Virtual "Svelte 5 support" issue #866 (open)
- GitHub releases: tauri-apps/tauri-action v1.0.0 (2026-06-29); semantic-release v26.0.0-beta.2 (prerelease)

---
*Stack research for: macOS duplicate-file finder (Rust core/CLI + Tauri v2/Svelte app + workflow automation)*
*Researched: 2026-10-02*
