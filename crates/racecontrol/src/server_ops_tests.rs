use super::*;
use axum::body::Body;
use http_body_util::BodyExt;
use tower::ServiceExt;

fn test_router() -> Router {
    Router::new()
        .route("/exec", post(exec_command))
        .route("/health", get(health))
        .route("/ping", get(ping))
}

async fn exec_post(app: Router, body: serde_json::Value) -> (u16, serde_json::Value) {
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/exec")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    (status, json)
}

#[tokio::test]
async fn test_exec_echo() {
    let app = test_router();
    let (status, json) = exec_post(app, serde_json::json!({
        "cmd": "echo hello",
        "timeout_ms": 10000
    })).await;
    assert_eq!(status, 200);
    assert_eq!(json["success"], true);
    assert!(json["stdout"].as_str().unwrap_or("").contains("hello"));
}

#[tokio::test]
async fn test_exec_timeout() {
    let app = test_router();
    let (status, json) = exec_post(app, serde_json::json!({
        "cmd": "echo timeout_test",
        "timeout_ms": 1
    })).await;
    assert_eq!(status, 500);
    assert_eq!(json["success"], false);
}

#[tokio::test]
async fn test_health() {
    START_TIME.get_or_init(Instant::now);
    let app = test_router();
    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["service"], "racecontrol");
}

#[tokio::test]
async fn test_base64_decoder() {
    use std::io::Read;
    let mut reader = base64_decode_reader(b"SGVsbG8gV29ybGQ=");
    let mut result = String::new();
    reader.read_to_string(&mut result).unwrap();
    assert_eq!(result, "Hello World");
}
