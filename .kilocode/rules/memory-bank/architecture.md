# System Patterns: DecentDB Studio

## Architecture Overview

```
src/
├── main.rs                 # GUI binary entry point
├── lib.rs                  # Reusable library exports
├── app/                    # iced application state, update, views, ERD canvas
├── db/                     # DecentDB wrapper, schema model, value helpers
├── convert/                # SQLite conversion pipeline and type mapping
├── export.rs               # CSV/JSON/Markdown/SQL export
├── settings.rs             # Persisted app settings and recent files
└── theme.rs                # Theme catalogue and syntax-highlighting themes
```

## Key Design Patterns

### 1. Library + GUI Binary

The crate exposes reusable modules through `src/lib.rs`; the GUI binary in
`src/main.rs` wires those modules into the iced application builder. Tests and
examples consume the library rather than duplicating GUI code.

### 2. Database Access Boundary

All DecentDB access is funnelled through `db::Connection`, a cheap-to-clone
handle around `decentdb::Db`. Query results and schema data are converted into
app-owned structs before rendering.

### 3. Responsive UI Work

Long-running operations, including conversion and export, are dispatched to
blocking worker threads through the runtime helpers so the iced UI remains
responsive.

### 4. Conversion Pipeline

SQLite conversion introspects source schema, maps SQLite affinities to DecentDB
types, recreates tables and indexes, then copies data in batches.

## Styling Conventions

### iced UI

- UI code lives in `src/app/views.rs` and `src/app/erd.rs`.
- Themes are centralized in `src/theme.rs`.
- Keep database work out of view construction.

## File Naming Conventions

- Rust modules use snake_case.
- Public cross-module types should live near their domain boundary (`db`, `convert`, `export`, `settings`, `theme`).
- Tests should cover shared behavior in library modules and high-risk conversion paths.

## State Management

Application state is owned by the iced app module. Database handles, selected
objects, query buffers, result tabs, conversion progress, edit state, settings,
and theme choices flow through messages and update handlers.
