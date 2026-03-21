use rusqlite::Connection;

use super::types::GalleryEntry;

/// Create the persons and face_embeddings tables if they don't exist.
pub fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS persons (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'customer',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS face_embeddings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            person_id INTEGER NOT NULL REFERENCES persons(id) ON DELETE CASCADE,
            embedding BLOB NOT NULL,
            enrolled_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_embeddings_person ON face_embeddings(person_id);
        CREATE INDEX IF NOT EXISTS idx_embeddings_expires ON face_embeddings(expires_at);",
    )
}

/// Load all non-expired gallery entries from SQLite.
///
/// Deserializes embedding BLOBs from little-endian f32 bytes.
pub fn load_gallery(conn: &Connection) -> rusqlite::Result<Vec<GalleryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, e.embedding
         FROM face_embeddings e
         JOIN persons p ON p.id = e.person_id
         WHERE e.expires_at > datetime('now')",
    )?;

    let entries = stmt
        .query_map([], |row| {
            let person_id: i64 = row.get(0)?;
            let person_name: String = row.get(1)?;
            let blob: Vec<u8> = row.get(2)?;

            let mut embedding = [0.0_f32; 512];
            if blob.len() == 512 * 4 {
                for (i, chunk) in blob.chunks_exact(4).enumerate() {
                    embedding[i] =
                        f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                }
            }

            Ok(GalleryEntry {
                person_id,
                person_name,
                embedding,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(entries)
}

/// Insert a new person and return their row ID.
pub fn insert_person(conn: &Connection, name: &str, role: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO persons (name, role) VALUES (?1, ?2)",
        rusqlite::params![name, role],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert a face embedding for a person with a retention period.
///
/// Serializes the 512-D embedding as little-endian f32 bytes (2048 bytes total).
pub fn insert_embedding(
    conn: &Connection,
    person_id: i64,
    embedding: &[f32; 512],
    retention_days: u64,
) -> rusqlite::Result<()> {
    let mut blob = Vec::with_capacity(512 * 4);
    for val in embedding.iter() {
        blob.extend_from_slice(&val.to_le_bytes());
    }

    let expires_sql = format!("datetime('now', '+{retention_days} days')");

    conn.execute(
        &format!(
            "INSERT INTO face_embeddings (person_id, embedding, expires_at) VALUES (?1, ?2, {expires_sql})"
        ),
        rusqlite::params![person_id, blob],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_tables_succeeds() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        let result = create_tables(&conn);
        assert!(result.is_ok(), "create_tables should succeed: {:?}", result);
    }

    #[test]
    fn test_insert_and_load_gallery() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        create_tables(&conn).expect("create tables");

        let person_id = insert_person(&conn, "TestUser", "staff").expect("insert person");

        let mut embedding = [0.0_f32; 512];
        embedding[0] = 1.0;
        embedding[511] = -0.5;

        insert_embedding(&conn, person_id, &embedding, 90).expect("insert embedding");

        let entries = load_gallery(&conn).expect("load gallery");
        assert_eq!(entries.len(), 1, "should have 1 gallery entry");
        assert_eq!(entries[0].person_id, person_id);
        assert_eq!(entries[0].person_name, "TestUser");
        assert!(
            (entries[0].embedding[0] - 1.0).abs() < 1e-6,
            "embedding[0] should be 1.0, got {}",
            entries[0].embedding[0]
        );
        assert!(
            (entries[0].embedding[511] - (-0.5)).abs() < 1e-6,
            "embedding[511] should be -0.5, got {}",
            entries[0].embedding[511]
        );
    }
}
