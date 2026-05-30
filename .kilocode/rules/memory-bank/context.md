# Active Context: DecentDB Studio

## Current State

**Application Status**: Rust desktop database client.

DecentDB Studio is an iced-based GUI for the DecentDB embedded database engine.
The source tree is a Cargo project with a reusable library and GUI binary.

## Recently Completed

- [x] Implemented SQL workbench, schema browser, ERD canvas, data editing, export, settings, themes, and SQLite -> DecentDB conversion.
- [x] Pinned DecentDB engine to git tag `v2.8.0`.
- [x] Fixed local compile failure caused by a stale `.cargo/config.toml` `LIBCLANG_PATH` override pointing at `/usr/lib/llvm-14/lib`.
- [x] Verified `cargo build` succeeds without a repo-level `LIBCLANG_PATH` override; bindgen auto-detects installed LLVM 22 on this machine.
- [x] Verified `cargo test` passes.

## Current Structure

| File/Directory | Purpose |
|----------------|---------|
| `src/main.rs` | GUI binary entry point |
| `src/lib.rs` | Library exports for database, conversion, export, settings, and theme modules |
| `src/app/` | iced application state, updates, views, and ERD canvas |
| `src/db/` | DecentDB connection wrapper, value parsing/formatting, and schema model |
| `src/convert/` | SQLite -> DecentDB conversion and type mapping |
| `tests/conversion.rs` | End-to-end SQLite conversion integration test |

## Current Focus

Keep the Cargo build reproducible across developer machines. DecentDB's
`libpg_query` dependency requires `libclang` through `bindgen`; do not commit an
active hard-coded `LIBCLANG_PATH` unless the repo is intentionally made
machine-specific.

## Build Notes

```bash
cargo build
cargo test
```

If `bindgen` reports that it cannot find `libclang`, install LLVM/libclang and
set `LIBCLANG_PATH` in the shell to the directory containing the shared library.

On this Fedora-based environment, LLVM is installed under
`/usr/lib64/llvm22/lib64`.

## Available Recipes

| Recipe | File | Use Case |
|--------|------|----------|
| Add Database | `.kilocode/recipes/add-database.md` | Data persistence features if the app scope expands |

## Pending Improvements

- [ ] Keep memory-bank files aligned with the Rust project; older template notes have been replaced where encountered.

## Session History

| Date | Changes |
|------|---------|
| 2026-05-30 | Removed stale active `LIBCLANG_PATH` from `.cargo/config.toml`; documented how to set it per machine. |
