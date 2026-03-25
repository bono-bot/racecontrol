/// Canonical pod ID format: `pod_N` (lowercase, underscore separator).
/// Accepts: pod-1, pod_1, POD_1, Pod-1, POD-1, etc.
/// Returns Err for empty, missing number, or unrecognized format.
pub fn normalize_pod_id(raw: &str) -> Result<String, String> {
    let s = raw.trim().to_lowercase();
    if s.is_empty() {
        return Err("empty pod ID".to_string());
    }
    // Strip "pod" prefix, then separator (- or _), then parse number
    let rest = s
        .strip_prefix("pod")
        .ok_or_else(|| format!("invalid pod ID format: '{}'", raw))?;
    let rest = rest
        .strip_prefix('-')
        .or_else(|| rest.strip_prefix('_'))
        .ok_or_else(|| format!("invalid pod ID format: '{}'", raw))?;
    let num: u32 = rest
        .parse()
        .map_err(|_| format!("invalid pod number in '{}': '{}'", raw, rest))?;
    Ok(format!("pod_{}", num))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hyphen_lowercase() {
        assert_eq!(normalize_pod_id("pod-1"), Ok("pod_1".to_string()));
    }

    #[test]
    fn test_underscore_lowercase() {
        assert_eq!(normalize_pod_id("pod_1"), Ok("pod_1".to_string()));
    }

    #[test]
    fn test_uppercase_underscore() {
        assert_eq!(normalize_pod_id("POD_1"), Ok("pod_1".to_string()));
    }

    #[test]
    fn test_mixed_case_hyphen() {
        assert_eq!(normalize_pod_id("Pod-1"), Ok("pod_1".to_string()));
    }

    #[test]
    fn test_multi_digit() {
        assert_eq!(normalize_pod_id("pod-99"), Ok("pod_99".to_string()));
    }

    #[test]
    fn test_uppercase_hyphen() {
        assert_eq!(normalize_pod_id("POD-8"), Ok("pod_8".to_string()));
    }

    #[test]
    fn test_empty_string_err() {
        assert!(normalize_pod_id("").is_err());
    }

    #[test]
    fn test_garbage_err() {
        assert!(normalize_pod_id("garbage").is_err());
    }

    #[test]
    fn test_zero_edge_case() {
        assert_eq!(normalize_pod_id("pod_0"), Ok("pod_0".to_string()));
    }

    #[test]
    fn test_no_number_after_prefix_err() {
        assert!(normalize_pod_id("pod-").is_err());
    }
}
