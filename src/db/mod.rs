//! DecentDB connection layer.
//!
//! Wraps the embedded [`decentdb::Db`] engine and exposes the higher-level
//! operations the UI needs: opening/creating databases, running queries,
//! capturing EXPLAIN plans, introspecting schema and gathering storage stats.

pub mod schema;
pub mod value;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use decentdb::{Db, DbConfig, QueryResult, StorageInfo, Value};

pub use schema::Schema;

/// A grid of query results ready for display.
#[derive(Debug, Clone, Default)]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    /// Number of rows affected by a DML statement (INSERT/UPDATE/DELETE).
    pub affected_rows: u64,
    /// Whether the executed statement returned a row set.
    pub is_query: bool,
    /// Wall-clock execution time in milliseconds.
    pub elapsed_ms: f64,
    /// The statement text that produced this result.
    pub statement: String,
}

impl ResultSet {
    fn from_query(statement: String, result: &QueryResult, elapsed_ms: f64) -> Self {
        let columns: Vec<String> = result.columns().to_vec();
        let rows: Vec<Vec<Value>> = result
            .rows()
            .iter()
            .map(|r| r.values().to_vec())
            .collect();
        let is_query = !columns.is_empty();
        Self {
            columns,
            rows,
            affected_rows: result.affected_rows(),
            is_query,
            elapsed_ms,
            statement,
        }
    }
}

/// Errors surfaced from the connection layer.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Engine(String),
    #[error("no database is currently open")]
    NotOpen,
}

impl From<decentdb::DbError> for DbError {
    fn from(e: decentdb::DbError) -> Self {
        DbError::Engine(e.to_string())
    }
}

/// An open connection to a DecentDB database file.
///
/// Cloning a [`Connection`] is cheap: the underlying handle is shared via
/// [`Arc`], so background tasks and the UI can hold their own clones.
#[derive(Clone)]
pub struct Connection {
    db: Arc<Db>,
    path: PathBuf,
}

impl Connection {
    /// Open an existing database file, creating it if it does not exist.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, DbError> {
        let path = path.as_ref().to_path_buf();
        let db = Db::open_or_create(&path, DbConfig::default())?;
        Ok(Self {
            db: Arc::new(db),
            path,
        })
    }

    /// Open an in-memory database (not persisted to disk).
    pub fn open_memory() -> Result<Self, DbError> {
        let db = Db::open(":memory:", DbConfig::default())?;
        Ok(Self {
            db: Arc::new(db),
            path: PathBuf::from(":memory:"),
        })
    }

    /// The on-disk path of this database.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A short display name for the database (file stem or `:memory:`).
    pub fn display_name(&self) -> String {
        if self.path == Path::new(":memory:") {
            return ":memory:".to_string();
        }
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.to_string_lossy().into_owned())
    }

    /// Execute a single SQL statement, returning a [`ResultSet`].
    pub fn execute(&self, sql: &str) -> Result<ResultSet, DbError> {
        let start = Instant::now();
        let result = self.db.execute(sql)?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        Ok(ResultSet::from_query(sql.to_string(), &result, elapsed))
    }

    /// Execute a batch of semicolon-separated statements, returning the result
    /// of each statement that produced output.
    pub fn execute_batch(&self, sql: &str) -> Result<Vec<ResultSet>, DbError> {
        let statements = split_statements(sql);
        let mut out = Vec::new();
        for stmt in statements {
            let trimmed = stmt.trim();
            if trimmed.is_empty() {
                continue;
            }
            let start = Instant::now();
            let result = self.db.execute(trimmed)?;
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            out.push(ResultSet::from_query(trimmed.to_string(), &result, elapsed));
        }
        Ok(out)
    }

    /// Run `EXPLAIN <sql>` and return the rendered plan lines.
    pub fn explain(&self, sql: &str) -> Result<Vec<String>, DbError> {
        let trimmed = sql.trim().trim_end_matches(';');
        let explain_sql = if trimmed.to_ascii_uppercase().starts_with("EXPLAIN") {
            trimmed.to_string()
        } else {
            format!("EXPLAIN {trimmed}")
        };
        let result = self.db.execute(&explain_sql)?;
        let lines = result.explain_lines();
        if !lines.is_empty() {
            Ok(lines.to_vec())
        } else {
            // Some plans come back as a normal result set; flatten it.
            Ok(result
                .rows()
                .iter()
                .map(|r| {
                    r.values()
                        .iter()
                        .map(value::display)
                        .collect::<Vec<_>>()
                        .join("  ")
                })
                .collect())
        }
    }

    /// Refresh and return the schema model.
    pub fn schema(&self) -> Result<Schema, DbError> {
        let snapshot = self.db.get_schema_snapshot()?;
        let mut schema = Schema::from_snapshot(&snapshot);
        if let Ok(meta) = self.db.get_tooling_metadata() {
            schema.fingerprint = meta.schema_fingerprint;
        }
        Ok(schema)
    }

    /// Storage statistics for the dashboard.
    pub fn storage_info(&self) -> Result<StorageInfo, DbError> {
        Ok(self.db.storage_info()?)
    }

    /// The DecentDB engine version string.
    pub fn engine_version(&self) -> String {
        decentdb::version().to_string()
    }

    /// Dump the full database as SQL (schema + data).
    pub fn dump_sql(&self) -> Result<String, DbError> {
        Ok(self.db.dump_sql()?)
    }

    /// Run a checkpoint to flush the WAL.
    pub fn checkpoint(&self) -> Result<(), DbError> {
        self.db.checkpoint()?;
        Ok(())
    }

    /// Browse a table's rows with a limit/offset for pagination.
    pub fn browse_table(
        &self,
        table: &str,
        limit: usize,
        offset: usize,
    ) -> Result<ResultSet, DbError> {
        let sql = format!("SELECT * FROM {} LIMIT {} OFFSET {}", quote_ident(table), limit, offset);
        self.execute(&sql)
    }

    /// Update a single column of a single row, identified by a set of
    /// key column/value pairs (typically the primary key).
    ///
    /// Returns the number of affected rows. If `key_columns` is empty the
    /// update is refused to avoid an unbounded `UPDATE`.
    pub fn update_cell(
        &self,
        table: &str,
        column: &str,
        new_value: &Value,
        key_columns: &[(String, Value)],
    ) -> Result<u64, DbError> {
        if key_columns.is_empty() {
            return Err(DbError::Engine(
                "cannot edit a row without a primary key or unique row identity".to_string(),
            ));
        }
        let where_clause = build_where(key_columns);
        let sql = format!(
            "UPDATE {} SET {} = {} WHERE {}",
            quote_ident(table),
            quote_ident(column),
            value::sql_literal(new_value),
            where_clause
        );
        Ok(self.execute(&sql)?.affected_rows)
    }

    /// Delete a single row identified by key column/value pairs.
    pub fn delete_row(
        &self,
        table: &str,
        key_columns: &[(String, Value)],
    ) -> Result<u64, DbError> {
        if key_columns.is_empty() {
            return Err(DbError::Engine(
                "cannot delete a row without a primary key or unique row identity".to_string(),
            ));
        }
        let sql = format!(
            "DELETE FROM {} WHERE {}",
            quote_ident(table),
            build_where(key_columns)
        );
        Ok(self.execute(&sql)?.affected_rows)
    }

    /// Insert a new row from column/value pairs. Columns with a `None` value
    /// are omitted so engine defaults / auto-increment apply.
    pub fn insert_row(
        &self,
        table: &str,
        columns: &[(String, Option<Value>)],
    ) -> Result<u64, DbError> {
        let provided: Vec<&(String, Option<Value>)> =
            columns.iter().filter(|(_, v)| v.is_some()).collect();
        if provided.is_empty() {
            // Insert defaults for every column.
            let sql = format!("INSERT INTO {} DEFAULT VALUES", quote_ident(table));
            return Ok(self.execute(&sql)?.affected_rows);
        }
        let cols = provided
            .iter()
            .map(|(c, _)| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let vals = provided
            .iter()
            .map(|(_, v)| value::sql_literal(v.as_ref().unwrap()))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quote_ident(table),
            cols,
            vals
        );
        Ok(self.execute(&sql)?.affected_rows)
    }

    /// Export the whole database to a SQLite file at `dest`.
    ///
    /// Recreates the schema and copies all table data. Types are rendered to
    /// SQLite-compatible literals. Returns the number of rows copied.
    pub fn export_to_sqlite(&self, dest: &Path) -> Result<usize, DbError> {
        crate::convert::export_to_sqlite(self, dest).map_err(|e| DbError::Engine(e.to_string()))
    }

    /// Provide the engine handle for advanced operations (e.g. bulk load during
    /// conversion).
    pub fn raw(&self) -> &Db {
        &self.db
    }
}

