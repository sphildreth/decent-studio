//! SQLite -> DecentDB conversion.
//!
//! Reads a SQLite database with `rusqlite`, maps its declared types onto
//! DecentDB's richer native type system, recreates the schema (tables,
//! indexes), and migrates the data in batched transactions. The conversion
//! deliberately leans on DecentDB-native types (BOOL, TIMESTAMP, UUID, DATE,
//! BLOB, DECIMAL, etc.) rather than collapsing everything to TEXT, so the
//! resulting database exercises DecentDB's full type and indexing surface.

mod typemap;

use std::path::Path;

use decentdb::Value as DdbValue;
use rusqlite::types::ValueRef;
use rusqlite::Connection as SqliteConn;

use crate::db::{quote_ident, Connection};

pub use typemap::{map_sqlite_type, DdbType};

/// Options controlling a conversion run.
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    /// Recreate indexes after loading the data (recommended for performance).
    pub create_indexes: bool,
    /// Number of rows per insert transaction batch.
    pub batch_size: usize,
    /// Run a checkpoint + analyze at the end.
    pub finalize: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            create_indexes: true,
            batch_size: 1000,
            finalize: true,
        }
    }
}

/// A progress event emitted during conversion, suitable for streaming to the UI.
#[derive(Debug, Clone)]
pub enum Progress {
    /// Conversion started; total table count.
    Started { tables: usize },
    /// A table's schema was created.
    SchemaCreated { table: String, columns: usize },
    /// Rows copied so far for a table.
    Rows {
        table: String,
        copied: usize,
        total: usize,
    },
    /// A table finished migrating.
    TableDone { table: String, rows: usize },
    /// An index was created.
    IndexCreated { name: String },
    /// A non-fatal warning.
    Warning(String),
    /// Conversion finished successfully.
    Finished {
        tables: usize,
        rows: usize,
        warnings: usize,
    },
}

/// A summary returned at the end of conversion.
#[derive(Debug, Clone, Default)]
pub struct ConvertReport {
    pub tables: usize,
    pub rows: usize,
    pub indexes: usize,
    pub warnings: Vec<String>,
    pub log: Vec<String>,
}

/// Errors during conversion.
#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("DecentDB error: {0}")]
    Ddb(#[from] crate::db::DbError),
    #[error("{0}")]
    Other(String),
}

impl From<decentdb::DbError> for ConvertError {
    fn from(e: decentdb::DbError) -> Self {
        ConvertError::Ddb(crate::db::DbError::from(e))
    }
}

/// Metadata about a SQLite column read from `PRAGMA table_info`.
#[derive(Debug, Clone)]
struct SqliteColumn {
    name: String,
    decl_type: String,
    not_null: bool,
    primary_key: bool,
    default: Option<String>,
}

/// Metadata about a SQLite table.
#[derive(Debug, Clone)]
struct SqliteTable {
    name: String,
    columns: Vec<SqliteColumn>,
}

/// Metadata about a SQLite index.
#[derive(Debug, Clone)]
struct SqliteIndex {
    name: String,
    table: String,
    unique: bool,
    columns: Vec<String>,
}

/// Convert a SQLite database file into a (new or existing) DecentDB connection.
///
/// `progress` is invoked synchronously for each [`Progress`] event so the
/// caller can forward it to the UI. The function returns a [`ConvertReport`].
pub fn convert<P: AsRef<Path>>(
    sqlite_path: P,
    target: &Connection,
    options: &ConvertOptions,
    mut progress: impl FnMut(Progress),
) -> Result<ConvertReport, ConvertError> {
    let src = SqliteConn::open(sqlite_path.as_ref())?;
    let mut report = ConvertReport::default();

    let tables = read_tables(&src)?;
    let indexes = if options.create_indexes {
        read_indexes(&src)?
    } else {
        Vec::new()
    };

    progress(Progress::Started {
        tables: tables.len(),
    });

    for table in &tables {
        // 1. Build and run the DecentDB CREATE TABLE statement.
        let create_sql = build_create_table(table, &mut report);
        report.log.push(create_sql.clone());
        target.execute(&create_sql)?;
        progress(Progress::SchemaCreated {
            table: table.name.clone(),
            columns: table.columns.len(),
        });

        // 2. Copy the data in batches.
        let rows = copy_table_data(&src, target, table, options, &mut progress, &mut report)?;
        report.rows += rows;
        report.tables += 1;
        progress(Progress::TableDone {
            table: table.name.clone(),
            rows,
        });
    }

    // 3. Recreate indexes after data load.
    if options.create_indexes {
        for index in &indexes {
            match build_create_index(index) {
                Some(sql) => {
                    if let Err(e) = target.execute(&sql) {
                        let warn = format!("Skipped index {}: {e}", index.name);
                        report.warnings.push(warn.clone());
                        progress(Progress::Warning(warn));
                    } else {
                        report.indexes += 1;
                        progress(Progress::IndexCreated {
                            name: index.name.clone(),
                        });
                    }
                }
                None => {}
            }
        }
    }

    // 4. Finalize.
    if options.finalize {
        if let Err(e) = target.checkpoint() {
            report
                .warnings
                .push(format!("Checkpoint after import failed: {e}"));
        }
    }

    progress(Progress::Finished {
        tables: report.tables,
        rows: report.rows,
        warnings: report.warnings.len(),
    });

    Ok(report)
}

