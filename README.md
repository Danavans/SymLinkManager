# Symlink Manager

Symlink Manager is a portable desktop app for backing up and recreating symbolic links.

It is useful when you move a media library, migrate between drives, rebuild a machine, or need to recreate a group of links on another Windows or Linux setup. The app scans a folder, finds symlinks, exports them to JSON, then recreates them later with optional root remapping.

## What It Does

- Scans a folder tree and finds symbolic links.
- Shows whether each target is OK, Broken, or Unreadable.
- Exports the scan result to a JSON file.
- Imports a previous JSON export.
- Recreates symlinks in another destination folder.
- Remaps target roots, for example from a Windows drive to a Linux mount path.
- Warns before replacing existing items.
- Requests admin elevation on Windows when symlink creation needs it.

## Portable App

Symlink Manager is meant to be used as a portable app.

Download the release file for your operating system, place it wherever you want, and launch it directly. No project setup is required to use the app.

## Supported Platforms

- Windows: tested and supported.
- Linux: tested and supported.
- macOS: not tested.

On Windows, creating symlinks may require Developer Mode or administrator rights. If Windows blocks symlink creation, the app can ask for elevation.

On Linux, the app needs normal write permissions in the destination folder. If the Linux file does not start by double-clicking, open its file properties and allow it to run as a program.

## How To Use

### Scan And Export

1. Open the app.
2. Go to Scan & Export.
3. Choose the folder you want to scan.
4. Click Scan symlinks.
5. Review the results.
6. Use the search field if you only want to export part of the scan.
7. Click Export JSON and save the file.

The JSON export is your symlink backup. Keep it somewhere safe if you plan to rebuild or move a library later.

### Import And Recreate

1. Go to Import & Recreate.
2. Load a JSON export.
3. Choose the target root where the links should be created.
4. Add root remap rules if paths changed.
5. Check the preview.
6. Click Recreate.
7. Confirm if the app warns that existing items will be replaced.

The app must run on the operating system where you want to create the links. Use the Windows version to create Windows symlinks, and the Linux version to create Linux symlinks.

## Root Remap Rules

Root remap rules replace the beginning of target paths before links are recreated.

Example:

```text
W:\Shows -> /mnt/media/shows
D:\Media -> /mnt/media
```

The first matching rule wins. If no rule matches, the original target path is used.

## Search Filter

The scan results can be filtered before export.

- Use multiple terms separated by commas.
- Prefix a term with `-` to exclude it.
- If the search field is empty, the full scan is exported.

Example:

```text
arcane, -1080p
```

## Status Meanings

- OK: the symlink target exists.
- Broken: the symlink target is missing.
- Unreadable: the target exists, but the app cannot access it.
- Skipped: some folders or files could not be read during the scan.

## Release Files

For a normal release, upload one Windows build and one Linux build.

Recommended names:

- `Symlink-Manager-v1.0.0-Windows-x64.exe`
- `Symlink-Manager-v1.0.0-Linux-x64`

If you package the Linux build as an archive, use a clear name such as:

- `Symlink-Manager-v1.0.0-Linux-x64.tar.gz`

## Notes

Symlink Manager only manages symbolic links. It does not copy the real target files.

Before recreating links into an important folder, check the preview and replacement warning carefully.

## Redesigned workspace and verification

The workspace now includes clickable health filters, 100-row pages, a guided import flow and one consistent connection logo. Export includes all matching rows across pages. Incomplete remap rules must be completed or removed before recreation. Existing items are preserved until replacement succeeds; non-empty directories and destinations beneath linked ancestors are refused.

Scans use bounded background workers. See [AUDIT.md](AUDIT.md) for measured results, reproducible tests and known limitations. UI-only fixture: `node node_modules/vite/bin/vite.js --mode test --port 1422`. This mode uses synthetic data and does not modify files.
