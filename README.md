# Symlink Manager

Portable desktop app to scan, export, and recreate symlinks across Windows and Linux with bulk root remapping. Built with Tauri + Svelte and a dark UI.

## Features
- Click detected roots to auto-fill remap rules.
- Scan a folder tree for symlinks with status: OK, Broken, Unreadable.
- Export results to JSON (respects current search filter).
- Import JSON and recreate symlinks at a new destination root.
- Root remap rules to swap target prefixes (Windows and Linux).
- Auto preview refresh when remap rules change.
- Windows admin elevation flow when needed.
- Warnings before replacing existing items.
- Skipped count when scan hits unreadable folders.

## Quick start (dev)
```bash
npm install
npm run tauri dev
```

## Usage
### Scan & Export
1) Choose a folder to scan.
2) Click "Scan symlinks".
3) Review results, filter if needed.
4) Click "Export JSON".

### Import & Recreate
1) Load a JSON export.
2) Set the destination root (where symlinks will be created).
3) Add root remap rules as needed.
4) Click a detected root to auto-fill a remap rule if needed.
5) Review the preview (auto refreshes as you type).
6) Click "Recreate" and confirm if items will be replaced.

### Root remap rules
Rules replace the start of each `target` path. The first rule that matches wins.

Example:
```
W:\Shows -> /mnt/media/shows
D:\Media -> /mnt/media
```

### Search filter (scan results)
- Multiple terms separated by commas: `arcane, game of thrones`
- Exclude terms with `-`: `arcane, -1080p`
- Export respects the current filter (empty filter exports everything).

## Status meanings
- OK: target exists.
- Broken: target is missing.
- Unreadable: target exists but cannot be accessed or read.

## Notes
- Windows symlink creation requires Developer Mode or admin rights.
- Linux requires write permissions to the destination root.
- The app must run on the OS that will create the symlinks.

## Build (portable)
```bash
npm run tauri build
```
Windows exe will be in `src-tauri/target/release/`.

## FAQ
**Why do I see "Skipped" in scan results?**
Some folders or files could not be read during the scan (permissions or read errors). Those entries are counted as skipped.

**What is the difference between Broken and Unreadable?**
Broken means the target is missing. Unreadable means the target exists but cannot be accessed or read.

**Why do I get an admin prompt on Windows?**
Creating symlinks requires elevated privileges unless Developer Mode is enabled.

**Can I recreate Linux symlinks from Windows?**
No. The app uses the OS filesystem APIs, so it must run on the target OS (Linux to create Linux symlinks, Windows for Windows symlinks).

**Do remap rules update the preview automatically?**
Yes. The preview refreshes as you edit remap rules or destination root.
