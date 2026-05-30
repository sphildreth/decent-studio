//! Conversion helpers between DecentDB [`Value`]s and display/edit strings.

use decentdb::Value;

/// Render a [`Value`] as a human-readable string for display in result grids.
///
/// `NULL` is rendered as an empty marker handled by the caller; here we return
/// the literal text `NULL` so callers can style it distinctly.
pub fn display(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Int64(v) => v.to_string(),
        Value::Float64(v) => format_float(*v),
        Value::Bool(v) => if *v { "true" } else { "false" }.to_string(),
        Value::Text(v) => v.clone(),
        Value::Blob(bytes) => format_blob(bytes),
        Value::Decimal { scaled, scale } => format_decimal(*scaled, *scale),
        Value::Uuid(bytes) => format_uuid(bytes),
        Value::TimestampMicros(micros) => format_timestamp(*micros),
        Value::TimestampTzMicros(micros) => format!("{}Z", format_timestamp(*micros)),
        Value::Geometry(bytes) => format!("GEOMETRY({} bytes)", bytes.len()),
        Value::Geography(bytes) => format!("GEOGRAPHY({} bytes)", bytes.len()),
        Value::Enum { enum_type_id, label_id } => {
            format!("enum#{enum_type_id}:{label_id}")
        }
        Value::IpAddr { family, addr } => format_ip(*family, addr),
        Value::Cidr { family, prefix_len, network } => {
            format!("{}/{}", format_ip(*family, network), prefix_len)
        }
        Value::MacAddr { len, bytes } => format_mac(*len, bytes),
        Value::DateDays(days) => format_date(*days),
        Value::TimeMicros(micros) => format_time(*micros),
        Value::Interval { months, days, micros } => {
            format!("{months} mon {days} d {micros} us")
        }
    }
}

/// Returns `true` when the value is SQL `NULL`.
pub fn is_null(value: &Value) -> bool {
    matches!(value, Value::Null)
}

/// A short label describing the runtime kind of a value, used in cell tooltips
/// and the inspector pane.
pub fn kind_label(value: &Value) -> &'static str {
    match value {
        Value::Null => "NULL",
        Value::Int64(_) => "INT64",
        Value::Float64(_) => "FLOAT64",
        Value::Bool(_) => "BOOL",
        Value::Text(_) => "TEXT",
        Value::Blob(_) => "BLOB",
        Value::Decimal { .. } => "DECIMAL",
        Value::Uuid(_) => "UUID",
        Value::TimestampMicros(_) => "TIMESTAMP",
        Value::TimestampTzMicros(_) => "TIMESTAMPTZ",
        Value::Geometry(_) => "GEOMETRY",
        Value::Geography(_) => "GEOGRAPHY",
        Value::Enum { .. } => "ENUM",
        Value::IpAddr { .. } => "IPADDR",
        Value::Cidr { .. } => "CIDR",
        Value::MacAddr { .. } => "MACADDR",
        Value::DateDays(_) => "DATE",
        Value::TimeMicros(_) => "TIME",
        Value::Interval { .. } => "INTERVAL",
    }
}

/// Quote and escape a string literal for inclusion in a SQL statement.
pub fn sql_quote_text(text: &str) -> String {
    let escaped = text.replace('\'', "''");
    format!("'{escaped}'")
}

/// Render a [`Value`] as a SQL literal suitable for INSERT/UPDATE statements.
pub fn sql_literal(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Int64(v) => v.to_string(),
        Value::Float64(v) => format_float(*v),
        Value::Bool(v) => if *v { "TRUE" } else { "FALSE" }.to_string(),
        Value::Text(v) => sql_quote_text(v),
        Value::Blob(bytes) => format!("X'{}'", hex(bytes)),
        Value::Decimal { scaled, scale } => format_decimal(*scaled, *scale),
        Value::Uuid(bytes) => sql_quote_text(&format_uuid(bytes)),
        Value::TimestampMicros(micros) => sql_quote_text(&format_timestamp(*micros)),
        Value::TimestampTzMicros(micros) => sql_quote_text(&format!("{}Z", format_timestamp(*micros))),
        Value::DateDays(days) => sql_quote_text(&format_date(*days)),
        Value::TimeMicros(micros) => sql_quote_text(&format_time(*micros)),
        Value::IpAddr { family, addr } => sql_quote_text(&format_ip(*family, addr)),
        Value::Cidr { family, prefix_len, network } => {
            sql_quote_text(&format!("{}/{}", format_ip(*family, network), prefix_len))
        }
        Value::MacAddr { len, bytes } => sql_quote_text(&format_mac(*len, bytes)),
        // Best-effort textual representation for the remaining types.
        other => sql_quote_text(&display(other)),
    }
}

