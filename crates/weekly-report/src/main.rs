use chrono::{Datelike, NaiveTime, Utc};
use chrono_tz::Asia::Kolkata;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("weekly-report error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = std::env::var("RACECONTROL_DB")
        .unwrap_or_else(|_| r"C:\RacingPoint\racecontrol.db".to_string());
    let email_script_path = std::env::var("EMAIL_SCRIPT")
        .unwrap_or_else(|_| r"C:\RacingPoint\send_email.js".to_string());
    let recipient = std::env::var("EMAIL_RECIPIENT")
        .unwrap_or_else(|_| "usingh@racingpoint.in".to_string());

    // Open SQLite read-only
    let db_url = format!("sqlite:{}?mode=ro", db_path);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await?;

    // Compute IST week boundaries (previous Monday 00:00 IST to previous Sunday 23:59:59 IST)
    let now_ist = Utc::now().with_timezone(&Kolkata);
    let today = now_ist.date_naive();
    let days_since_monday = today.weekday().num_days_from_monday() as i64;
    let this_monday = today - chrono::Duration::days(days_since_monday);
    let prev_monday = this_monday - chrono::Duration::days(7);
    let prev_sunday = this_monday - chrono::Duration::days(1);

    // Convert IST boundaries to UTC strings for SQL
    // Monday 00:00 IST = previous day 18:30 UTC
    let ist_offset = chrono::Duration::hours(5) + chrono::Duration::minutes(30);
    let week_start_utc = prev_monday
        .and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
        - ist_offset;
    let week_end_utc = prev_sunday
        .and_time(NaiveTime::from_hms_opt(23, 59, 59).unwrap())
        - ist_offset;

    let week_start_utc_str = week_start_utc.format("%Y-%m-%d %H:%M:%S").to_string();
    let week_end_utc_str = week_end_utc.format("%Y-%m-%d %H:%M:%S").to_string();

    println!(
        "Querying week: {} to {} (UTC)",
        week_start_utc_str, week_end_utc_str
    );

    // Query 1: Total sessions
    let total_sessions: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM billing_sessions
         WHERE started_at >= ? AND started_at < ?
         AND status IN ('completed', 'active', 'ended_early')",
    )
    .bind(&week_start_utc_str)
    .bind(&week_end_utc_str)
    .fetch_one(&pool)
    .await?;

    // Query 2: Total credits billed (paise)
    let total_paise: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(
            COALESCE(wallet_debit_paise, custom_price_paise, 0)
        ), 0) FROM billing_sessions
         WHERE started_at >= ? AND started_at < ?",
    )
    .bind(&week_start_utc_str)
    .bind(&week_end_utc_str)
    .fetch_one(&pool)
    .await?;

    let total_rupees = total_paise.0 as f64 / 100.0;

    // Query 3: Uptime % per pod
    let uptime_rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT pod_id, ROUND(AVG(ws_connected) * 100.0, 1) AS uptime_pct
         FROM pod_uptime_samples
         WHERE sampled_at >= ? AND sampled_at < ?
         GROUP BY pod_id ORDER BY pod_id",
    )
    .bind(&week_start_utc_str)
    .bind(&week_end_utc_str)
    .fetch_all(&pool)
    .await?;

    // Query 4: Incidents
    let incidents: Vec<(String, String, Option<String>, Option<i64>, Option<String>)> =
        sqlx::query_as(
            "SELECT alert_type, started_at, resolved_at, pod_count, description
             FROM alert_incidents
             WHERE started_at >= ? AND started_at < ?
             ORDER BY started_at",
        )
        .bind(&week_start_utc_str)
        .bind(&week_end_utc_str)
        .fetch_all(&pool)
        .await?;

    // Compute fleet average uptime
    let fleet_avg_uptime = if uptime_rows.is_empty() {
        0.0
    } else {
        uptime_rows.iter().map(|(_, pct)| pct).sum::<f64>() / uptime_rows.len() as f64
    };

    // Format dates for display
    let prev_monday_formatted = prev_monday.format("%d %b %Y").to_string();
    let prev_sunday_formatted = prev_sunday.format("%d %b %Y").to_string();
    let ist_now = now_ist.format("%d %b %Y %H:%M IST").to_string();

    // Build HTML email
    let html_body = build_html(
        &prev_monday_formatted,
        &prev_sunday_formatted,
        total_sessions.0,
        total_rupees,
        fleet_avg_uptime,
        &uptime_rows,
        &incidents,
        &ist_now,
    );

    // Send via send_email.js
    let subject = format!(
        "Racing Point Weekly Report - Week of {}",
        prev_monday_formatted
    );

    println!("Sending report to {} ...", recipient);

    let output = tokio::process::Command::new("node")
        .arg(&email_script_path)
        .arg(&recipient)
        .arg(&subject)
        .arg(&html_body)
        .kill_on_drop(true)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("send_email.js failed: {}", stderr);
        std::process::exit(1);
    }

    println!("Weekly report sent to {}", recipient);
    Ok(())
}

