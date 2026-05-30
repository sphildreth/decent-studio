//! End-to-end integration test for the SQLite -> DecentDB conversion path.
//!
//! Builds a SQLite database that exercises a range of type affinities and a
//! foreign-key relationship, converts it into a DecentDB database, then queries
//! the result through the same [`Connection`] the GUI uses.

use decentdb_studio::convert::{convert, ConvertOptions};
use decentdb_studio::db::Connection;

fn temp_path(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("ddb_studio_it_{}_{}", std::process::id(), name));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn converts_realistic_sqlite_database() {
    let sqlite_path = temp_path("source.sqlite");

    // Build a representative SQLite database.
    {
        let conn = rusqlite::Connection::open(&sqlite_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE authors (
                 id        INTEGER PRIMARY KEY,
                 name      TEXT NOT NULL,
                 verified  BOOLEAN DEFAULT 0,
                 rating    REAL,
                 joined    DATE
             );
             CREATE TABLE books (
                 id         INTEGER PRIMARY KEY,
                 author_id  INTEGER NOT NULL REFERENCES authors(id),
                 title      VARCHAR(255) NOT NULL,
                 price      DECIMAL(10,2),
                 published  DATETIME,
                 cover      BLOB
             );
             CREATE INDEX idx_books_author ON books(author_id);
             INSERT INTO authors (name, verified, rating, joined)
                 VALUES ('Ada Lovelace', 1, 4.9, '1815-12-10');
             INSERT INTO authors (name, verified, rating, joined)
                 VALUES ('Alan Turing', 0, 4.7, '1912-06-23');
             INSERT INTO books (author_id, title, price, published)
                 VALUES (1, 'Notes on the Analytical Engine', 19.99, '1843-10-01 09:00:00');
             INSERT INTO books (author_id, title, price, published)
                 VALUES (2, 'On Computable Numbers', 24.50, '1936-05-28 12:00:00');",
        )
        .unwrap();
    }

    // Convert into an in-memory DecentDB database.
    let target = Connection::open_memory().unwrap();
    let report = convert(&sqlite_path, &target, &ConvertOptions::default(), |_| {}).unwrap();

    assert_eq!(report.tables, 2, "both tables converted");
    assert_eq!(report.rows, 4, "all rows copied");
    assert!(report.indexes >= 1, "index recreated");

    // Schema should expose DecentDB-native types.
    let schema = target.schema().unwrap();
    let books = schema.table("books").expect("books table present");
    let price = books
        .columns
        .iter()
        .find(|c| c.name == "price")
        .expect("price column");
    assert!(
        price.type_name.to_uppercase().contains("DECIMAL"),
        "price mapped to DECIMAL, got {}",
        price.type_name
    );
    let verified_col = schema
        .table("authors")
        .unwrap()
        .columns
        .iter()
        .find(|c| c.name == "verified")
        .unwrap()
        .clone();
    assert!(
        verified_col.type_name.to_uppercase().contains("BOOL"),
        "verified mapped to BOOL, got {}",
        verified_col.type_name
    );

    // Data should be queryable through the studio connection.
    let rs = target
        .execute("SELECT a.name, b.title FROM books b JOIN authors a ON a.id = b.author_id ORDER BY b.id")
        .unwrap();
    assert_eq!(rs.rows.len(), 2);

    // Explain should produce a non-empty plan.
    let plan = target
        .explain("SELECT * FROM books WHERE author_id = 1")
        .unwrap();
    assert!(!plan.is_empty(), "explain produced a plan");

    let _ = std::fs::remove_file(&sqlite_path);
}
