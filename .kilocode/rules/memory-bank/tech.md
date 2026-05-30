# Technical Context: DecentDB Studio

## Technology Stack

| Technology | Version | Purpose |
| ---------- | ------- | ------- |
| Rust | Edition 2021, stable toolchain | Application and library implementation |
| Cargo | Rust toolchain default | Build, dependency, and test runner |
| iced | 0.14 | Cross-platform desktop UI |
| DecentDB | Git tag `v2.8.0` | Embedded database engine |
| rusqlite | 0.32, bundled SQLite | SQLite import/export support |
| rfd | 0.15 | Native file dialogs |
| serde / serde_json | 1.x | Settings and data serialization |
| tokio | 1.x | Blocking worker/runtime helpers used with iced |

## Development Environment

### Prerequisites

- Recent stable Rust toolchain from `rustup`.
- C compiler/toolchain.
- LLVM/libclang for `bindgen`, required by DecentDB's `libpg_query` dependency.
- Usual desktop runtime libraries for iced/wgpu on Linux.

### Commands

```bash
cargo build              # Development build
cargo build --release    # Optimized GUI binary
cargo run --release      # Launch GUI
cargo test               # Unit and integration tests
cargo run --release --example seed -- demo.ddb
```

## Project Configuration

### Cargo

- `Cargo.toml` defines the `decentdb-studio` binary at `src/main.rs`.
- `decentdb` is a git dependency pinned to `https://github.com/sphildreth/decentdb`, tag `v2.8.0`.
- `rusqlite` uses the `bundled` feature, so no system SQLite library is needed.
- `.cargo/config.toml` must not set an active machine-specific `LIBCLANG_PATH`.

### Native Build Dependency

`pg_query` uses `bindgen`, which needs libclang at build time. If auto-discovery
fails, set `LIBCLANG_PATH` in the shell to the directory containing libclang:

```bash
LIBCLANG_PATH=/path/to/llvm/lib cargo build
```

## Key Dependencies

### Production Dependencies

```toml
iced = { version = "0.14", features = ["highlighter", "tokio", "canvas", "advanced", "image", "lazy"] }
decentdb = { git = "https://github.com/sphildreth/decentdb", tag = "v2.8.0" }
rusqlite = { version = "0.32", features = ["bundled", "column_decltype"] }
rfd = "0.15"
```

### Dev Dependencies

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
```

## File Structure

```
/
├── Cargo.toml              # Crate metadata and dependencies
├── Cargo.lock              # Locked dependency graph
├── .cargo/config.toml      # Comments for local native build configuration
├── src/
│   ├── main.rs             # GUI binary entry point
│   ├── lib.rs              # Library module exports
│   ├── app/                # iced application state, update, views, ERD canvas
│   ├── db/                 # DecentDB wrapper, schema, value helpers
│   ├── convert/            # SQLite conversion and type mapping
│   ├── export.rs           # Result/table export formats
│   ├── settings.rs         # Persisted user settings
│   └── theme.rs            # Theme catalogue
├── examples/seed.rs        # Demo DecentDB database generator
└── tests/conversion.rs     # SQLite conversion integration test
```

## Technical Constraints

- DecentDB is not published to crates.io; keep the git tag pinned.
- The UI should stay responsive; long database conversion/export work runs on blocking workers.
- Cross-platform support matters; avoid committing machine-local absolute paths as active config.

## Performance Considerations

- Release builds use `opt-level = 3`, thin LTO, one codegen unit, and stripping.
- GUI rendering uses iced/wgpu with a software fallback path.
- Conversion and export work should avoid blocking the UI thread.

## Deployment

### Build Output

- Development binary: `target/debug/decentdb-studio`.
- Release binary: `target/release/decentdb-studio`.

### Environment Variables

- `LIBCLANG_PATH` may be needed when bindgen cannot auto-detect libclang.