/// Read user table definitions from the SQLite catalog.
fn read_tables(src: &SqliteConn) -> Result<Vec<SqliteTable>, ConvertError> {
    let mut names: Vec<String> = Vec::new();
    {
        let mut stmt = src.prepare(
            "SELECT name FROM sqlite_master \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for r in rows {
            names.push(r?);
        }
    }

    let mut tables = Vec::new();
    for name in names {
        let columns = read_columns(src, &name)?;
        tables.push(SqliteTable { name, columns });
    }
    Ok(tables)
}

/// Read column info for a table from `PRAGMA table_info`.
fn read_columns(src: &SqliteConn, table: &str) -> Result<Vec<SqliteColumn>, ConvertError> {
    let pragma = format!("PRAGMA table_info({})", quote_ident(table));
    let mut stmt = src.prepare(&pragma)?;
    let rows = stmt.query_map([], |row| {
        Ok(SqliteColumn {
            // cid is column 0
            name: row.get::<_, String>(1)?,
            decl_type: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            not_null: row.get::<_, i64>(3)? != 0,
            default: row.get::<_, Option<String>>(4)?,
            primary_key: row.get::<_, i64>(5)? != 0,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Read index definitions, excluding auto-generated ones.
fn read_indexes(src: &SqliteConn) -> Result<Vec<SqliteIndex>, ConvertError> {
    let mut idx_list: Vec<(String, String, bool)> = Vec::new();
    {
        let mut stmt = src.prepare(
            "SELECT m.name, m.tbl_name FROM sqlite_master m \
             WHERE m.type = 'index' AND m.sql IS NOT NULL AND m.name NOT LIKE 'sqlite_%'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for r in rows {
            let (name, table) = r?;
            idx_list.push((name, table, false));
        }
    }

    let mut indexes = Vec::new();
    for (name, table, _) in idx_list {
        // PRAGMA index_info returns the indexed columns in order.
        let mut columns = Vec::new();
        {
            let pragma = format!("PRAGMA index_info({})", quote_ident(&name));
            let mut stmt = src.prepare(&pragma)?;
            let rows = stmt.query_map([], |row| row.get::<_, Option<String>>(2))?;
            for r in rows {
                if let Some(col) = r? {
                    columns.push(col);
                }
            }
        }
        // Determine uniqueness via PRAGMA index_list.
        let unique = index_is_unique(src, &table, &name)?;
        if !columns.is_empty() {
            indexes.push(SqliteIndex {
                name,
                table,
                unique,
                columns,
            });
        }
    }
    Ok(indexes)
}

fn index_is_unique(src: &SqliteConn, table: &str, index: &str) -> Result<bool, ConvertError> {
    let pragma = format!("PRAGMA index_list({})", quote_ident(table));
    let mut stmt = src.prepare(&pragma)?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)? != 0))
    })?;
    for r in rows {
        let (name, unique) = r?;
        if name == index {
            return Ok(unique);
        }
    }
    Ok(false)
}

