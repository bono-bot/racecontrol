/// Server-side input validation for launch_args and FFB safety cap.
///
/// SEC-01: Block INI injection by rejecting launch_args fields with special chars.
/// SEC-02: Cap FFB GAIN at 100 (physical safety — motors rated for 0-100%).
///
/// These checks run at the server boundary, BEFORE the WS message reaches the agent.
/// The agent also has its own validation (defence-in-depth), but server-side is the
/// primary gate because it returns a 400 to the caller immediately.

/// Validate all launch_args fields against the safe content allowlist.
///
/// Checks: "car", "track", "skin", "track_config" string fields, and "ai_cars" entries.
/// Allowlist: `^[a-zA-Z0-9._-]{0,128}$` — same as agent's validate_content_id.
/// Rejects any field containing `\n`, `=`, `[`, `]`, `;`, `#`, or chars outside allowlist.
///
/// Returns `Ok(())` if all fields pass, `Err(String)` with a descriptive message on rejection.
pub fn validate_launch_args(args_json: &serde_json::Value) -> Result<(), String> {
    // Fields that map directly to INI values — any special char could be an injection
    let direct_fields = ["car", "track", "skin", "track_config"];

    for field in &direct_fields {
        if let Some(val) = args_json.get(field).and_then(|v| v.as_str()) {
            validate_content_field(val, field)?;
        }
    }

    // Validate ai_cars array entries
    if let Some(ai_cars) = args_json.get("ai_cars").and_then(|v| v.as_array()) {
        for (i, entry) in ai_cars.iter().enumerate() {
            for sub_field in &["model", "skin"] {
                if let Some(val) = entry.get(sub_field).and_then(|v| v.as_str()) {
                    let label = format!("ai_cars[{}].{}", i, sub_field);
                    validate_content_field(val, &label)?;
                }
            }
        }
    }

    Ok(())
}

/// Validate a single content field value against the safe allowlist.
///
/// Allowlist: `^[a-zA-Z0-9._-]{0,128}$`
/// Also explicitly rejects INI-injection chars: `\n`, `=`, `[`, `]`, `;`, `#`
fn validate_content_field(value: &str, field: &str) -> Result<(), String> {
    if value.len() > 128 {
        tracing::warn!(field = %field, len = value.len(), "launch_args field rejected: too long");
        return Err(format!(
            "launch_args field '{}' exceeds 128 character limit",
            field
        ));
    }

    // Check for INI-injection special characters
    let injection_chars = ['\n', '\r', '=', '[', ']', ';', '#'];
    for c in injection_chars {
        if value.contains(c) {
            tracing::warn!(
                field = %field,
                value = %value,
                char = %c,
                "launch_args field rejected: INI injection char"
            );
            return Err(format!(
                "launch_args field '{}' contains disallowed character '{}'",
                field, c
            ));
        }
    }

    // Allowlist: alphanumeric + period + underscore + hyphen only
    for c in value.chars() {
        if !c.is_ascii_alphanumeric() && c != '.' && c != '_' && c != '-' {
            tracing::warn!(
                field = %field,
                value = %value,
                char = %c,
                "launch_args field rejected: char outside allowlist"
            );
            return Err(format!(
                "launch_args field '{}' contains disallowed character '{}'",
                field, c
            ));
        }
    }

    // Check for path traversal
    if value.contains("..") {
        tracing::warn!(field = %field, value = %value, "launch_args field rejected: path traversal");
        return Err(format!(
            "launch_args field '{}' contains path traversal sequence '..'",
            field
        ));
    }

    Ok(())
}

