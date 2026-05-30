# Product

## Why this exists

DecentDB is an embedded engine with a Rust API but no graphical administration
tool. DecentDB Studio fills that gap: a desktop app for developers and DBAs to
inspect schema, run queries, read query plans, browse and edit data, visualize
relationships, and migrate data — without writing throwaway Rust code.

## Primary users

- Developers building on DecentDB who want a visual workbench.
- Users with existing **SQLite** databases who want to move to DecentDB and use
  its richer type system.

## Key user flows

1. **Open / create / in-memory** database from the toolbar or a recent list.
2. **Query**: write SQL (highlighted, with completion chips), run with
   `Ctrl/Cmd+Enter`, view per-statement result tabs.
3. **Browse & edit data**: select a table → Data tab → paginate, click a cell
   to edit (UPDATE), Add row (INSERT), delete row (DELETE). PK-less tables are
   read-only.
4. **Inspect structure**: Structure tab shows columns, keys, FKs and DDL.
5. **Explain**: view the plan for the current statement.
6. **Diagram**: pan/zoom ERD with PK/FK markers and relationship lines.
7. **Dashboard**: engine version, storage stats, object counts, and
   migrate/export actions.
8. **Export**: result sets/tables to CSV/JSON/Markdown/SQL; whole database to
   SQLite or a SQL dump.
9. **Convert SQLite → DecentDB**: pick a SQLite file, rebuild it in DecentDB
   with native types and indexes, then auto-open the result.

## UX goals

- Slick, modern, **themeable** (16 themes); theme + recents persist.
- Responsive: background work never blocks the UI.
- Safe edits: row identity uses the primary key; refuses unbounded UPDATE/DELETE.
