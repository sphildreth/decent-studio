//! Developer helper: create a small populated DecentDB database for manual
//! testing of DecentDB Studio.
//!
//! Usage: `cargo run --example seed -- /path/to/demo.ddb`

use decentdb_studio::db::Connection;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: seed <path-to-database.ddb>");
    let _ = std::fs::remove_file(&path);

    let c = Connection::open(&path).expect("create database");
    c.execute(
        "CREATE TABLE authors (id INT64 PRIMARY KEY, name TEXT NOT NULL, verified BOOL, rating FLOAT64)",
    )
    .unwrap();
    c.execute(
        "CREATE TABLE books (id INT64 PRIMARY KEY, author_id INT64 REFERENCES authors(id), \
         title TEXT, price DECIMAL(10,2), published TIMESTAMP)",
    )
    .unwrap();
    c.execute(
        "CREATE TABLE reviews (id INT64 PRIMARY KEY, book_id INT64 REFERENCES books(id), \
         stars INT64, body TEXT)",
    )
    .unwrap();
    c.execute("CREATE INDEX idx_books_author ON books(author_id)")
        .unwrap();

    c.execute("INSERT INTO authors (id,name,verified,rating) VALUES (1,'Ada Lovelace',true,4.9)")
        .unwrap();
    c.execute("INSERT INTO authors (id,name,verified,rating) VALUES (2,'Alan Turing',false,4.7)")
        .unwrap();
    c.execute(
        "INSERT INTO books (id,author_id,title,price,published) \
         VALUES (1,1,'Analytical Engine Notes',19.99,'1843-10-01 09:00:00')",
    )
    .unwrap();
    c.execute(
        "INSERT INTO books (id,author_id,title,price,published) \
         VALUES (2,2,'On Computable Numbers',24.50,'1936-05-28 12:00:00')",
    )
    .unwrap();
    c.execute("INSERT INTO reviews (id,book_id,stars,body) VALUES (1,1,5,'Visionary')")
        .unwrap();

    c.checkpoint().unwrap();
    println!("seeded {path}");
}
