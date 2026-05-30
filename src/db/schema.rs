//! UI-facing schema model derived from DecentDB's [`SchemaSnapshot`].
//!
//! The engine exposes rich metadata structs, but the UI benefits from a flat,
//! cloneable model that owns its data and is easy to render in trees, ERDs and
//! object inspectors.

use decentdb::{SchemaSnapshot, SchemaTableInfo};

/// A single column within a table.
#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub unique: bool,
    pub auto_increment: bool,
    pub default_sql: Option<String>,
    /// Name of the table referenced by this column's foreign key, if any.
    pub references: Option<ForeignKeyRef>,
}

/// A resolved foreign-key reference for a column or table.
#[derive(Debug, Clone)]
pub struct ForeignKeyRef {
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
}

/// A table and its columns plus derived metadata.
#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub primary_key_columns: Vec<String>,
    pub foreign_keys: Vec<ForeignKeyRef>,
    pub row_count: usize,
    pub ddl: String,
}

/// An index definition.
#[derive(Debug, Clone)]
pub struct Index {
    pub name: String,
    pub table_name: String,
    pub kind: String,
    pub unique: bool,
    pub columns: Vec<String>,
}

/// A view definition.
#[derive(Debug, Clone)]
pub struct View {
    pub name: String,
    pub columns: Vec<String>,
    pub sql_text: String,
}

/// A trigger definition.
#[derive(Debug, Clone)]
pub struct Trigger {
    pub name: String,
    pub target: String,
    pub timing: String,
    pub events: Vec<String>,
}

/// The complete schema model for an open database.
#[derive(Debug, Clone, Default)]
pub struct Schema {
    pub tables: Vec<Table>,
    pub views: Vec<View>,
    pub indexes: Vec<Index>,
    pub triggers: Vec<Trigger>,
    pub fingerprint: String,
}

impl Schema {
    /// Build a UI schema model from an engine [`SchemaSnapshot`].
    pub fn from_snapshot(snapshot: &SchemaSnapshot) -> Self {
        let tables = snapshot.tables.iter().map(Table::from_info).collect();

        let views = snapshot
            .views
            .iter()
            .map(|v| View {
                name: v.name.clone(),
                columns: v.column_names.clone(),
                sql_text: v.sql_text.clone(),
            })
            .collect();

        let indexes = snapshot
            .indexes
            .iter()
            .map(|i| Index {
                name: i.name.clone(),
                table_name: i.table_name.clone(),
                kind: i.kind.clone(),
                unique: i.unique,
                columns: i.columns.clone(),
            })
            .collect();

        let triggers = snapshot
            .triggers
            .iter()
            .map(|t| Trigger {
                name: t.name.clone(),
                target: t.target_name.clone(),
                timing: t.timing.clone(),
                events: t.events.clone(),
            })
            .collect();

        Self {
            tables,
            views,
            indexes,
            triggers,
            fingerprint: String::new(),
        }
    }

    /// Find a table by name (case-insensitive).
    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// All identifiers (tables, views, columns) for autocompletion.
    pub fn completion_identifiers(&self) -> Vec<String> {
        let mut out = Vec::new();
        for t in &self.tables {
            out.push(t.name.clone());
            for c in &t.columns {
                out.push(c.name.clone());
            }
        }
        for v in &self.views {
            out.push(v.name.clone());
            for c in &v.columns {
                out.push(c.clone());
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

impl Table {
    fn from_info(info: &SchemaTableInfo) -> Self {
        let columns = info
            .columns
            .iter()
            .map(|c| {
                let references = c.foreign_key.as_ref().map(|fk| ForeignKeyRef {
                    columns: fk.columns.clone(),
                    referenced_table: fk.referenced_table.clone(),
                    referenced_columns: fk.referenced_columns.clone(),
                });
                Column {
                    name: c.name.clone(),
                    type_name: c.column_type.clone(),
                    nullable: c.nullable,
                    primary_key: c.primary_key,
                    unique: c.unique,
                    auto_increment: c.auto_increment,
                    default_sql: c.default_sql.clone(),
                    references,
                }
            })
            .collect();

        let foreign_keys = info
            .foreign_keys
            .iter()
            .map(|fk| ForeignKeyRef {
                columns: fk.columns.clone(),
                referenced_table: fk.referenced_table.clone(),
                referenced_columns: fk.referenced_columns.clone(),
            })
            .collect();

        Self {
            name: info.name.clone(),
            columns,
            primary_key_columns: info.primary_key_columns.clone(),
            foreign_keys,
            row_count: info.row_count,
            ddl: info.ddl.clone(),
        }
    }
}