/// Sanitize an FFB GAIN value for physical safety (SEC-02).
///
/// Rules:
/// - Known presets ("light", "medium", "strong"): pass through unchanged
/// - Valid numeric 0-100: pass through unchanged
/// - Numeric > 100: cap to "100" and log WARN
/// - Invalid (negative, non-numeric non-preset): return "medium" (safe default) and log WARN
pub fn sanitize_ffb_gain(ffb_value: &str) -> String {
    // Known presets pass through unchanged
    match ffb_value {
        "light" | "medium" | "strong" => return ffb_value.to_string(),
        _ => {}
    }

    // Try to parse as integer
    match ffb_value.trim().parse::<i64>() {
        Ok(n) if n > 100 => {
            tracing::warn!(
                ffb_value = %ffb_value,
                capped_to = 100,
                "FFB GAIN above 100 capped to 100 (physical safety)"
            );
            "100".to_string()
        }
        Ok(n) if n < 0 => {
            tracing::warn!(
                ffb_value = %ffb_value,
                "FFB GAIN negative — returning safe default 'medium'"
            );
            "medium".to_string()
        }
        Ok(_) => ffb_value.trim().to_string(), // 0-100 numeric: pass through
        Err(_) => {
            tracing::warn!(
                ffb_value = %ffb_value,
                "FFB GAIN invalid value — returning safe default 'medium'"
            );
            "medium".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ─── validate_launch_args tests ───────────────────────────────────────

    #[test]
    fn rejects_car_with_newline() {
        let args = json!({ "car": "ks_porsche_911\nGAIN=0" });
        let result = validate_launch_args(&args);
        assert!(result.is_err(), "Expected Err for newline in car field");
        assert!(result.unwrap_err().contains("car"));
    }

    #[test]
    fn rejects_car_with_equals() {
        let args = json!({ "car": "ks_porsche=911" });
        let result = validate_launch_args(&args);
        assert!(result.is_err(), "Expected Err for '=' in car field");
    }

    #[test]
    fn rejects_car_with_bracket_open() {
        let args = json!({ "car": "ks_porsche[911" });
        let result = validate_launch_args(&args);
        assert!(result.is_err(), "Expected Err for '[' in car field");
    }

    #[test]
    fn rejects_car_with_bracket_close() {
        let args = json!({ "car": "ks_porsche]911" });
        let result = validate_launch_args(&args);
        assert!(result.is_err(), "Expected Err for ']' in car field");
    }

    #[test]
    fn accepts_valid_car_name() {
        let args = json!({ "car": "ks_porsche_911_gt3_r" });
        let result = validate_launch_args(&args);
        assert!(result.is_ok(), "Expected Ok for valid car name");
    }

    #[test]
    fn accepts_valid_track_and_config() {
        let args = json!({ "track": "imola", "track_config": "gp" });
        let result = validate_launch_args(&args);
        assert!(result.is_ok(), "Expected Ok for valid track/config");
    }

    #[test]
    fn rejects_skin_with_path_traversal() {
        let args = json!({ "skin": "../../evil_skin" });
        let result = validate_launch_args(&args);
        assert!(result.is_err(), "Expected Err for path traversal in skin");
    }

    #[test]
    fn accepts_empty_args() {
        // No relevant fields present — should pass
        let args = json!({ "duration_minutes": 30 });
        let result = validate_launch_args(&args);
        assert!(result.is_ok(), "Expected Ok for args with no content fields");
    }

    #[test]
    fn rejects_ai_cars_model_with_injection() {
        let args = json!({
            "car": "ks_ferrari",
            "ai_cars": [{ "model": "evil[CAR]\nGAIN=0", "skin": "default" }]
        });
        let result = validate_launch_args(&args);
        assert!(result.is_err(), "Expected Err for injection in ai_cars model");
    }

    // ─── sanitize_ffb_gain tests ──────────────────────────────────────────

    #[test]
    fn ffb_caps_150_to_100() {
        assert_eq!(sanitize_ffb_gain("150"), "100");
    }

    #[test]
    fn ffb_preset_strong_passes_through() {
        assert_eq!(sanitize_ffb_gain("strong"), "strong");
    }

    #[test]
    fn ffb_preset_light_passes_through() {
        assert_eq!(sanitize_ffb_gain("light"), "light");
    }

    #[test]
    fn ffb_preset_medium_passes_through() {
        assert_eq!(sanitize_ffb_gain("medium"), "medium");
    }

    #[test]
    fn ffb_numeric_70_passes_through() {
        assert_eq!(sanitize_ffb_gain("70"), "70");
    }

    #[test]
    fn ffb_negative_returns_default() {
        assert_eq!(sanitize_ffb_gain("-5"), "medium");
    }

    #[test]
    fn ffb_invalid_string_returns_default() {
        assert_eq!(sanitize_ffb_gain("notanumber"), "medium");
    }

    #[test]
    fn ffb_zero_passes_through() {
        assert_eq!(sanitize_ffb_gain("0"), "0");
    }

    #[test]
    fn ffb_exactly_100_passes_through() {
        assert_eq!(sanitize_ffb_gain("100"), "100");
    }
}