/// Build a DecentDB CREATE TABLE statement from a SQLite table definition.
fn build_create_table(table: &SqliteTable, report: &mut ConvertReport) -> String {
    let pk_count = table.columns.iter().filter(|c| c.primary_key).count();
    let mut col_defs = Vec::new();

    for col in &table.columns {
        let ddb = map_sqlite_type(&col.decl_type);
        let mut def = format!("{} {}", quote_ident(&col.name), ddb.sql());

        // Inline single-column primary key (DecentDB auto-increments INT64 PKs).
        if col.primary_key && pk_count == 1 {
            def.push_str(" PRIMARY KEY");
        }
        if col.not_null && !(col.primary_key && pk_count == 1) {
            def.push_str(" NOT NULL");
        }
        if let Some(default) = &col.default {
            if is_safe_default(default) {
                def.push_str(&format!(" DEFAULT {default}"));
            } else {
                report
                    .warnings
                    .push(format!("Dropped default for {}.{}", table.name, col.name));
            }
        }
        col_defs.push(def);
    }

    // Composite primary key.
    if pk_count > 1 {
        let pk_cols: Vec<String> = table
            .columns
            .iter()
            .filter(|c| c.primary_key)
            .map(|c| quote_ident(&c.name))
            .collect();
        col_defs.push(format!("PRIMARY KEY ({})", pk_cols.join(", ")));
    }

    format!(
        "CREATE TABLE {} (\n  {}\n)",
        quote_ident(&table.name),
        col_defs.join(",\n  ")
    )
}

/// Build a DecentDB CREATE INDEX statement.
fn build_create_index(index: &SqliteIndex) -> Option<String> {
    let cols: Vec<String> = index.columns.iter().map(|c| quote_ident(c)).collect();
    let unique = if index.unique { "UNIQUE " } else { "" };
    Some(format!(
        "CREATE {}INDEX {} ON {} ({})",
        unique,
        quote_ident(&index.name),
        quote_ident(&index.table),
        cols.join(", ")
    ))
}

/// Whether a SQLite default expression is safe to carry over verbatim.
fn is_safe_default(default: &str) -> bool {
    let d = default.trim();
    if d.is_empty() {
        return false;
    }
    let upper = d.to_ascii_uppercase();
    if matches!(upper.as_str(), "NULL" | "TRUE" | "FALSE" | "CURRENT_TIMESTAMP") {
        return true;
    }
    // Numeric literal
    if d.parse::<f64>().is_ok() {
        return true;
    }
    // Quoted string literal
    if d.starts_with('\'') && d.ends_with('\'') {
        return true;
    }
    false
}

