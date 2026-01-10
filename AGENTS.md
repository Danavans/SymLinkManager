# Agent Instructions (SymLinkManager)

Read `PROJECT_CONTEXT.md` first for product context, UX decisions, and JSON formats.

Operational rules:
- If `AGENTS.md` conflicts with `PROJECT_CONTEXT.md`, follow `AGENTS.md`.
- Preserve PlexTools dark UI style (no light panels or light background).
- Keep UI text in English.
- Prefer editing these files when changing core behavior: `src/routes/+page.svelte`, `src-tauri/src/lib.rs`, `src-tauri/src/main.rs`, `src-tauri/tauri.conf.json`.
- Follow existing patterns for root remap rules and Windows elevation flow.
- Avoid introducing new build steps; keep the app portable.
- Keep behavior parity across Windows/Linux unless a platform requires a specific flow.