# Project Brief

## What this is

**DecentDB Studio** — a cross-platform desktop client and database
administration tool for the [DecentDB](https://github.com/sphildreth/decentdb)
embedded database engine. Comparable to DBeaver / SQL Server Management Studio
but native and written in Rust.

## Hard requirements (source of truth)

1. Written in **Rust**.
2. **Cross-platform**: Linux, macOS, Windows.
3. Feature set comparable to DBeaver / SSMS:
   - Rich SQL editor with **syntax highlighting** and **autocompletion**.
   - **Data browsing/editing**.
   - **Entity-Relationship Diagram (ERD)** generation.
   - **Data export / migration**.
   - Schema viewing, querying, **EXPLAIN plans**.
4. UI built with **[iced](https://github.com/iced-rs/iced)**.
5. **Slick, modern, themeable UI** with several common themes.
6. Uses the **latest DecentDB release** (currently **v2.8.0**).
7. Ability to **convert SQLite databases into DecentDB**, using DecentDB's
   native types and features.

## Constraints

- DecentDB is **not on crates.io**; it must be a **git dependency** pinned to a
  release tag (`tag = "v2.8.0"`).
- DecentDB's build requires **libclang** (via `bindgen`/`libpg_query`).
- The UI must remain responsive: long operations (conversion, export) run on a
  blocking worker thread.

## Status

All requirements implemented and verified (builds clean, tests pass, GUI runs
and renders under a headless X server). See `context.md` for the current state.
