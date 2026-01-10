# SymLinkManager

Portable desktop app to scan, export, and recreate symlinks across Windows and Linux with bulk root remapping. Built with Tauri + Svelte (PlexTools dark UI).

## Features
- Scan a folder tree for symlinks (OK/Broken).
- Export results to JSON.
- Import JSON and recreate symlinks at a new destination root.
- Root remap rules to swap target prefixes (Z:, X:, etc.).
- Windows admin elevation prompt when needed.
- Live search with multi-term includes and exclusions.

## Quick start (dev)
```
npm install
npm run tauri dev
```

## Usage
1) Scan your library root (example: `D:\Medias\Streaming`).
2) Export to JSON.
3) On the target machine, load the JSON.
4) Set the destination root (where symlinks will be created).
5) Add root remap rules for `target` paths.
6) Preview and recreate.

### Root remap rules
Rules replace the start of each `target` path.

Example:
```
Z:\magnets -> /mnt/webdav/alldebrid/magnets
W:\shows   -> /mnt/media/shows
```

### Search filter (scan results)
- Multiple terms separated by commas: `arcane, game of thrones`
- Exclude terms with `-`: `arcane, -1080p`
- Export respects the current filter (empty filter exports everything).

## Notes
- On Windows, symlink creation requires Developer Mode or admin rights.
- On Linux, you need write permissions to the destination root.

## Build (portable)
```
npm run tauri build
```
Windows exe will be in `src-tauri/target/release/`.

## Project context
See `PROJECT_CONTEXT.md` for decisions, structure, and workflow details.
