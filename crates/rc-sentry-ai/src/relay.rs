use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct RelayStatus {
    pub status: String,
    pub api_url: String,
}

/// Check go2rtc relay health by hitting its /api/streams endpoint.
pub async fn check_relay_health(api_url: &str) -> RelayStatus {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return RelayStatus {
                status: "error".into(),
                api_url: api_url.into(),
            };
        }
    };

    match client
        .get(format!("{api_url}/api/streams"))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => RelayStatus {
            status: "healthy".into(),
            api_url: api_url.into(),
        },
        Ok(_) => RelayStatus {
            status: "error".into(),
            api_url: api_url.into(),
        },
        Err(_) => RelayStatus {
            status: "unreachable".into(),
            api_url: api_url.into(),
        },
    }
}
