//! Mapping from SQLite declared types onto DecentDB native types.
//!
//! SQLite uses dynamic typing with "type affinity" derived from the declared
//! type name. DecentDB has a rich static type system. This module inspects the
//! declared type string and chooses the closest DecentDB native type so the
//! converted database benefits from DecentDB's compact encodings and indexing.

/// A DecentDB target type chosen for a SQLite column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DdbType {
    Int,
    Float,
    Bool,
    Text,
    Blob,
    Decimal { precision: u8, scale: u8 },
    Uuid,
    Timestamp,
    TimestampTz,
    Date,
    Time,
}

impl DdbType {
    /// The DecentDB SQL type keyword for this type.
    pub fn sql(&self) -> String {
        match self {
            DdbType::Int => "INT64".to_string(),
            DdbType::Float => "FLOAT64".to_string(),
            DdbType::Bool => "BOOL".to_string(),
            DdbType::Text => "TEXT".to_string(),
            DdbType::Blob => "BLOB".to_string(),
            DdbType::Decimal { precision, scale } => format!("DECIMAL({precision},{scale})"),
            DdbType::Uuid => "UUID".to_string(),
            DdbType::Timestamp => "TIMESTAMP".to_string(),
            DdbType::TimestampTz => "TIMESTAMPTZ".to_string(),
            DdbType::Date => "DATE".to_string(),
            DdbType::Time => "TIME".to_string(),
        }
    }
}

/// Map a SQLite declared type name to a [`DdbType`].
///
/// The matching follows SQLite's affinity rules with extra DecentDB-aware
/// heuristics layered on top:
/// - declared types containing `INT` -> INT64
/// - `CHAR`/`CLOB`/`TEXT` -> TEXT
/// - `BLOB` or empty -> BLOB (empty defaults to TEXT, see below)
/// - `REAL`/`FLOA`/`DOUB` -> FLOAT64
/// - `BOOL` -> BOOL
/// - `DECIMAL`/`NUMERIC(p,s)` -> DECIMAL(p,s)
/// - `UUID`/`GUID` -> UUID
/// - `DATETIME`/`TIMESTAMP` -> TIMESTAMP (TZ-aware if it says so)
/// - `DATE` -> DATE, `TIME` -> TIME
pub fn map_sqlite_type(decl: &str) -> DdbType {
    let raw = decl.trim();
    let upper = raw.to_ascii_uppercase();

    if upper.is_empty() {
        // SQLite columns with no declared type have BLOB affinity, but in
        // practice they usually hold text. TEXT is the safer, lossless choice.
        return DdbType::Text;
    }

    // Boolean before INT, since "BOOLEAN" does not contain "INT".
    if upper.contains("BOOL") {
        return DdbType::Bool;
    }

    // UUID / GUID.
    if upper.contains("UUID") || upper.contains("GUID") {
        return DdbType::Uuid;
    }

    // Decimal / numeric with optional precision/scale.
    if upper.starts_with("DECIMAL") || upper.starts_with("NUMERIC") {
        if let Some((p, s)) = parse_precision_scale(raw) {
            // DecentDB supports up to 18 digits of precision.
            let p = p.min(18).max(1);
            let s = s.min(p);
            return DdbType::Decimal {
                precision: p,
                scale: s,
            };
        }
        return DdbType::Decimal {
            precision: 18,
            scale: 6,
        };
    }

    // Temporal types. Check TIMESTAMP/DATETIME before DATE/TIME substrings.
    if upper.contains("TIMESTAMPTZ") || upper.contains("TIMESTAMP WITH TIME ZONE") {
        return DdbType::TimestampTz;
    }
    if upper.contains("TIMESTAMP") || upper.contains("DATETIME") {
        return DdbType::Timestamp;
    }
    if upper.contains("DATE") {
        return DdbType::Date;
    }
    if upper.contains("TIME") {
        return DdbType::Time;
    }

    // SQLite affinity rules (order matters):
    // 1. INT -> INTEGER affinity.
    if upper.contains("INT") {
        return DdbType::Int;
    }
    // 2. CHAR, CLOB, TEXT -> TEXT affinity.
    if upper.contains("CHAR") || upper.contains("CLOB") || upper.contains("TEXT") {
        return DdbType::Text;
    }
    // 3. BLOB -> BLOB affinity.
    if upper.contains("BLOB") || upper.contains("BINARY") || upper.contains("BYTEA") {
        return DdbType::Blob;
    }
    // 4. REAL, FLOA, DOUB -> REAL affinity.
    if upper.contains("REAL") || upper.contains("FLOA") || upper.contains("DOUB") {
        return DdbType::Float;
    }

    // 5. Everything else -> NUMERIC affinity; we use TEXT to stay lossless.
    DdbType::Text
}

/// Parse a `TYPE(p, s)` or `TYPE(p)` precision/scale suffix.
fn parse_precision_scale(decl: &str) -> Option<(u8, u8)> {
    let open = decl.find('(')?;
    let close = decl.find(')')?;
    if close <= open {
        return None;
    }
    let inner = &decl[open + 1..close];
    let mut parts = inner.split(',');
    let p = parts.next()?.trim().parse::<u8>().ok()?;
    let s = parts
        .next()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .unwrap_or(0);
    Some((p, s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_affinities() {
        assert_eq!(map_sqlite_type("INTEGER"), DdbType::Int);
        assert_eq!(map_sqlite_type("int"), DdbType::Int);
        assert_eq!(map_sqlite_type("VARCHAR(255)"), DdbType::Text);
        assert_eq!(map_sqlite_type("TEXT"), DdbType::Text);
        assert_eq!(map_sqlite_type("REAL"), DdbType::Float);
        assert_eq!(map_sqlite_type("DOUBLE"), DdbType::Float);
        assert_eq!(map_sqlite_type("BLOB"), DdbType::Blob);
        assert_eq!(map_sqlite_type("BOOLEAN"), DdbType::Bool);
        assert_eq!(map_sqlite_type(""), DdbType::Text);
    }

    #[test]
    fn maps_rich_types() {
        assert_eq!(map_sqlite_type("UUID"), DdbType::Uuid);
        assert_eq!(map_sqlite_type("DATETIME"), DdbType::Timestamp);
        assert_eq!(map_sqlite_type("DATE"), DdbType::Date);
        assert_eq!(map_sqlite_type("TIME"), DdbType::Time);
        assert_eq!(
            map_sqlite_type("DECIMAL(10,2)"),
            DdbType::Decimal {
                precision: 10,
                scale: 2
            }
        );
    }

    #[test]
    fn generates_sql() {
        assert_eq!(DdbType::Int.sql(), "INT64");
        assert_eq!(
            DdbType::Decimal {
                precision: 10,
                scale: 2
            }
            .sql(),
            "DECIMAL(10,2)"
        );
    }
}
