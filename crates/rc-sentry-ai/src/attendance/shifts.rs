use rusqlite::Connection;

use super::db::{is_staff, upsert_shift, ShiftAction};

/// Process a recognition event for potential staff shift tracking.
///
/// - If the person is not staff, returns `Ok(None)`.
/// - If staff, upserts the shift and returns the action taken.
///
/// This is a pure synchronous function -- called from within `spawn_blocking`.
pub fn process_staff_recognition(
    conn: &Connection,
    person_id: i64,
    person_name: &str,
    day: &str,
    timestamp: &str,
    _min_shift_hours: u64,
) -> rusqlite::Result<Option<ShiftAction>> {
    if !is_staff(conn, person_id)? {
        return Ok(None);
    }
    let action = upsert_shift(conn, person_id, person_name, day, timestamp)?;
    Ok(Some(action))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        super::super::db::create_tables(&conn).expect("create attendance tables");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS persons (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'customer',
                phone TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .expect("create persons table");
        conn
    }

    fn insert_person(conn: &Connection, name: &str, role: &str) -> i64 {
        conn.execute(
            "INSERT INTO persons (name, role) VALUES (?1, ?2)",
            rusqlite::params![name, role],
        )
        .expect("insert person");
        conn.last_insert_rowid()
    }

    #[test]
    fn test_non_staff_returns_none() {
        let conn = setup_db();
        let customer_id = insert_person(&conn, "Customer", "customer");
        let result = process_staff_recognition(&conn, customer_id, "Customer", "2026-03-21", "2026-03-21 09:00:00", 4)
            .expect("process");
        assert!(result.is_none(), "non-staff should return None");
    }

    #[test]
    fn test_staff_clock_in() {
        let conn = setup_db();
        let staff_id = insert_person(&conn, "StaffMember", "staff");
        let result = process_staff_recognition(&conn, staff_id, "StaffMember", "2026-03-21", "2026-03-21 09:00:00", 4)
            .expect("process");
        assert_eq!(result, Some(ShiftAction::ClockIn));
    }

    #[test]
    fn test_staff_update() {
        let conn = setup_db();
        let staff_id = insert_person(&conn, "StaffMember", "staff");
        process_staff_recognition(&conn, staff_id, "StaffMember", "2026-03-21", "2026-03-21 09:00:00", 4)
            .expect("clock in");
        let result = process_staff_recognition(&conn, staff_id, "StaffMember", "2026-03-21", "2026-03-21 17:00:00", 4)
            .expect("update");
        assert_eq!(result, Some(ShiftAction::Update));
    }

    #[test]
    fn test_nonexistent_person_returns_none() {
        let conn = setup_db();
        let result = process_staff_recognition(&conn, 9999, "Nobody", "2026-03-21", "2026-03-21 09:00:00", 4)
            .expect("process");
        assert!(result.is_none(), "non-existent person should return None");
    }
}
