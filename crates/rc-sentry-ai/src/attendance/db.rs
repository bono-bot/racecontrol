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

/// A staff shift entry (one per person per day).
#[derive(Debug, Clone, Serialize)]
pub struct ShiftEntry {
    pub id: i64,
    pub person_id: i64,
    pub person_name: String,
    pub day: String,
    pub clock_in: String,
    pub clock_out: Option<String>,
    pub shift_minutes: Option<i64>,
}

/// A person currently present (seen within recency window).
#[derive(Debug, Clone, Serialize)]
pub struct PresentPerson {
    pub person_id: i64,
    pub person_name: String,
    pub last_seen: String,
    pub sighting_count: i64,
}

/// Result of an upsert_shift operation.
#[derive(Debug, Clone, PartialEq)]
pub enum ShiftAction {
    /// First recognition of the day -- clock-in recorded.
    ClockIn,
    /// Subsequent recognition -- clock_out updated.
    Update,
}

/// Create the attendance_log and staff_shifts tables if they do not exist.
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
        CREATE INDEX IF NOT EXISTS idx_attendance_day ON attendance_log(day);

        CREATE TABLE IF NOT EXISTS staff_shifts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            person_id INTEGER NOT NULL,
            person_name TEXT NOT NULL,
            day TEXT NOT NULL,
            clock_in TEXT NOT NULL,
            clock_out TEXT,
            shift_minutes INTEGER,
            UNIQUE(person_id, day)
        );

        CREATE INDEX IF NOT EXISTS idx_shifts_person_day ON staff_shifts(person_id, day);
        CREATE INDEX IF NOT EXISTS idx_shifts_day ON staff_shifts(day);",
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
#[allow(dead_code)]
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

/// Upsert a staff shift for a given person and day.
///
/// - If no row exists: INSERT with clock_in = timestamp. Returns `ShiftAction::ClockIn`.
/// - If row exists: UPDATE clock_out = timestamp, compute shift_minutes. Returns `ShiftAction::Update`.
pub fn upsert_shift(
    conn: &Connection,
    person_id: i64,
    person_name: &str,
    day: &str,
    timestamp: &str,
) -> rusqlite::Result<ShiftAction> {
    let existing: Option<String> = conn
        .prepare("SELECT clock_in FROM staff_shifts WHERE person_id = ?1 AND day = ?2")?
        .query_row(rusqlite::params![person_id, day], |row| row.get(0))
        .ok();

    match existing {
        None => {
            conn.execute(
                "INSERT INTO staff_shifts (person_id, person_name, day, clock_in)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![person_id, person_name, day, timestamp],
            )?;
            Ok(ShiftAction::ClockIn)
        }
        Some(clock_in) => {
            // Compute shift_minutes from clock_in to timestamp
            let minutes = compute_shift_minutes(&clock_in, timestamp);
            conn.execute(
                "UPDATE staff_shifts SET clock_out = ?1, shift_minutes = ?2
                 WHERE person_id = ?3 AND day = ?4",
                rusqlite::params![timestamp, minutes, person_id, day],
            )?;
            Ok(ShiftAction::Update)
        }
    }
}

/// Compute the difference in minutes between two ISO 8601 datetime strings.
fn compute_shift_minutes(clock_in: &str, clock_out: &str) -> Option<i64> {
    use chrono::NaiveDateTime;
    let fmt = "%Y-%m-%dT%H:%M:%S%.f%:z";
    let t_in = NaiveDateTime::parse_from_str(clock_in, fmt)
        .or_else(|_| NaiveDateTime::parse_from_str(clock_in, "%Y-%m-%d %H:%M:%S"))
        .ok()?;
    let t_out = NaiveDateTime::parse_from_str(clock_out, fmt)
        .or_else(|_| NaiveDateTime::parse_from_str(clock_out, "%Y-%m-%d %H:%M:%S"))
        .ok()?;
    let diff = t_out.signed_duration_since(t_in);
    Some(diff.num_minutes())
}

/// Get a single shift entry for a person on a given day.
#[allow(dead_code)]
pub fn get_shift(conn: &Connection, person_id: i64, day: &str) -> rusqlite::Result<Option<ShiftEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, person_id, person_name, day, clock_in, clock_out, shift_minutes
         FROM staff_shifts WHERE person_id = ?1 AND day = ?2",
    )?;
    let mut rows = stmt.query_map(rusqlite::params![person_id, day], |row| {
        Ok(ShiftEntry {
            id: row.get(0)?,
            person_id: row.get(1)?,
            person_name: row.get(2)?,
            day: row.get(3)?,
            clock_in: row.get(4)?,
            clock_out: row.get(5)?,
            shift_minutes: row.get(6)?,
        })
    })?;
    match rows.next() {
        Some(result) => Ok(Some(result?)),
        None => Ok(None),
    }
}

