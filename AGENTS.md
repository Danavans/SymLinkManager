# Agent Instructions (Symlink Manager)

Read `PROJECT_CONTEXT.md` first for the current product, architecture, UX and JSON format. If it conflicts with this file, follow `AGENTS.md`.

## Current application

Symlink Manager 1.1.0 is a portable Tauri + Svelte application for Windows and Linux. Its current identity uses graphite surfaces, indigo accents, green for healthy links, rose for broken links and amber for unreadable links. The workspace has sidebar navigation, a paginated scan inventory and a guided import/recreation flow. These describe the current design; future user-requested design changes may replace them.

## Working rules

- Keep UI text in English and maintain accessible controls, focus handling and responsive layouts.
- Keep branding consistent across `assets/icon.svg`, `static/logo.svg`, the favicon and Tauri application/window icons when changing the identity.
- For UI work, start with `src/routes/+page.svelte`; for filesystem behavior, start with `src-tauri/src/lib.rs`. `src-tauri/src/main.rs` handles the elevated-worker entry point, and `src-tauri/tauri.conf.json` contains application settings.
- Preserve JSON compatibility, first-match root mapping, internal-target relocation, search exclusions and filtered export across all pages unless the requested change explicitly revises those behaviors.
- Preserve import validation, destination ancestor checks, replacement rollback and the Windows elevation flow. Never remove these safeguards as a simplification.
- Keep expensive scan/recreation/conflict checks off the UI thread and retain bounded scan concurrency.
- Keep the app portable, with no unnecessary dependencies, runtime services or additional build stages.
- Maintain Windows/Linux behavior parity where possible; document platform-specific flows and distinguish tested behavior from intended support.
- Run checks appropriate to the changes. Documentation/version-only updates need consistency checks, not a full UI rebuild or filesystem benchmark.
- Keep `README.md` user-facing, `PROJECT_CONTEXT.md` accurate for future agents and `AUDIT.md` as the detailed findings/measurement record.
- When changing the app version, synchronize `package.json`, the root entries in `package-lock.json`, `src-tauri/Cargo.toml`, the application entry in `src-tauri/Cargo.lock` and `src-tauri/tauri.conf.json`. Update release references without changing dependency versions.
- After code or metadata changes, propose a Conventional Commits message.
