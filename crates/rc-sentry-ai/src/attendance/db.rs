use rusqlite::Connection;
use serde::Serialize;

/// A single attendance log entry.
#[derive(Debug, Clone, Serialize)]
pub struct AttendanceEntry {
    pub id: i64,
    pub person_id: i64,
    pub person_name: String,
    pub camera_id: String,
    pub confidence: f32,
    pub logged_at: String,
    pub day: String,
}

/// Create the attendance_log table if it does not exist.
pub fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS attendance_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            person_id INTEGER NOT NULL,
            person_name TEXT NOT NULL,
            camera_id TEXT NOT NULL,
            confidence REAL NOT NULL,
            logged_at TEXT NOT NULL DEFAULT (datetime('now')),
            day TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_attendance_person_day ON attendance_log(person_id, day);
        CREATE INDEX IF NOT EXISTS idx_attendance_day ON attendance_log(day);",
    )?;
    Ok(())
}

/// Insert an attendance entry and return the row ID.
pub fn insert_attendance(
    conn: &Connection,
    person_id: i64,
    person_name: &str,
    camera_id: &str,
    confidence: f32,
    day: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO attendance_log (person_id, person_name, camera_id, confidence, day)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![person_id, person_name, camera_id, confidence, day],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Get all attendance entries for a given day (format "YYYY-MM-DD").
pub fn get_attendance_for_day(conn: &Connection, day: &str) -> rusqlite::Result<Vec<AttendanceEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, person_id, person_name, camera_id, confidence, logged_at, day
         FROM attendance_log WHERE day = ?1 ORDER BY logged_at",
    )?;
    let entries = stmt
        .query_map(rusqlite::params![day], |row| {
            Ok(AttendanceEntry {
                id: row.get(0)?,
                person_id: row.get(1)?,
                person_name: row.get(2)?,
                camera_id: row.get(3)?,
                confidence: row.get(4)?,
                logged_at: row.get(5)?,
                day: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}

/// Get the most recent `logged_at` timestamp for a person on a given day.
pub fn get_last_seen(
    conn: &Connection,
    person_id: i64,
    day: &str,
) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT logged_at FROM attendance_log
         WHERE person_id = ?1 AND day = ?2
         ORDER BY logged_at DESC LIMIT 1",
    )?;
    let mut rows = stmt.query_map(rusqlite::params![person_id, day], |row| {
        row.get::<_, String>(0)
    })?;
    match rows.next() {
        Some(result) => Ok(Some(result?)),
        None => Ok(None),
    }
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
    fn test_create_tables() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        let result = create_tables(&conn);
        assert!(result.is_ok(), "create_tables should succeed: {:?}", result);
        // Idempotent: calling again should succeed
        let result2 = create_tables(&conn);
        assert!(result2.is_ok(), "second create_tables should succeed");
    }

    #[test]
    fn test_insert_and_query() {
        let conn = setup_db();

        let id = insert_attendance(&conn, 1, "Alice", "entrance", 0.95, "2026-03-21")
            .expect("insert");
        assert!(id > 0, "should return positive row ID");

        insert_attendance(&conn, 2, "Bob", "reception", 0.88, "2026-03-21")
            .expect("insert");

        let entries = get_attendance_for_day(&conn, "2026-03-21").expect("query");
        assert_eq!(entries.len(), 2, "should have 2 entries");
        assert_eq!(entries[0].person_name, "Alice");
        assert_eq!(entries[1].person_name, "Bob");

        // Different day should return empty
        let empty = get_attendance_for_day(&conn, "2026-03-22").expect("query");
        assert!(empty.is_empty(), "different day should have no entries");
    }

    #[test]
    fn test_get_last_seen_none() {
        let conn = setup_db();
        let result = get_last_seen(&conn, 999, "2026-03-21").expect("query");
        assert!(result.is_none(), "non-existent person should return None");
    }

    #[test]
    fn test_get_last_seen_some() {
        let conn = setup_db();

        insert_attendance(&conn, 1, "Alice", "entrance", 0.95, "2026-03-21")
            .expect("insert");
        insert_attendance(&conn, 1, "Alice", "reception", 0.92, "2026-03-21")
            .expect("insert");

        let result = get_last_seen(&conn, 1, "2026-03-21").expect("query");
        assert!(result.is_some(), "should find last_seen for person 1");
    }
}
