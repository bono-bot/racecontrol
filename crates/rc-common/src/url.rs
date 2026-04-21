//! URL helpers shared across rc-agent and racecontrol.
//!
//! Pattern I Part 5 (C7): this module exists because the same
//! `ws:// → http://` + `/ws...` strip pattern was inlined in 10+ call sites
//! across `rc-agent/src/{main.rs,ws_handler.rs}`. Divergence between call
//! sites is a deploy-time regression class (different base URLs for
//! different HTTP consumers on the same pod), so we pin the conversion
//! here with a tested contract.

/// Derive an HTTP(S) base URL from a WebSocket URL used for the racecontrol core.
///
/// Contract:
/// - `ws://host:port/ws/...` → `http://host:port`
/// - `wss://host:port/ws` → `https://host:port`
/// - `ws://host:port/ws?jwt=...` → `http://host:port` (query string discarded with path)
/// - `ws://host:port` (no path) → `http://host:port`
/// - Any input lacking `ws://` and `wss://` prefix is returned unchanged AFTER
///   `/ws` stripping (caller should not depend on this — it is a best-effort guard).
///
/// Returns a full base URL (scheme://host:port) WITHOUT a trailing slash and
/// WITHOUT `/api/v1` suffix — the caller appends the API path it needs.
///
/// This function is intentionally zero-dependency (no `url` crate) so it can
/// ship in `rc_common` without expanding the shared-crate dependency surface.
pub fn http_base_from_ws(ws_url: &str) -> String {
    let http_scheme = ws_url
        .replacen("wss://", "https://", 1)
        .replacen("ws://", "http://", 1);

    // Split at the first `/ws` occurrence AFTER the scheme's `//`.
    // Falls back to the scheme-converted URL when `/ws` is absent.
    let after_scheme_start = http_scheme.find("://").map_or(0, |i| i + 3);
    match http_scheme[after_scheme_start..].find("/ws") {
        Some(rel_idx) => http_scheme[..after_scheme_start + rel_idx].to_string(),
        None => {
            // Also strip any trailing `?query` or `#frag` when there's no `/ws` path.
            let tail_start = http_scheme[after_scheme_start..]
                .find(['?', '#'])
                .map(|i| after_scheme_start + i);
            match tail_start {
                Some(idx) => http_scheme[..idx].to_string(),
                None => http_scheme.trim_end_matches('/').to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::http_base_from_ws;

    #[test]
    fn ws_with_ws_path() {
        assert_eq!(http_base_from_ws("ws://192.168.31.23:8080/ws"), "http://192.168.31.23:8080");
    }

    #[test]
    fn wss_with_jwt_query() {
        assert_eq!(
            http_base_from_ws("wss://core.racingpoint.cloud/ws?jwt=eyJhbGciOi..."),
            "https://core.racingpoint.cloud",
        );
    }

    #[test]
    fn ws_bare_host_port() {
        assert_eq!(http_base_from_ws("ws://127.0.0.1:8080"), "http://127.0.0.1:8080");
    }

    #[test]
    fn wss_with_nested_ws_subpath() {
        assert_eq!(
            http_base_from_ws("wss://core.racingpoint.cloud/ws/pods/pod_7"),
            "https://core.racingpoint.cloud",
        );
    }

    #[test]
    fn ws_with_trailing_slash() {
        assert_eq!(http_base_from_ws("ws://192.168.31.23:8080/"), "http://192.168.31.23:8080");
    }

    #[test]
    fn ws_with_port_no_path() {
        assert_eq!(http_base_from_ws("ws://pod-7.lan:9090"), "http://pod-7.lan:9090");
    }

    #[test]
    fn ws_host_contains_ws_literal_in_subdomain() {
        // Guard: hostname like "ws-core" must not be mis-split. Scheme-relative find
        // starts after `://` so the host itself is not scanned for `/ws`.
        assert_eq!(
            http_base_from_ws("ws://ws-core.internal:8080/ws/agent"),
            "http://ws-core.internal:8080",
        );
    }

    #[test]
    fn already_http_passthrough() {
        // If caller somehow supplies an http URL, don't mangle it.
        assert_eq!(
            http_base_from_ws("http://192.168.31.23:8080/ws"),
            "http://192.168.31.23:8080",
        );
    }

    #[test]
    fn empty_string_does_not_panic() {
        // Best-effort guard — malformed input returns something safe.
        assert_eq!(http_base_from_ws(""), "");
    }

    #[test]
    fn unknown_scheme_unchanged() {
        // Non-ws URL is returned post-strip — caller should validate separately.
        assert_eq!(http_base_from_ws("tcp://host:1234"), "tcp://host:1234");
    }
}
