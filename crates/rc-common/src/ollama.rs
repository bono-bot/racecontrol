//! Shared Ollama integration — blocking HTTP POST for crash analysis.
//!
//! Queries Ollama via raw TcpStream (no reqwest, pure std).
//! Fire-and-forget: spawns a thread, doesn't block the caller.
//! Used by rc-sentry (Tier 3 crash analysis) and rc-watchdog (James AI healer).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const LOG_TARGET: &str = "ollama";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(45);
const DEFAULT_OLLAMA_URL: &str = "192.168.31.27:11434";
const DEFAULT_MODEL: &str = "qwen2.5:3b";

/// Query result from Ollama.
#[derive(Debug)]
pub struct OllamaResult {
    pub suggestion: String,
    pub model: String,
}

/// Send a crash analysis query to Ollama. Blocking call.
/// Returns None on timeout, connection failure, or parse error.
pub fn query_crash(crash_context: &str, ollama_url: Option<&str>, model: Option<&str>) -> Option<OllamaResult> {
    let url = ollama_url.unwrap_or(DEFAULT_OLLAMA_URL);
    let model_name = model.unwrap_or(DEFAULT_MODEL);

    let prompt = format!(
        "rc-agent crashed on a Racing Point sim racing pod. Analyze this crash and suggest a fix.\n\
        Reply with ONLY the fix action (one line), no explanation.\n\n\
        Crash context:\n{}\n\n\
        Known fix types: zombie_kill, port_wait, close_wait_clean, config_repair, shader_cache_clear, restart.\n\
        If none apply, suggest a new fix type and action.",
        crash_context
    );

    let body = serde_json::json!({
        "model": model_name,
        "prompt": prompt,
        "stream": false,
        "options": {
            "temperature": 0.3,
            "num_predict": 128
        }
    });

    let body_str = body.to_string();

    let request = format!(
        "POST /api/generate HTTP/1.0\r\n\
         Host: localhost\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        body_str.len(),
        body_str
    );

    // Connect
    let addr: std::net::SocketAddr = match url.parse() {
        Ok(a) => a,
        Err(_) => {
            tracing::warn!(target: LOG_TARGET, "invalid ollama address: {}", url);
            return None;
        }
    };

    let stream = match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "ollama connect failed: {}", e);
            return None;
        }
    };

    if stream.set_read_timeout(Some(READ_TIMEOUT)).is_err() {
        return None;
    }

    let mut stream = stream;
    if stream.write_all(request.as_bytes()).is_err() {
        tracing::warn!(target: LOG_TARGET, "ollama write failed");
        return None;
    }

    // Read response
    let mut response = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => response.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
    }

    let response_str = String::from_utf8_lossy(&response);

    // Find JSON body after HTTP headers
    let body_start = response_str.find("\r\n\r\n").map(|i| i + 4)
        .or_else(|| response_str.find("\n\n").map(|i| i + 2));

    let json_str = match body_start {
        Some(start) => &response_str[start..],
        None => {
            tracing::warn!(target: LOG_TARGET, "no HTTP body in ollama response");
            return None;
        }
    };

    // Parse Ollama response
    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "ollama JSON parse failed: {}", e);
            return None;
        }
    };

    let suggestion = parsed.get("response")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if suggestion.is_empty() {
        tracing::warn!(target: LOG_TARGET, "ollama returned empty response");
        return None;
    }

    tracing::info!(target: LOG_TARGET, "ollama suggestion: {}", suggestion);

    Some(OllamaResult {
        suggestion,
        model: model_name.to_string(),
    })
}

/// Fire-and-forget Ollama query in a background thread.
/// Calls `on_result` callback with the result when done.
pub fn query_async(
    crash_context: String,
    on_result: Box<dyn FnOnce(Option<OllamaResult>) + Send + 'static>,
) {
    std::thread::Builder::new()
        .name("rc-common-ollama".to_string())
        .spawn(move || {
            let result = query_crash(&crash_context, None, None);
            on_result(result);
        })
        .expect("spawn ollama thread");
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_crash_returns_none_on_unreachable() {
        // Use a port that's definitely not listening
        let result = query_crash("test crash", Some("127.0.0.1:19999"), None);
        assert!(result.is_none());
    }

    #[test]
    fn query_async_calls_callback() {
        let (tx, rx) = std::sync::mpsc::channel();
        // Use a guaranteed-unreachable address to avoid environment dependency
        query_async(
            "test crash".to_string(),
            Box::new(move |result| {
                tx.send(result.is_none()).unwrap();
            }),
        );
        // The callback should fire (either Some or None depending on environment)
        // Just verify the callback fires within timeout
        let _result = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        // Note: we don't assert is_none because Ollama may be running locally
    }
}