fn uptime_color(pct: f64) -> &'static str {
    if pct > 95.0 {
        "#4CAF50" // green
    } else if pct > 80.0 {
        "#FFC107" // yellow
    } else {
        "#E10600" // red (Racing Point brand)
    }
}

fn build_html(
    week_start: &str,
    week_end: &str,
    total_sessions: i64,
    total_rupees: f64,
    fleet_avg_uptime: f64,
    uptime_rows: &[(String, f64)],
    incidents: &[(String, String, Option<String>, Option<i64>, Option<String>)],
    generated_at: &str,
) -> String {
    // Pod uptime rows
    let mut uptime_html = String::new();
    for (pod_id, pct) in uptime_rows {
        let color = uptime_color(*pct);
        uptime_html.push_str(&format!(
            r#"<tr>
                <td style="padding:8px 12px;border-bottom:1px solid #333333;color:#ffffff;">{pod_id}</td>
                <td style="padding:8px 12px;border-bottom:1px solid #333333;color:{color};font-weight:bold;">{pct:.1}%</td>
            </tr>"#
        ));
    }
    if uptime_rows.is_empty() {
        uptime_html.push_str(
            r#"<tr><td colspan="2" style="padding:12px;color:#888888;text-align:center;">No uptime data this week</td></tr>"#,
        );
    }

    // Incident rows
    let mut incident_html = String::new();
    if incidents.is_empty() {
        incident_html.push_str(
            r#"<tr><td colspan="5" style="padding:12px;color:#4CAF50;text-align:center;">No incidents this week</td></tr>"#,
        );
    } else {
        for (i, (alert_type, started_at, resolved_at, _pod_count, description)) in
            incidents.iter().enumerate()
        {
            let duration_str = match resolved_at {
                Some(resolved) => {
                    if let (Ok(start), Ok(end)) = (
                        chrono::NaiveDateTime::parse_from_str(started_at, "%Y-%m-%d %H:%M:%S"),
                        chrono::NaiveDateTime::parse_from_str(resolved, "%Y-%m-%d %H:%M:%S"),
                    ) {
                        let mins = (end - start).num_minutes();
                        format!("{}m", mins)
                    } else {
                        "N/A".to_string()
                    }
                }
                None => "Ongoing".to_string(),
            };
            let desc = description.as_deref().unwrap_or("-");
            incident_html.push_str(&format!(
                r#"<tr>
                    <td style="padding:8px 12px;border-bottom:1px solid #333333;color:#ffffff;">{num}</td>
                    <td style="padding:8px 12px;border-bottom:1px solid #333333;color:#ffffff;">{alert_type}</td>
                    <td style="padding:8px 12px;border-bottom:1px solid #333333;color:#ffffff;">{started_at}</td>
                    <td style="padding:8px 12px;border-bottom:1px solid #333333;color:#ffffff;">{duration_str}</td>
                    <td style="padding:8px 12px;border-bottom:1px solid #333333;color:#ffffff;">{desc}</td>
                </tr>"#,
                num = i + 1,
            ));
        }
    }

    format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1.0"></head>
<body style="margin:0;padding:0;background-color:#1A1A1A;font-family:Arial,Helvetica,sans-serif;">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background-color:#1A1A1A;">
<tr><td align="center" style="padding:20px 10px;">
<table role="presentation" width="600" cellpadding="0" cellspacing="0" style="max-width:600px;width:100%;">

<!-- Header -->
<tr><td style="background-color:#E10600;padding:24px 20px;text-align:center;">
    <h1 style="margin:0;color:#ffffff;font-size:22px;font-weight:bold;">Racing Point Weekly Report</h1>
    <p style="margin:8px 0 0;color:#ffffff;font-size:14px;opacity:0.9;">Week of {week_start} - {week_end}</p>
</td></tr>

<!-- Summary Cards -->
<tr><td style="padding:20px 0;">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0">
<tr>
    <td width="33%" style="padding:0 6px;">
        <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background-color:#222222;border:1px solid #333333;border-radius:8px;">
        <tr><td style="padding:16px;text-align:center;">
            <p style="margin:0;color:#888888;font-size:11px;text-transform:uppercase;">Total Sessions</p>
            <p style="margin:8px 0 0;color:#ffffff;font-size:28px;font-weight:bold;">{total_sessions}</p>
        </td></tr>
        </table>
    </td>
    <td width="33%" style="padding:0 6px;">
        <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background-color:#222222;border:1px solid #333333;border-radius:8px;">
        <tr><td style="padding:16px;text-align:center;">
            <p style="margin:0;color:#888888;font-size:11px;text-transform:uppercase;">Total Revenue</p>
            <p style="margin:8px 0 0;color:#ffffff;font-size:28px;font-weight:bold;">Rs. {total_rupees:.0}</p>
        </td></tr>
        </table>
    </td>
    <td width="33%" style="padding:0 6px;">
        <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background-color:#222222;border:1px solid #333333;border-radius:8px;">
        <tr><td style="padding:16px;text-align:center;">
            <p style="margin:0;color:#888888;font-size:11px;text-transform:uppercase;">Fleet Avg Uptime</p>
            <p style="margin:8px 0 0;color:#ffffff;font-size:28px;font-weight:bold;">{fleet_avg_uptime:.1}%</p>
        </td></tr>
        </table>
    </td>
</tr>
</table>
</td></tr>

<!-- Pod Uptime Table -->
<tr><td style="background-color:#222222;border:1px solid #333333;border-radius:8px;overflow:hidden;">
    <table role="presentation" width="100%" cellpadding="0" cellspacing="0">
    <tr><td style="padding:14px 12px;border-bottom:2px solid #E10600;">
        <h2 style="margin:0;color:#ffffff;font-size:16px;">Pod Uptime</h2>
    </td></tr>
    <tr><td>
        <table role="presentation" width="100%" cellpadding="0" cellspacing="0">
        <tr style="background-color:#2a2a2a;">
            <th style="padding:8px 12px;text-align:left;color:#888888;font-size:12px;text-transform:uppercase;">Pod</th>
            <th style="padding:8px 12px;text-align:left;color:#888888;font-size:12px;text-transform:uppercase;">Uptime</th>
        </tr>
        {uptime_html}
        </table>
    </td></tr>
    </table>
</td></tr>

<tr><td style="height:16px;"></td></tr>

<!-- Incidents Table -->
<tr><td style="background-color:#222222;border:1px solid #333333;border-radius:8px;overflow:hidden;">
    <table role="presentation" width="100%" cellpadding="0" cellspacing="0">
    <tr><td style="padding:14px 12px;border-bottom:2px solid #E10600;">
        <h2 style="margin:0;color:#ffffff;font-size:16px;">Incidents</h2>
    </td></tr>
    <tr><td>
        <table role="presentation" width="100%" cellpadding="0" cellspacing="0">
        <tr style="background-color:#2a2a2a;">
            <th style="padding:8px 12px;text-align:left;color:#888888;font-size:12px;text-transform:uppercase;">#</th>
            <th style="padding:8px 12px;text-align:left;color:#888888;font-size:12px;text-transform:uppercase;">Type</th>
            <th style="padding:8px 12px;text-align:left;color:#888888;font-size:12px;text-transform:uppercase;">Started</th>
            <th style="padding:8px 12px;text-align:left;color:#888888;font-size:12px;text-transform:uppercase;">Duration</th>
            <th style="padding:8px 12px;text-align:left;color:#888888;font-size:12px;text-transform:uppercase;">Description</th>
        </tr>
        {incident_html}
        </table>
    </td></tr>
    </table>
</td></tr>

<!-- Footer -->
<tr><td style="padding:20px;text-align:center;">
    <p style="margin:0;color:#5A5A5A;font-size:12px;">Generated {generated_at} by James Vowles | Racing Point eSports</p>
</td></tr>

</table>
</td></tr>
</table>
</body>
</html>"#
    )
}
