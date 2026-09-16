# Symlink Manager

**Version 1.1.0** · Portable desktop application for Windows and Linux

Scan, inspect, export and recreate symbolic links when moving a library, changing drives or rebuilding a machine. Symlink Manager preserves connections and folder structure; it does not copy the real target files.

## Workspace

- Graphite and mint interface with sidebar navigation, a consistent connection logo and layouts that adapt to smaller windows.
- **Scan & export**: clickable health summaries, searchable and sortable inventory, 100-row pages, scan duration and skipped-entry count.
- **Import & recreate**: guided snapshot loading, destination selection and optional target remapping, with an automatically updated preview.
- Clear replacement confirmations, operation results and details for partial failures. Dialogs support keyboard focus and Escape; status remains visible in the bottom bar.
- Local processing and system fonts, with no remote font downloads.

## Scan and export

1. Open **Scan & export**, choose a folder and click **Scan symlinks**.
2. Review the health summaries and inventory. Subfolders are scanned; discovered linked folders are not traversed.
3. Optionally select a health filter and search paths, targets or status.
4. Click **Export JSON** to save every matching entry across all pages.

Search is case-insensitive. Separate alternatives with commas (OR), and prefix exclusions with `-`, for example `arcane, -1080p`. To export the entire scan, clear the search and select **Total links**. Pagination does not limit the export.

| Status | Meaning |
| --- | --- |
| OK / Healthy | The target is accessible. |
| Broken | The target was not found. |
| Unreadable | The link could not be read, or its target could not be verified. This does not establish that the target exists. |
| Skipped | A traversal error prevented reading part of the scanned tree. This is a separate counter. |

The scan runs in the background with up to eight workers and reuses target metadata for status and type. The interface renders at most 100 result rows at a time. A local benchmark of 3,000 NTFS junctions measured approximately **2.4× faster scanning**; network and cold-disk performance can differ. Methodology and limitations are in [AUDIT.md](AUDIT.md).

## Import and recreate

1. Open **Import & recreate** and choose a JSON export.
2. Set the destination folder where the new links will be created.
3. Add target-prefix rules if locations changed. Both fields of each rule must be completed, or the unfinished rule removed.
4. Review the automatic preview. Click a detected root to prefill a mapping rule; the preview shows one example per root.
5. Click **Recreate links**, confirm any replacements and review the result.

The original relative folder structure is preserved. Root mappings change target paths, not link locations. The first matching rule wins, and matching respects path-component boundaries. Windows matching ignores case and accepts either slash style.

```text
W:\Shows -> E:\Media\Shows
W:\Shows -> /mnt/media/shows
```

After explicit rules, targets inside the original scan root are also relocated to the new destination. Other unmatched targets remain unchanged. Recreate links on the operating system where they will be used, with mappings appropriate to that system.

### Replacement and validation

Imported paths are checked for traversal, absolute link paths, duplicates and platform-specific unsafe names. Destinations beneath linked ancestors are refused. Before replacing an existing item, the app temporarily preserves it and restores it if link creation fails. Non-empty real directories are never replaced. Entries whose original target could not be read are reported as failures rather than recreated with a placeholder target.

These safeguards cover normal failures; they are not a crash-recovery journal or a guarantee against concurrent external filesystem changes. Review the preview and replacement warning before modifying an important folder.

## Platforms and portability

- **Windows**: the current refactor has been compiled and tested at the code level. Symlink creation may require Developer Mode or administrator privileges; error 1314 triggers the UAC recreation flow.
- **Linux**: supported by the implementation; native validation of this refactor is still pending. Write permissions and executable permissions are required. Tauri's platform runtime prerequisites still apply.
- **macOS**: not tested.

The latest checks include browser UI fixtures and real NTFS junction scans. Full native UAC and Linux validation remain follow-up work. Elevated jobs are polled for up to 60 seconds; after a timeout, the worker may still be running, so check the destination before retrying.

Release executables are intended to run without project setup or an installer. The Windows build uses WebView2. Suggested release filenames:

- `Symlink-Manager-v1.1.0-Windows-x64.exe`
- `Symlink-Manager-v1.1.0-Linux-x64`
- `Symlink-Manager-v1.1.0-Linux-x64.tar.gz` if distributing a Linux archive.

These names are packaging guidance, not a claim that all platform releases have been built or published.

## Development and verification

Use Node.js/npm, Rust and the Tauri prerequisites for your platform.

```sh
npm ci
npm run tauri dev
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --lib -- -D warnings
npm run tauri build
```

The portable Windows output is `src-tauri/target/release/symlinkmanager.exe`; installers are disabled in the Tauri configuration. Version changes take effect in the executable after rebuilding.

Optional checks:

- `./tests/benchmark.ps1` in PowerShell 7 on Windows creates and removes a disposable 3,000-junction fixture.
- `node node_modules/vite/bin/vite.js --mode test --port 1422` starts a UI-only fixture with synthetic data and no filesystem operations. It is excluded from production builds.

See [PROJECT_CONTEXT.md](PROJECT_CONTEXT.md) for architecture and compatibility details, [AGENTS.md](AGENTS.md) for contributor instructions, and [AUDIT.md](AUDIT.md) for the detailed audit and remaining priorities.