/// Build a `col = literal AND col = literal` WHERE clause from key pairs.
fn build_where(key_columns: &[(String, Value)]) -> String {
    key_columns
        .iter()
        .map(|(col, val)| {
            if value::is_null(val) {
                format!("{} IS NULL", quote_ident(col))
            } else {
                format!("{} = {}", quote_ident(col), value::sql_literal(val))
            }
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// Quote an SQL identifier with double quotes, escaping embedded quotes.
pub fn quote_ident(ident: &str) -> String {
    let escaped = ident.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

/// Naive SQL statement splitter that respects single/double quotes and line
/// comments. Sufficient for an interactive editor; the engine performs the
/// authoritative parse.
pub fn split_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut chars = sql.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(c) = chars.next() {
        if in_line_comment {
            current.push(c);
            if c == '\n' {
                in_line_comment = false;
            }
            continue;
        }
        if in_block_comment {
            current.push(c);
            if c == '*' && chars.peek() == Some(&'/') {
                current.push(chars.next().unwrap());
                in_block_comment = false;
            }
            continue;
        }
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(c);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(c);
            }
            '-' if !in_single && !in_double && chars.peek() == Some(&'-') => {
                current.push(c);
                current.push(chars.next().unwrap());
                in_line_comment = true;
            }
            '/' if !in_single && !in_double && chars.peek() == Some(&'*') => {
                current.push(c);
                current.push(chars.next().unwrap());
                in_block_comment = true;
            }
            ';' if !in_single && !in_double => {
                statements.push(current.clone());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        statements.push(current);
    }
    statements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_statements_respecting_quotes() {
        let sql = "SELECT ';'; INSERT INTO t VALUES (1); -- comment;\nSELECT 2;";
        let parts = split_statements(sql);
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn in_memory_roundtrip() {
        let conn = Connection::open_memory().unwrap();
        conn.execute("CREATE TABLE t (id INT64 PRIMARY KEY, name TEXT)")
            .unwrap();
        conn.execute("INSERT INTO t (id, name) VALUES (1, 'Ada')")
            .unwrap();
        let rs = conn.execute("SELECT id, name FROM t").unwrap();
        assert_eq!(rs.columns, vec!["id", "name"]);
        assert_eq!(rs.rows.len(), 1);
    }
}
