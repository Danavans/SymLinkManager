<div align="center">

<img src="assets/icon.svg" width="96" alt="Symlink Manager logo">

# Symlink Manager

**A portable desktop tool for scanning, monitoring, exporting, and recreating symbolic links.**

Quickly inspect large symlink collections, find broken links, preserve existing structures, and recreate them across drives, systems, or platforms.

[**Download latest release**](https://github.com/Danavans/SymLinkManager/releases/latest)

</div>

---

<p align="center">
  <img src="assets/screenshot-scan.png" alt="Symlink Manager scan workspace" width="900">
</p>

## What is Symlink Manager?

Symlink Manager is a lightweight desktop tool built around two main use cases: checking symbolic links in everyday use and preserving them when storage or systems change.

As a scanner, it can recursively inspect large folder structures and quickly show which symbolic links are still valid, which are broken, and where each one points. This makes it easy to check the health of a large symlink collection, identify missing targets, or find a specific link without manually browsing through folders.

It can also export the complete symlink structure to a portable JSON snapshot. That snapshot can later be imported to recreate the links somewhere else, making migrations between drives, mount points, machines, Windows, and Linux much easier.

Symlink Manager only manages the links themselves. It never copies or moves the target files.

## Scan and inspect

Select a folder and Symlink Manager recursively discovers the symbolic links inside it and checks their targets.

Each link is classified as:

- **Healthy** — the target is accessible.
- **Broken** — the target no longer exists.
- **Unreadable** — the link or target could not be verified.

This makes it easy to scan large symlink collections and immediately see which links are still valid and which ones need attention.

Search and filters can also be used to quickly find a specific link, path, target, or status without browsing through the entire folder structure manually.

Results are displayed in compact paginated pages, while scanning runs in the background to keep the interface responsive even with large libraries.

Search supports comma-separated terms and `-term` exclusions.

## Export and preserve

Once a scan is complete, the result can be exported to a JSON snapshot containing the link locations and their targets.

An exported snapshot acts as a portable record of your symlink structure and can be useful before:

- reinstalling or rebuilding a machine;
- moving a library to another drive;
- changing drive letters or mount points;
- migrating between Windows and Linux;
- reorganizing storage while keeping the same link layout.

The exported file contains the information required to recreate the links later without copying the files they point to.

## Import and recreate

Load a previous export, choose where the links should be recreated, and review the resulting paths before applying any changes.

Optional root mappings can be used when the target location has changed.

For example:

`W:\Shows`

can be remapped to:

`/mnt/media/shows`

This also makes snapshots useful when moving between different operating systems or storage layouts.

Before recreating anything, Symlink Manager validates the imported paths and displays a preview of the resulting links.

Existing items that need to be replaced are handled carefully: the original item is temporarily preserved and restored if link creation fails. Unrelated files and directories are left untouched.

<p align="center">
  <img src="assets/screenshot-import.png" alt="Symlink Manager import workspace" width="900">
</p>

## Portable by design

Symlink Manager does not require a traditional installer.

The application can be kept as a standalone executable and launched wherever you need it.

On Windows, recreating symbolic links requires administrator privileges. Symlink Manager will request elevation through the standard Windows UAC prompt when needed.

On Linux, normal filesystem write permissions apply.

## Platform Support

| Platform | Status |
| --- | --- |
| Windows | ✅ Tested and published |
| Linux | ✅ Tested and published |
| macOS | ❌ Not currently tested or published |

## Tech Stack

Built with **Tauri 2**, **Rust**, **SvelteKit**, **Svelte 5**, and **Vite**.

## Development note

Symlink Manager was developed with AI-assisted coding tools as part of an iterative workflow combining feature design, testing, debugging, and real-world use.

Bug reports and feedback are welcome.

## License

MIT © 2026 Danavans. See [LICENSE](LICENSE).