use rusqlite::Connection;
use serde::Serialize;

use super::types::GalleryEntry;

/// Information about a registered person.
#[derive(Debug, Clone, Serialize)]
pub struct PersonInfo {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub phone: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Create the persons and face_embeddings tables if they don't exist.
pub fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS persons (
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
    )?;

    // Idempotent migration: add phone column if it doesn't exist
    let has_phone: bool = conn.prepare("SELECT phone FROM persons LIMIT 0").is_ok();
    if !has_phone {
        conn.execute(
            "ALTER TABLE persons ADD COLUMN phone TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }

    Ok(())
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
pub fn insert_person(
    conn: &Connection,
    name: &str,
    role: &str,
    phone: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO persons (name, role, phone) VALUES (?1, ?2, ?3)",
        rusqlite::params![name, role, phone],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Get a single person by ID.
pub fn get_person(conn: &Connection, person_id: i64) -> rusqlite::Result<Option<PersonInfo>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, role, phone, created_at, updated_at FROM persons WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(rusqlite::params![person_id], |row| {
        Ok(PersonInfo {
            id: row.get(0)?,
            name: row.get(1)?,
            role: row.get(2)?,
            phone: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;
    match rows.next() {
        Some(result) => Ok(Some(result?)),
        None => Ok(None),
    }
}

/// List all persons, ordered by name.
pub fn list_persons(conn: &Connection) -> rusqlite::Result<Vec<PersonInfo>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, role, phone, created_at, updated_at FROM persons ORDER BY name",
    )?;
    let persons = stmt
        .query_map([], |row| {
            Ok(PersonInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                role: row.get(2)?,
                phone: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(persons)
}

/// Update a person's name, role, and phone. Returns true if a row was updated.
pub fn update_person(
    conn: &Connection,
    person_id: i64,
    name: &str,
    role: &str,
    phone: &str,
) -> rusqlite::Result<bool> {
    let rows = conn.execute(
        "UPDATE persons SET name = ?1, role = ?2, phone = ?3, updated_at = datetime('now') WHERE id = ?4",
        rusqlite::params![name, role, phone, person_id],
    )?;
    Ok(rows > 0)
}

/// Delete a person and their embeddings (via CASCADE). Returns true if a row was deleted.
pub fn delete_person(conn: &Connection, person_id: i64) -> rusqlite::Result<bool> {
    // SQLite defaults foreign_keys OFF per-connection, enable for CASCADE
    conn.execute_batch("PRAGMA foreign_keys = ON")?;
    let rows = conn.execute(
        "DELETE FROM persons WHERE id = ?1",
        rusqlite::params![person_id],
    )?;
    Ok(rows > 0)
}

/// Count the number of embeddings for a person.
pub fn embedding_count(conn: &Connection, person_id: i64) -> rusqlite::Result<u64> {
    conn.query_row(
        "SELECT COUNT(*) FROM face_embeddings WHERE person_id = ?1",
        rusqlite::params![person_id],
        |row| {
            let count: i64 = row.get(0)?;
            Ok(count as u64)
        },
    )
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

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        create_tables(&conn).expect("create tables");
        conn
    }

    #[test]
    fn test_create_tables_succeeds() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        let result = create_tables(&conn);
        assert!(result.is_ok(), "create_tables should succeed: {:?}", result);
    }

    #[test]
    fn test_insert_and_load_gallery() {
        let conn = setup_db();

        let person_id = insert_person(&conn, "TestUser", "staff", "").expect("insert person");

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

    #[test]
    fn test_phone_column_migration() {
        let conn = setup_db();
        // Phone column should exist after create_tables
        let has_phone: bool = conn.prepare("SELECT phone FROM persons LIMIT 0").is_ok();
        assert!(has_phone, "phone column should exist");

        // Calling create_tables again should be idempotent
        let result = create_tables(&conn);
        assert!(result.is_ok(), "second create_tables should succeed (idempotent)");
    }

    #[test]
    fn test_insert_person_with_phone() {
        let conn = setup_db();
        let id = insert_person(&conn, "Alice", "staff", "+91-9876543210").expect("insert");
        let person = get_person(&conn, id).expect("get").expect("should exist");
        assert_eq!(person.phone, "+91-9876543210");
    }

    #[test]
    fn test_get_person() {
        let conn = setup_db();
        let id = insert_person(&conn, "Bob", "customer", "").expect("insert");
        let person = get_person(&conn, id).expect("get").expect("should exist");
        assert_eq!(person.id, id);
        assert_eq!(person.name, "Bob");
        assert_eq!(person.role, "customer");
        assert_eq!(person.phone, "");

        // Non-existent ID returns None
        let none = get_person(&conn, 9999).expect("get");
        assert!(none.is_none(), "non-existent ID should return None");
    }

    #[test]
    fn test_list_persons() {
        let conn = setup_db();
        insert_person(&conn, "Charlie", "customer", "").expect("insert");
        insert_person(&conn, "Alice", "staff", "").expect("insert");
        insert_person(&conn, "Bob", "customer", "").expect("insert");

        let persons = list_persons(&conn).expect("list");
        assert_eq!(persons.len(), 3);
        // Should be ordered by name
        assert_eq!(persons[0].name, "Alice");
        assert_eq!(persons[1].name, "Bob");
        assert_eq!(persons[2].name, "Charlie");
    }

    #[test]
    fn test_update_person() {
        let conn = setup_db();
        let id = insert_person(&conn, "OldName", "customer", "").expect("insert");

        let updated = update_person(&conn, id, "NewName", "staff", "+91-1234567890").expect("update");
        assert!(updated, "should return true for existing person");

        let person = get_person(&conn, id).expect("get").expect("should exist");
        assert_eq!(person.name, "NewName");
        assert_eq!(person.role, "staff");
        assert_eq!(person.phone, "+91-1234567890");

        // Update non-existent returns false
        let not_updated = update_person(&conn, 9999, "X", "Y", "Z").expect("update");
        assert!(!not_updated, "should return false for non-existent person");
    }

    #[test]
    fn test_delete_person() {
        let conn = setup_db();
        let id = insert_person(&conn, "ToDelete", "customer", "").expect("insert");

        let mut emb = [0.0_f32; 512];
        emb[0] = 1.0;
        insert_embedding(&conn, id, &emb, 90).expect("insert embedding");

        let deleted = delete_person(&conn, id).expect("delete");
        assert!(deleted, "should return true for existing person");

        // Person should be gone
        let person = get_person(&conn, id).expect("get");
        assert!(person.is_none(), "person should be deleted");

        // Embeddings should be cascaded
        let entries = load_gallery(&conn).expect("load");
        assert!(entries.is_empty(), "embeddings should be cascade deleted");

        // Delete non-existent returns false
        let not_deleted = delete_person(&conn, 9999).expect("delete");
        assert!(!not_deleted, "should return false for non-existent person");
    }

    #[test]
    fn test_embedding_count() {
        let conn = setup_db();
        let id = insert_person(&conn, "Counter", "customer", "").expect("insert");

        assert_eq!(embedding_count(&conn, id).expect("count"), 0);

        let emb = [0.0_f32; 512];
        for _ in 0..3 {
            insert_embedding(&conn, id, &emb, 90).expect("insert embedding");
        }

        assert_eq!(embedding_count(&conn, id).expect("count"), 3);
    }
}
