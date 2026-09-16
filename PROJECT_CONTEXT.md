# Symlink Manager — Project Context

## Product
Portable Tauri + Svelte desktop application to scan, export and recreate symbolic links on Windows/Linux. UI text is English. No extra runtime service, build stage or database.

Read AGENTS.md for operational rules. It takes precedence over this document.

## Current UI (2026-09-17)
Graphite dark surfaces and mint accents; rose = broken, amber = unreadable. Sidebar with Scan & export and Import & recreate. The scan workspace includes health filters, comma-separated OR search and `-term` exclusions, sortable inventory, 100-row pagination and filtered export across all pages. Import is a three-step flow with automatic preview, detected roots, editable mapping rules and replacement confirmation. Native HTML dialogs contain focus. Status is visible in the bottom bar. Narrow windows reflow navigation and panels.

`assets/icon.svg` is the brand source; `static/logo.svg`, favicon and all Tauri platform/window icons use the same mark. Generate icons using the existing Tauri CLI when changing the mark. Fonts are local/system only.

## Data compatibility
```json
{
  "src_root": "D:\\Media\\Library",
  "entries": [
    { "relative": "Series\\Show\\link.mkv", "target": "W:\\Shows\\Show\\file.mkv", "status": "OK", "link_is_dir": false }
  ]
}
```
Statuses: OK, Broken (target not found), Unreadable (target cannot be verified or link cannot be read). Skipped counts traversal errors. Scan resolves the selected root to an absolute path and never descends into discovered linked folders.

Root mappings affect targets only; first match wins, path boundaries matter, slash normalization and case-insensitive matching apply on Windows. Targets inside src_root also follow dst_root after explicit mappings. Preserve this historical behavior.

## Safety and execution
Scan and bulk filesystem operations run off the main thread. Scan uses up to eight standard-library workers and one metadata read per accessible target. Replacement temporarily preserves an existing item and restores it if symlink creation fails; non-empty real directories are never replaced. Imported relative paths are validated and linked destination ancestors rejected. This does not promise immunity to concurrent external filesystem mutation or process crashes.

Windows error 1314 triggers the existing UAC worker via --admin-recreate. The worker reads a temporary job and publishes a result including failure details. Job IDs are validated, jobs use exclusive creation, and result publication uses rename. Polling remains 60 seconds; see AUDIT.md for remaining limitations.

## Main files
- src/routes/+page.svelte: UI and interaction state.
- src-tauri/src/lib.rs: commands, scan, remap, recreation and elevation.
- src-tauri/src/main.rs: elevated-worker entry point.
- src-tauri/tauri.conf.json: portable bundle settings, window and CSP.
- src-tauri/src/tests.rs and scan_baseline.rs: checks and before/after scan benchmark (test-only).
- src/lib/ui-fixture.js: explicit Vite test-mode IPC fixture; excluded from production.
- tests/benchmark.ps1: disposable Windows NTFS junction fixture.
- AUDIT.md: findings, measurements, verification and follow-up priorities.

## Commands
`npm run check`, `npm run build`, `npm run tauri dev`, `npm run tauri build`.
Rust checks: `cargo test --manifest-path src-tauri/Cargo.toml --lib`.
Benchmark: `./tests/benchmark.ps1` in PowerShell 7, or the ignored Rust benchmark where symlink privileges are available.
Portable Windows executable: src-tauri/target/release/symlinkmanager.exe.