/// Get all shift entries for a given day.
pub fn get_shifts_for_day(conn: &Connection, day: &str) -> rusqlite::Result<Vec<ShiftEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, person_id, person_name, day, clock_in, clock_out, shift_minutes
         FROM staff_shifts WHERE day = ?1 ORDER BY clock_in",
    )?;
    let entries = stmt
        .query_map(rusqlite::params![day], |row| {
            Ok(ShiftEntry {
                id: row.get(0)?,
                person_id: row.get(1)?,
                person_name: row.get(2)?,
                day: row.get(3)?,
                clock_in: row.get(4)?,
                clock_out: row.get(5)?,
                shift_minutes: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}

/// Get the last N shifts for a person, ordered by day DESC.
pub fn get_shifts_for_person(
    conn: &Connection,
    person_id: i64,
    limit: u32,
) -> rusqlite::Result<Vec<ShiftEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, person_id, person_name, day, clock_in, clock_out, shift_minutes
         FROM staff_shifts WHERE person_id = ?1 ORDER BY day DESC LIMIT ?2",
    )?;
    let entries = stmt
        .query_map(rusqlite::params![person_id, limit], |row| {
            Ok(ShiftEntry {
                id: row.get(0)?,
                person_id: row.get(1)?,
                person_name: row.get(2)?,
                day: row.get(3)?,
                clock_in: row.get(4)?,
                clock_out: row.get(5)?,
                shift_minutes: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}

/// Get all persons seen since the given timestamp on the given day.
pub fn get_present_persons(
    conn: &Connection,
    day: &str,
    since: &str,
) -> rusqlite::Result<Vec<PresentPerson>> {
    let mut stmt = conn.prepare(
        "SELECT person_id, person_name, MAX(logged_at) as last_seen, COUNT(*) as sighting_count
         FROM attendance_log
         WHERE day = ?1 AND logged_at >= ?2
         GROUP BY person_id
         ORDER BY last_seen DESC",
    )?;
    let entries = stmt
        .query_map(rusqlite::params![day, since], |row| {
            Ok(PresentPerson {
                person_id: row.get(0)?,
                person_name: row.get(1)?,
                last_seen: row.get(2)?,
                sighting_count: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}

/// Check if a person has the 'staff' role in the persons table.
///
/// Returns `false` if the person does not exist or has a different role.
pub fn is_staff(conn: &Connection, person_id: i64) -> rusqlite::Result<bool> {
    let role: Option<String> = conn
        .prepare("SELECT role FROM persons WHERE id = ?1")?
        .query_row(rusqlite::params![person_id], |row| row.get(0))
        .ok();
    Ok(role.as_deref() == Some("staff"))
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

    /// Helper to create the persons table for shift/is_staff tests.
    fn setup_persons_table(conn: &Connection) {
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
    }

    fn insert_test_person(conn: &Connection, name: &str, role: &str) -> i64 {
        conn.execute(
            "INSERT INTO persons (name, role) VALUES (?1, ?2)",
            rusqlite::params![name, role],
        )
        .expect("insert person");
        conn.last_insert_rowid()
    }

    #[test]
    fn test_upsert_clock_in() {
        let conn = setup_db();
        let action = upsert_shift(&conn, 1, "Alice", "2026-03-21", "2026-03-21 09:00:00")
            .expect("upsert");
        assert_eq!(action, ShiftAction::ClockIn);

        let shift = get_shift(&conn, 1, "2026-03-21").expect("get").expect("should exist");
        assert_eq!(shift.person_name, "Alice");
        assert_eq!(shift.clock_in, "2026-03-21 09:00:00");
        assert!(shift.clock_out.is_none());
        assert!(shift.shift_minutes.is_none());
    }

    #[test]
    fn test_upsert_updates_clock_out() {
        let conn = setup_db();
        upsert_shift(&conn, 1, "Alice", "2026-03-21", "2026-03-21 09:00:00")
            .expect("clock in");

        let action = upsert_shift(&conn, 1, "Alice", "2026-03-21", "2026-03-21 17:30:00")
            .expect("update");
        assert_eq!(action, ShiftAction::Update);

        let shift = get_shift(&conn, 1, "2026-03-21").expect("get").expect("should exist");
        assert_eq!(shift.clock_out.as_deref(), Some("2026-03-21 17:30:00"));
        assert_eq!(shift.shift_minutes, Some(510)); // 8h30m = 510 minutes
    }

    #[test]
    fn test_get_shifts_for_day() {
        let conn = setup_db();
        upsert_shift(&conn, 1, "Alice", "2026-03-21", "2026-03-21 09:00:00")
            .expect("upsert");
        upsert_shift(&conn, 2, "Bob", "2026-03-21", "2026-03-21 10:00:00")
            .expect("upsert");
        upsert_shift(&conn, 3, "Charlie", "2026-03-22", "2026-03-22 09:00:00")
            .expect("upsert");

        let shifts = get_shifts_for_day(&conn, "2026-03-21").expect("query");
        assert_eq!(shifts.len(), 2);
        assert_eq!(shifts[0].person_name, "Alice");
        assert_eq!(shifts[1].person_name, "Bob");
    }

    #[test]
    fn test_shift_unique_per_person_per_day() {
        let conn = setup_db();
        upsert_shift(&conn, 1, "Alice", "2026-03-21", "2026-03-21 09:00:00")
            .expect("first upsert");
        // Second upsert for same person+day should UPDATE, not create duplicate
        upsert_shift(&conn, 1, "Alice", "2026-03-21", "2026-03-21 18:00:00")
            .expect("second upsert");

        let shifts = get_shifts_for_day(&conn, "2026-03-21").expect("query");
        assert_eq!(shifts.len(), 1, "should only have one shift per person per day");
    }

    #[test]
    fn test_get_shifts_for_person() {
        let conn = setup_db();
        upsert_shift(&conn, 1, "Alice", "2026-03-19", "2026-03-19 09:00:00").expect("upsert");
        upsert_shift(&conn, 1, "Alice", "2026-03-20", "2026-03-20 09:00:00").expect("upsert");
        upsert_shift(&conn, 1, "Alice", "2026-03-21", "2026-03-21 09:00:00").expect("upsert");

        let shifts = get_shifts_for_person(&conn, 1, 2).expect("query");
        assert_eq!(shifts.len(), 2);
        // Should be ordered by day DESC
        assert_eq!(shifts[0].day, "2026-03-21");
        assert_eq!(shifts[1].day, "2026-03-20");
    }

    #[test]
    fn test_get_present_persons() {
        let conn = setup_db();

        // Insert entries with explicit timestamps
        conn.execute(
            "INSERT INTO attendance_log (person_id, person_name, camera_id, confidence, logged_at, day)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![1, "Alice", "entrance", 0.95, "2026-03-21 10:00:00", "2026-03-21"],
        ).expect("insert");
        conn.execute(
            "INSERT INTO attendance_log (person_id, person_name, camera_id, confidence, logged_at, day)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![1, "Alice", "reception", 0.92, "2026-03-21 10:15:00", "2026-03-21"],
        ).expect("insert");
        conn.execute(
            "INSERT INTO attendance_log (person_id, person_name, camera_id, confidence, logged_at, day)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![2, "Bob", "entrance", 0.88, "2026-03-21 09:30:00", "2026-03-21"],
        ).expect("insert");

        // Since 10:00 — should find Alice (2 sightings) but not Bob (09:30 < 10:00)
        let present = get_present_persons(&conn, "2026-03-21", "2026-03-21 10:00:00")
            .expect("query");
        assert_eq!(present.len(), 1, "only Alice should be present since 10:00");
        assert_eq!(present[0].person_name, "Alice");
        assert_eq!(present[0].sighting_count, 2);
        assert_eq!(present[0].last_seen, "2026-03-21 10:15:00");

        // Since 09:00 — should find both
        let present_all = get_present_persons(&conn, "2026-03-21", "2026-03-21 09:00:00")
            .expect("query");
        assert_eq!(present_all.len(), 2);
    }

    #[test]
    fn test_is_staff() {
        let conn = setup_db();
        setup_persons_table(&conn);

        let staff_id = insert_test_person(&conn, "StaffMember", "staff");
        let customer_id = insert_test_person(&conn, "Customer", "customer");

        assert!(is_staff(&conn, staff_id).expect("query"), "staff role should return true");
        assert!(!is_staff(&conn, customer_id).expect("query"), "customer role should return false");
        assert!(!is_staff(&conn, 9999).expect("query"), "non-existent person should return false");
    }
}
