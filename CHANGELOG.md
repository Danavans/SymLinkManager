# Changelog

All notable changes to this project are documented here. This project follows the spirit of [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - 2026-09-17

### Added

- A redesigned graphite and indigo workspace with compact, paginated scan results.
- A guided Import & recreate flow with automatic preview, detected roots, and optional path remapping.
- Window size, position, and maximized-state restoration between launches.

### Changed

- Faster bounded-concurrency scanning, with heavy filesystem work kept off the UI thread.
- Safer recreation validation and replacement handling, including restoration after a failed replacement.
- More robust handling of UNC paths, Unicode, and filesystem errors.

## [1.0.0] - 2026-06-24

### Added

- First public-ready release for scanning, exporting, importing, and recreating symbolic links on Windows and Linux.
- Health states, search filters, JSON snapshots, target-root remapping, and Windows elevation when needed.