/// Copy all rows of a SQLite table into the DecentDB target in batches.
fn copy_table_data(
    src: &SqliteConn,
    target: &Connection,
    table: &SqliteTable,
    options: &ConvertOptions,
    progress: &mut impl FnMut(Progress),
    _report: &mut ConvertReport,
) -> Result<usize, ConvertError> {
    let total: usize = src
        .query_row(
            &format!("SELECT COUNT(*) FROM {}", quote_ident(&table.name)),
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize;

    if total == 0 {
        return Ok(0);
    }

    let col_names: Vec<String> = table.columns.iter().map(|c| c.name.clone()).collect();
    let col_list = col_names
        .iter()
        .map(|c| quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");

    let select_sql = format!(
        "SELECT {} FROM {}",
        col_list,
        quote_ident(&table.name)
    );
    let mut stmt = src.prepare(&select_sql)?;
    let column_count = table.columns.len();

    let mut rows = stmt.query([])?;
    let mut copied = 0usize;
    let mut batch: Vec<String> = Vec::with_capacity(options.batch_size);

    let target_db = target.raw();

    let flush = |batch: &mut Vec<String>, db: &decentdb::Db| -> Result<(), ConvertError> {
        if batch.is_empty() {
            return Ok(());
        }
        db.begin_transaction()?;
        let result: Result<(), ConvertError> = (|| {
            for sql in batch.iter() {
                db.execute(sql)?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                db.commit_transaction()?;
            }
            Err(e) => {
                let _ = db.rollback_transaction();
                return Err(e);
            }
        }
        batch.clear();
        Ok(())
    };

    while let Some(row) = rows.next()? {
        let mut literals = Vec::with_capacity(column_count);
        for (i, col) in table.columns.iter().enumerate() {
            let value_ref = row.get_ref(i)?;
            let ddb_value = sqlite_to_ddb(value_ref, &map_sqlite_type(&col.decl_type));
            literals.push(crate::db::value::sql_literal(&ddb_value));
        }
        let insert = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quote_ident(&table.name),
            col_list,
            literals.join(", ")
        );
        batch.push(insert);
        copied += 1;

        if batch.len() >= options.batch_size {
            flush(&mut batch, target_db)?;
            progress(Progress::Rows {
                table: table.name.clone(),
                copied,
                total,
            });
        }
    }
    flush(&mut batch, target_db)?;
    progress(Progress::Rows {
        table: table.name.clone(),
        copied,
        total,
    });

    Ok(copied)
}

/// Convert a SQLite value into a DecentDB [`DdbValue`], coercing toward the
/// mapped target type when sensible.
fn sqlite_to_ddb(value: ValueRef<'_>, target: &DdbType) -> DdbValue {
    match value {
        ValueRef::Null => DdbValue::Null,
        ValueRef::Integer(i) => match target {
            DdbType::Bool => DdbValue::Bool(i != 0),
            DdbType::Float => DdbValue::Float64(i as f64),
            DdbType::Text => DdbValue::Text(i.to_string()),
            _ => DdbValue::Int64(i),
        },
        ValueRef::Real(f) => match target {
            DdbType::Int => DdbValue::Int64(f as i64),
            DdbType::Text => DdbValue::Text(f.to_string()),
            _ => DdbValue::Float64(f),
        },
        ValueRef::Text(bytes) => {
            let s = String::from_utf8_lossy(bytes).into_owned();
            match target {
                DdbType::Int => s.parse::<i64>().map(DdbValue::Int64).unwrap_or(DdbValue::Text(s)),
                DdbType::Float => {
                    s.parse::<f64>().map(DdbValue::Float64).unwrap_or(DdbValue::Text(s))
                }
                DdbType::Bool => match s.to_ascii_lowercase().as_str() {
                    "1" | "true" | "t" | "yes" => DdbValue::Bool(true),
                    "0" | "false" | "f" | "no" => DdbValue::Bool(false),
                    _ => DdbValue::Text(s),
                },
                _ => DdbValue::Text(s),
            }
        }
        ValueRef::Blob(bytes) => DdbValue::Blob(bytes.to_vec()),
    }
}

// ----------------------------------------------------------------------------
// Reverse direction: DecentDB -> SQLite export (data migration)
// ----------------------------------------------------------------------------

/// Export a DecentDB database into a fresh SQLite file.
///
/// This is the reverse of [`convert`] and supports migrating data out of
/// DecentDB into the ubiquitous SQLite format. Schema is recreated with SQLite
/// affinities and data is copied in a single transaction. Returns the number of
/// rows written.
pub fn export_to_sqlite(
    source: &crate::db::Connection,
    dest: &std::path::Path,
) -> Result<usize, ConvertError> {
    let _ = std::fs::remove_file(dest);
    let sqlite = SqliteConn::open(dest)?;
    let schema = source.schema().map_err(ConvertError::Ddb)?;

    sqlite.execute_batch("PRAGMA journal_mode=OFF; BEGIN;")?;
    let mut total_rows = 0usize;

    for table in &schema.tables {
        // Recreate the table with SQLite-friendly types.
        let mut col_defs = Vec::new();
        let single_pk = table.primary_key_columns.len() == 1;
        for col in &table.columns {
            let sqlite_type = ddb_type_to_sqlite(&col.type_name);
            let mut def = format!("{} {}", quote_ident(&col.name), sqlite_type);
            if col.primary_key && single_pk {
                def.push_str(" PRIMARY KEY");
            }
            if !col.nullable && !(col.primary_key && single_pk) {
                def.push_str(" NOT NULL");
            }
            col_defs.push(def);
        }
        if table.primary_key_columns.len() > 1 {
            let pk = table
                .primary_key_columns
                .iter()
                .map(|c| quote_ident(c))
                .collect::<Vec<_>>()
                .join(", ");
            col_defs.push(format!("PRIMARY KEY ({pk})"));
        }
        let create = format!(
            "CREATE TABLE {} ({})",
            quote_ident(&table.name),
            col_defs.join(", ")
        );
        sqlite.execute(&create, [])?;

        // Copy data.
        let rs = source
            .execute(&format!("SELECT * FROM {}", quote_ident(&table.name)))
            .map_err(ConvertError::Ddb)?;
        let col_list = rs
            .columns
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        for row in &rs.rows {
            let literals = row
                .iter()
                .map(crate::db::value::sql_literal)
                .collect::<Vec<_>>()
                .join(", ");
            let insert = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                quote_ident(&table.name),
                col_list,
                literals
            );
            sqlite.execute(&insert, [])?;
            total_rows += 1;
        }
    }

    sqlite.execute_batch("COMMIT;")?;
    Ok(total_rows)
}

/// Map a DecentDB type name onto a SQLite column type with matching affinity.
fn ddb_type_to_sqlite(ddb_type: &str) -> &'static str {
    let upper = ddb_type.to_ascii_uppercase();
    if upper.contains("INT") || upper.contains("BOOL") {
        "INTEGER"
    } else if upper.contains("FLOAT") || upper.contains("REAL") || upper.contains("DOUBLE") {
        "REAL"
    } else if upper.contains("BLOB") || upper.contains("GEOMETRY") || upper.contains("GEOGRAPHY") {
        "BLOB"
    } else if upper.contains("DECIMAL") || upper.contains("NUMERIC") {
        "NUMERIC"
    } else {
        // TEXT for everything else (TEXT, UUID, TIMESTAMP, DATE, etc.).
        "TEXT"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_source() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, \
             active BOOLEAN DEFAULT 1, score REAL, avatar BLOB);
             INSERT INTO users (name, active, score) VALUES ('Ada', 1, 9.5);
             INSERT INTO users (name, active, score) VALUES ('Linus', 0, 8.0);
             CREATE INDEX idx_users_name ON users(name);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn end_to_end_conversion() {
        let src_path = std::env::temp_dir().join("ddb_studio_convert_test.sqlite");
        let _ = std::fs::remove_file(&src_path);
        {
            let conn = rusqlite::Connection::open(&src_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, \
                 active BOOLEAN DEFAULT 1, score REAL);
                 INSERT INTO users (name, active, score) VALUES ('Ada', 1, 9.5);
                 INSERT INTO users (name, active, score) VALUES ('Linus', 0, 8.0);
                 CREATE INDEX idx_users_name ON users(name);",
            )
            .unwrap();
        }

        let target = Connection::open_memory().unwrap();
        let report = convert(&src_path, &target, &ConvertOptions::default(), |_| {}).unwrap();
        assert_eq!(report.tables, 1);
        assert_eq!(report.rows, 2);

        let rs = target.execute("SELECT name FROM users ORDER BY id").unwrap();
        assert_eq!(rs.rows.len(), 2);
        let _ = std::fs::remove_file(&src_path);
    }

    #[test]
    fn exports_back_to_sqlite() {
        // Build a DecentDB database, then export it to SQLite and read it back.
        let target = Connection::open_memory().unwrap();
        target
            .execute("CREATE TABLE t (id INT64 PRIMARY KEY, name TEXT, flag BOOL, amount DECIMAL(10,2))")
            .unwrap();
        target
            .execute("INSERT INTO t (id, name, flag, amount) VALUES (1, 'Ada', true, 19.99)")
            .unwrap();
        target
            .execute("INSERT INTO t (id, name, flag, amount) VALUES (2, 'Linus', false, 8.50)")
            .unwrap();

        let dest = std::env::temp_dir().join(format!("ddb_export_{}.sqlite", std::process::id()));
        let rows = export_to_sqlite(&target, &dest).unwrap();
        assert_eq!(rows, 2);

        let back = rusqlite::Connection::open(&dest).unwrap();
        let count: i64 = back
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        let name: String = back
            .query_row("SELECT name FROM t WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Ada");
        let _ = std::fs::remove_file(&dest);
    }

    #[test]
    fn maps_columns_to_native_types() {
        let conn = make_source();
        let tables = read_tables(&conn).unwrap();
        assert_eq!(tables.len(), 1);
        let create = build_create_table(&tables[0], &mut ConvertReport::default());
        assert!(create.contains("BOOL"));
        assert!(create.contains("PRIMARY KEY"));
    }
}