/// Best-effort parse of an edited cell string back into a [`Value`], guided by
/// the declared column type name. Used by inline data editing.
pub fn parse_for_type(input: &str, column_type: &str) -> Value {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("null") || trimmed.is_empty() {
        return Value::Null;
    }
    let upper = column_type.to_ascii_uppercase();
    if upper.contains("INT") {
        if let Ok(v) = trimmed.parse::<i64>() {
            return Value::Int64(v);
        }
    }
    if upper.contains("FLOAT") || upper.contains("REAL") || upper.contains("DOUBLE") {
        if let Ok(v) = trimmed.parse::<f64>() {
            return Value::Float64(v);
        }
    }
    if upper.contains("BOOL") {
        match trimmed.to_ascii_lowercase().as_str() {
            "true" | "1" | "t" | "yes" => return Value::Bool(true),
            "false" | "0" | "f" | "no" => return Value::Bool(false),
            _ => {}
        }
    }
    Value::Text(trimmed.to_string())
}

fn format_float(v: f64) -> String {
    if v == v.trunc() && v.is_finite() && v.abs() < 1e15 {
        format!("{v:.1}")
    } else {
        v.to_string()
    }
}

fn format_decimal(scaled: i64, scale: u8) -> String {
    if scale == 0 {
        return scaled.to_string();
    }
    let negative = scaled < 0;
    let digits = scaled.unsigned_abs().to_string();
    let scale = scale as usize;
    let padded = if digits.len() <= scale {
        format!("{:0>width$}", digits, width = scale + 1)
    } else {
        digits
    };
    let split = padded.len() - scale;
    let (int_part, frac_part) = padded.split_at(split);
    let sign = if negative { "-" } else { "" };
    format!("{sign}{int_part}.{frac_part}")
}

fn format_uuid(bytes: &[u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

fn format_blob(bytes: &[u8]) -> String {
    if bytes.len() <= 32 {
        format!("0x{}", hex(bytes))
    } else {
        format!("0x{}… ({} bytes)", hex(&bytes[..16]), bytes.len())
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

const MICROS_PER_SECOND: i64 = 1_000_000;
const MICROS_PER_MINUTE: i64 = 60 * MICROS_PER_SECOND;
const MICROS_PER_HOUR: i64 = 60 * MICROS_PER_MINUTE;
const MICROS_PER_DAY: i64 = 24 * MICROS_PER_HOUR;

fn format_timestamp(micros: i64) -> String {
    let days = micros.div_euclid(MICROS_PER_DAY);
    let time = micros.rem_euclid(MICROS_PER_DAY);
    let (y, m, d) = civil_from_days(days);
    let hour = time / MICROS_PER_HOUR;
    let minute = (time % MICROS_PER_HOUR) / MICROS_PER_MINUTE;
    let second = (time % MICROS_PER_MINUTE) / MICROS_PER_SECOND;
    let frac = time % MICROS_PER_SECOND;
    if frac == 0 {
        format!("{y:04}-{m:02}-{d:02} {hour:02}:{minute:02}:{second:02}")
    } else {
        format!("{y:04}-{m:02}-{d:02} {hour:02}:{minute:02}:{second:02}.{frac:06}")
    }
}

fn format_date(days: i32) -> String {
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn format_time(micros: i64) -> String {
    let hour = micros / MICROS_PER_HOUR;
    let minute = (micros % MICROS_PER_HOUR) / MICROS_PER_MINUTE;
    let second = (micros % MICROS_PER_MINUTE) / MICROS_PER_SECOND;
    let frac = micros % MICROS_PER_SECOND;
    if frac == 0 {
        format!("{hour:02}:{minute:02}:{second:02}")
    } else {
        format!("{hour:02}:{minute:02}:{second:02}.{frac:06}")
    }
}

fn format_ip(family: u8, addr: &[u8; 16]) -> String {
    match family {
        4 => format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3]),
        6 => {
            let segments: Vec<String> = (0..8)
                .map(|i| format!("{:x}", u16::from_be_bytes([addr[i * 2], addr[i * 2 + 1]])))
                .collect();
            segments.join(":")
        }
        _ => "<invalid ip>".to_string(),
    }
}

fn format_mac(len: u8, bytes: &[u8; 8]) -> String {
    let len = (len as usize).min(8);
    bytes[..len]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Convert a day-count since the Unix epoch into (year, month, day).
/// Algorithm from Howard Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_decimal() {
        assert_eq!(format_decimal(1999, 2), "19.99");
        assert_eq!(format_decimal(-5, 1), "-0.5");
        assert_eq!(format_decimal(42, 0), "42");
    }

    #[test]
    fn formats_uuid() {
        let bytes = [
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ];
        assert_eq!(format_uuid(&bytes), "12345678-9abc-def0-1122-334455667788");
    }

    #[test]
    fn parses_typed_values() {
        assert!(matches!(parse_for_type("42", "INTEGER"), Value::Int64(42)));
        assert!(matches!(parse_for_type("", "TEXT"), Value::Null));
        assert!(matches!(parse_for_type("true", "BOOL"), Value::Bool(true)));
    }
}
