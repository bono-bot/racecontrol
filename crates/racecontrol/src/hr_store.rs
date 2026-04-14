//! HR persistence: employees, attendance tracking, and payroll calculations.
//!
//! Extracted from `maintenance_store.rs` — Phase 13/14/17 (v29.0).

use crate::maintenance_models::{
    AttendanceRecord, Employee, EmployeePayroll, PayrollSummary, StaffRole,
};
use sqlx::SqlitePool;
use uuid::Uuid;

// ===========================================================================
// Phase 13 (v29.0): HR employee database
// ===========================================================================

/// Create the employees and attendance tables.
pub async fn init_hr_tables(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS employees (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            role TEXT NOT NULL,
            skills TEXT DEFAULT '[]',
            hourly_rate_paise INTEGER DEFAULT 0,
            phone TEXT DEFAULT '',
            is_active INTEGER DEFAULT 1,
            face_enrollment_id TEXT,
            hired_at TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // Phase 14: attendance_records
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS attendance_records (
            id TEXT PRIMARY KEY,
            employee_id TEXT NOT NULL,
            date TEXT NOT NULL,
            clock_in TEXT,
            clock_out TEXT,
            source TEXT DEFAULT 'manual',
            hours_worked REAL DEFAULT 0,
            FOREIGN KEY (employee_id) REFERENCES employees(id)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_attendance_date ON attendance_records(date)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_attendance_employee ON attendance_records(employee_id)",
    )
    .execute(pool)
    .await?;

    tracing::info!("HR + attendance tables initialized");
    Ok(())
}

/// Insert a new employee.
pub async fn insert_employee(pool: &SqlitePool, employee: &Employee) -> anyhow::Result<()> {
    // P2: Validate employee ID is a non-empty UUID-format string.
    let id_str = employee.id.to_string();
    if id_str.is_empty() || id_str == "00000000-0000-0000-0000-000000000000" {
        anyhow::bail!("insert_employee: invalid employee id '{}'", id_str);
    }

    let role_str = serde_json::to_string(&employee.role)?.replace('"', "");
    let skills_str = serde_json::to_string(&employee.skills)?;

    sqlx::query(
        "INSERT INTO employees
            (id, name, role, skills, hourly_rate_paise, phone, is_active, face_enrollment_id, hired_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
    )
    .bind(employee.id.to_string())
    .bind(&employee.name)
    .bind(&role_str)
    .bind(&skills_str)
    .bind(employee.hourly_rate_paise)
    .bind(&employee.phone)
    .bind(if employee.is_active { 1i64 } else { 0i64 })
    .bind(&employee.face_enrollment_id)
    .bind(employee.hired_at.format("%Y-%m-%d").to_string())
    .execute(pool)
    .await?;

    Ok(())
}

/// List employees, optionally filtering to active-only.
pub async fn list_employees(
    pool: &SqlitePool,
    active_only: bool,
) -> anyhow::Result<Vec<Employee>> {
    let rows = if active_only {
        sqlx::query_as::<_, EmployeeRow>(
            "SELECT id, name, role, skills, hourly_rate_paise, phone, is_active, face_enrollment_id, hired_at
             FROM employees WHERE is_active = 1 ORDER BY name ASC",
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, EmployeeRow>(
            "SELECT id, name, role, skills, hourly_rate_paise, phone, is_active, face_enrollment_id, hired_at
             FROM employees ORDER BY name ASC",
        )
        .fetch_all(pool)
        .await?
    };

    rows.into_iter().map(row_to_employee).collect()
}

/// Get a single employee by ID.
pub async fn get_employee(
    pool: &SqlitePool,
    id: &str,
) -> anyhow::Result<Option<Employee>> {
    let row = sqlx::query_as::<_, EmployeeRow>(
        "SELECT id, name, role, skills, hourly_rate_paise, phone, is_active, face_enrollment_id, hired_at
         FROM employees WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(Some(row_to_employee(r)?)),
        None => Ok(None),
    }
}

/// Update an employee's fields. Only non-None fields are changed.
/// P1-5: Uses explicit column-by-column parameterized queries — no dynamic SQL construction.
pub async fn update_employee(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
    role: Option<&StaffRole>,
    skills: Option<&[String]>,
    hourly_rate_paise: Option<i64>,
    phone: Option<&str>,
    is_active: Option<bool>,
    face_enrollment_id: Option<&str>,
) -> anyhow::Result<bool> {
    // MMA-R1: Wrap all updates in a transaction to prevent partial updates
    let mut tx = pool.begin().await?;
    let mut any_updated = false;

    if let Some(n) = name {
        let r = sqlx::query("UPDATE employees SET name = ?1 WHERE id = ?2")
            .bind(n).bind(id).execute(&mut *tx).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }
    if let Some(r) = role {
        let role_str = serde_json::to_string(r)?.replace('"', "");
        let r = sqlx::query("UPDATE employees SET role = ?1 WHERE id = ?2")
            .bind(&role_str).bind(id).execute(&mut *tx).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }
    if let Some(s) = skills {
        let skills_str = serde_json::to_string(s)?;
        let r = sqlx::query("UPDATE employees SET skills = ?1 WHERE id = ?2")
            .bind(&skills_str).bind(id).execute(&mut *tx).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }
    if let Some(rate) = hourly_rate_paise {
        let r = sqlx::query("UPDATE employees SET hourly_rate_paise = ?1 WHERE id = ?2")
            .bind(rate).bind(id).execute(&mut *tx).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }
    if let Some(p) = phone {
        let r = sqlx::query("UPDATE employees SET phone = ?1 WHERE id = ?2")
            .bind(p).bind(id).execute(&mut *tx).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }
    if let Some(a) = is_active {
        let r = sqlx::query("UPDATE employees SET is_active = ?1 WHERE id = ?2")
            .bind(if a { 1i64 } else { 0i64 }).bind(id).execute(&mut *tx).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }
    if let Some(f) = face_enrollment_id {
        let r = sqlx::query("UPDATE employees SET face_enrollment_id = ?1 WHERE id = ?2")
            .bind(f).bind(id).execute(&mut *tx).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }

    tx.commit().await?;
    Ok(any_updated)
}

#[derive(sqlx::FromRow)]
struct EmployeeRow {
    id: String,
    name: String,
    role: String,
    skills: String,
    hourly_rate_paise: i64,
    phone: String,
    is_active: i64,
    face_enrollment_id: Option<String>,
    hired_at: String,
}

fn row_to_employee(row: EmployeeRow) -> anyhow::Result<Employee> {
    let role_json = format!("\"{}\"", row.role);
    let role: StaffRole = serde_json::from_str(&role_json)?;
    let skills: Vec<String> = serde_json::from_str(&row.skills).unwrap_or_default();
    let hired_at = chrono::NaiveDate::parse_from_str(&row.hired_at, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());

    Ok(Employee {
        id: Uuid::parse_str(&row.id)?,
        name: row.name,
        role,
        skills,
        hourly_rate_paise: row.hourly_rate_paise,
        phone: row.phone,
        is_active: row.is_active != 0,
        face_enrollment_id: row.face_enrollment_id,
        hired_at,
    })
}

// ===========================================================================
// Phase 14 (v29.0): Attendance tracking
// ===========================================================================

/// Record a clock-in/clock-out attendance entry.
pub async fn record_attendance(
    pool: &SqlitePool,
    employee_id: &str,
    date: &str,
    clock_in: Option<&str>,
    clock_out: Option<&str>,
    source: &str,
) -> anyhow::Result<()> {
    // P2: Validate employee_id is a non-empty, UUID-like string.
    if employee_id.trim().is_empty() {
        anyhow::bail!("record_attendance: employee_id must not be empty");
    }
    // Validate date is non-empty (basic guard — full format validation happens at the API layer).
    if date.trim().is_empty() {
        anyhow::bail!("record_attendance: date must not be empty");
    }

    let id = Uuid::new_v4().to_string();

    // P2-1 + P2-4: Compute hours_worked in whole minutes (integer) to avoid f64 drift.
    // Handle overnight shifts where clock_out < clock_in by adding 24h.
    let hours = match (clock_in, clock_out) {
        (Some(ci), Some(co)) => {
            let t_in = chrono::NaiveTime::parse_from_str(ci, "%H:%M");
            let t_out = chrono::NaiveTime::parse_from_str(co, "%H:%M");
            match (t_in, t_out) {
                (Ok(i), Ok(o)) => {
                    let mut total_minutes = (o - i).num_minutes();
                    // P2-1: overnight shift — clock_out before clock_in means next day
                    if total_minutes < 0 {
                        total_minutes += 24 * 60;
                    }
                    // P2-4: integer minutes → hours only for storage (avoids f64 accumulation)
                    total_minutes as f64 / 60.0
                }
                (Err(e), _) => {
                    tracing::warn!(
                        "record_attendance: clock_in parse failed for employee '{}' on '{}': '{}' — {}. Defaulting hours_worked to 0.",
                        employee_id, date, ci, e
                    );
                    0.0
                }
                (_, Err(e)) => {
                    tracing::warn!(
                        "record_attendance: clock_out parse failed for employee '{}' on '{}': '{}' — {}. Defaulting hours_worked to 0.",
                        employee_id, date, co, e
                    );
                    0.0
                }
            }
        }
        _ => 0.0,
    };

    sqlx::query(
        "INSERT INTO attendance_records
            (id, employee_id, date, clock_in, clock_out, source, hours_worked)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
    )
    .bind(&id)
    .bind(employee_id)
    .bind(date)
    .bind(clock_in)
    .bind(clock_out)
    .bind(source)
    .bind(hours)
    .execute(pool)
    .await?;

    Ok(())
}

/// Query attendance records by date and/or employee.
pub async fn query_attendance(
    pool: &SqlitePool,
    date: Option<&str>,
    employee_id: Option<&str>,
) -> anyhow::Result<Vec<AttendanceRecord>> {
    let rows = match (date, employee_id) {
        // P2-2: All query branches have LIMIT to prevent unbounded result sets
        (Some(d), Some(eid)) => {
            sqlx::query_as::<_, AttendanceRow>(
                "SELECT id, employee_id, date, clock_in, clock_out, source, hours_worked
                 FROM attendance_records WHERE date = ?1 AND employee_id = ?2
                 ORDER BY clock_in ASC
                 LIMIT 1000",
            )
            .bind(d)
            .bind(eid)
            .fetch_all(pool)
            .await?
        }
        (Some(d), None) => {
            sqlx::query_as::<_, AttendanceRow>(
                "SELECT id, employee_id, date, clock_in, clock_out, source, hours_worked
                 FROM attendance_records WHERE date = ?1
                 ORDER BY clock_in ASC
                 LIMIT 1000",
            )
            .bind(d)
            .fetch_all(pool)
            .await?
        }
        (None, Some(eid)) => {
            sqlx::query_as::<_, AttendanceRow>(
                "SELECT id, employee_id, date, clock_in, clock_out, source, hours_worked
                 FROM attendance_records WHERE employee_id = ?1
                 ORDER BY date DESC, clock_in ASC
                 LIMIT 1000",
            )
            .bind(eid)
            .fetch_all(pool)
            .await?
        }
        (None, None) => {
            sqlx::query_as::<_, AttendanceRow>(
                "SELECT id, employee_id, date, clock_in, clock_out, source, hours_worked
                 FROM attendance_records
                 ORDER BY date DESC, clock_in ASC
                 LIMIT 500",
            )
            .fetch_all(pool)
            .await?
        }
    };

    rows.into_iter().map(row_to_attendance).collect()
}

#[derive(sqlx::FromRow)]
struct AttendanceRow {
    id: String,
    employee_id: String,
    date: String,
    clock_in: Option<String>,
    clock_out: Option<String>,
    source: String,
    hours_worked: f64,
}

fn row_to_attendance(row: AttendanceRow) -> anyhow::Result<AttendanceRecord> {
    let date = chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
    Ok(AttendanceRecord {
        id: Uuid::parse_str(&row.id)?,
        employee_id: Uuid::parse_str(&row.employee_id)?,
        date,
        clock_in: row.clock_in,
        clock_out: row.clock_out,
        source: row.source,
        // P2: Clamp negative DB values — guard against corrupt/legacy rows.
        hours_worked: row.hours_worked.max(0.0),
    })
}

// ===========================================================================
// Phase 17 (v29.0): Payroll & labor cost
// ===========================================================================

/// Calculate monthly payroll by joining employees and attendance_records.
pub async fn calculate_monthly_payroll(
    pool: &SqlitePool,
    year: i32,
    month: u32,
) -> anyhow::Result<PayrollSummary> {
    let start_date = format!("{:04}-{:02}-01", year, month);
    // P1: Use exclusive upper bound (first day of next month) to avoid including
    // entries from the next month when the date field is a string-compared YYYY-MM-DD.
    // "YYYY-MM-31" would include next-month entries for months with < 31 days
    // because string comparison: "2024-03-01" > "2024-02-31" (doesn't exist but sorts after Feb).
    // Using < next_month_start is always correct regardless of days in month.
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1u32)
    } else {
        (year, month + 1)
    };
    let next_month_start = format!("{:04}-{:02}-01", next_year, next_month);

    let rows = sqlx::query_as::<_, PayrollRow>(
        "SELECT e.id AS employee_id, e.name, e.hourly_rate_paise,
                COALESCE(SUM(a.hours_worked), 0) AS total_hours
         FROM employees e
         LEFT JOIN attendance_records a
             ON a.employee_id = e.id
             AND a.date >= ?1 AND a.date < ?2
         WHERE e.is_active = 1
         GROUP BY e.id
         ORDER BY e.name ASC",
    )
    .bind(&start_date)
    .bind(&next_month_start)
    .fetch_all(pool)
    .await?;

    let mut total_hours = 0.0f64;
    let mut total_paise = 0i64;
    let mut by_employee = Vec::with_capacity(rows.len());

    for row in rows {
        // P1: Compute wages in integer paise to avoid f64 accumulation.
        // hours_worked is stored as f64 (minutes/60); convert back to whole minutes
        // then do integer-only multiplication: minutes * rate_paise / 60.
        let worked_minutes = (row.total_hours * 60.0).round() as i64;
        let emp_total = worked_minutes.max(0) * row.hourly_rate_paise / 60;
        total_hours += row.total_hours;
        total_paise += emp_total;
        by_employee.push(EmployeePayroll {
            employee_id: row.employee_id,
            name: row.name,
            hours_worked: row.total_hours,
            rate_paise: row.hourly_rate_paise,
            total_paise: emp_total,
        });
    }

    Ok(PayrollSummary {
        year,
        month,
        total_hours,
        total_paise,
        by_employee,
    })
}

#[derive(sqlx::FromRow)]
struct PayrollRow {
    employee_id: String,
    name: String,
    hourly_rate_paise: i64,
    total_hours: f64,
}
