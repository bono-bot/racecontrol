use std::sync::Mutex;

use crate::config::NvrConfig;
use md5::{Digest, Md5};
use reqwest::Client;
use serde::Serialize;

/// Cached digest auth credentials to avoid 401 roundtrip on every request.
struct CachedAuth {
    realm: String,
    nonce: String,
    qop: String,
    nc: u32,
}

pub struct NvrClient {
    client: Client,
    base_url: String,
    username: String,
    password: String,
    cached_auth: Mutex<Option<CachedAuth>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NvrFileInfo {
    pub channel: u32,
    pub start_time: String,
    pub end_time: String,
    pub file_path: String,
    pub file_size: u64,
    pub file_type: String,
}

/// Parsed components from a WWW-Authenticate: Digest header
struct DigestChallenge {
    realm: String,
    nonce: String,
    qop: String,
}

impl NvrClient {
    pub fn new(config: &NvrConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest client");

        let base_url = format!("http://{}:{}", config.host, config.port);

        Self {
            client,
            base_url,
            username: config.username.clone(),
            password: config.password.clone(),
            cached_auth: Mutex::new(None),
        }
    }

    /// Search for recorded files on the NVR for a given channel and time range.
    ///
    /// `start` and `end` are in "YYYY-MM-DD HH:MM:SS" format.
    pub async fn search_files(
        &self,
        channel: u32,
        start: &str,
        end: &str,
    ) -> anyhow::Result<Vec<NvrFileInfo>> {
        // Step 1: Create search handle (factory)
        let factory_url = format!(
            "{}/cgi-bin/mediaFileFind.cgi?action=factory&object=",
            self.base_url
        );
        let factory_resp = self.digest_request("GET", &factory_url).await?;
        let handle = parse_factory_result(&factory_resp)?;

        // Step 2: Start the search with conditions
        let encoded_start = start.replace(' ', "%20");
        let encoded_end = end.replace(' ', "%20");
        let find_url = format!(
            "{}/cgi-bin/mediaFileFind.cgi?action=findFile&object={}\
            &condition.Channel={}&condition.StartTime={}&condition.EndTime={}\
            &condition.Types[0]=dav&condition.Flags[0]=Event",
            self.base_url, handle, channel, encoded_start, encoded_end
        );
        let _find_resp = self.digest_request("GET", &find_url).await?;

        // Step 3: Get results
        let next_url = format!(
            "{}/cgi-bin/mediaFileFind.cgi?action=findNextFile&object={}&count=100",
            self.base_url, handle
        );
        let next_resp = self.digest_request("GET", &next_url).await?;
        let files = parse_file_list(&next_resp);

        // Step 4: Close the search handle
        let close_url = format!(
            "{}/cgi-bin/mediaFileFind.cgi?action=close&object={}",
            self.base_url, handle
        );
        let _ = self.digest_request("GET", &close_url).await;

        Ok(files)
    }

    /// Stream a recorded file from the NVR. Returns the raw response for byte streaming.
    ///
    /// `file_path` is the full path returned by `search_files` (e.g. `/mnt/dvr/.../*.dav`).
    pub async fn stream_file(&self, file_path: &str) -> anyhow::Result<reqwest::Response> {
        let url = format!(
            "{}/cgi-bin/RPC_Loadfile{}",
            self.base_url, file_path
        );
        let response = self.digest_request_raw("GET", &url).await?;
        Ok(response)
    }

    /// Fetch a JPEG snapshot for the given NVR channel (1-indexed).
    pub async fn snapshot(&self, channel: u32) -> anyhow::Result<bytes::Bytes> {
        let url = format!(
            "{}/cgi-bin/snapshot.cgi?channel={}",
            self.base_url, channel
        );
        let resp = self.digest_request_raw("GET", &url).await?;
        let bytes = resp.bytes().await?;
        Ok(bytes)
    }

    /// Perform an HTTP request with Digest authentication, returning the response body as text.
    async fn digest_request(&self, method: &str, url: &str) -> anyhow::Result<String> {
        let resp = self.digest_request_raw(method, url).await?;
        let text = resp.text().await?;
        Ok(text)
    }

    /// Build an Authorization header from cached nonce (if available), incrementing nc.
    fn build_cached_auth_header(&self, method: &str, uri_path: &str) -> Option<String> {
        let mut guard = self.cached_auth.lock().ok()?;
        let cached = guard.as_mut()?;
        cached.nc += 1;
        let nc = format!("{:08x}", cached.nc);
        let cnonce = format!("{:08x}", rand_cnonce());
        Some(compute_digest_header(
            &self.username,
            &self.password,
            &cached.realm,
            &cached.nonce,
            &cached.qop,
            method,
            uri_path,
            &nc,
            &cnonce,
        ))
    }

    /// Save a successful digest challenge for future requests.
    fn cache_challenge(&self, challenge: &DigestChallenge) {
        if let Ok(mut guard) = self.cached_auth.lock() {
            *guard = Some(CachedAuth {
                realm: challenge.realm.clone(),
                nonce: challenge.nonce.clone(),
                qop: challenge.qop.clone(),
                nc: 0,
            });
        }
    }

    /// Parse the URI path (with query) from a full URL string.
    fn parse_uri_path(url: &str) -> anyhow::Result<String> {
        let uri = url::Url::parse(url)?;
        Ok(if let Some(q) = uri.query() {
            format!("{}?{}", uri.path(), q)
        } else {
            uri.path().to_string()
        })
    }

    /// Perform an HTTP request with Digest authentication, returning the raw response.
    ///
    /// Uses cached nonce when available to skip the initial 401 roundtrip.
    /// Falls back to full challenge-response if the cached nonce is stale.
    async fn digest_request_raw(
        &self,
        method: &str,
        url: &str,
    ) -> anyhow::Result<reqwest::Response> {
        let uri_path = Self::parse_uri_path(url)?;

        // Try cached auth first (single roundtrip)
        if let Some(auth_header) = self.build_cached_auth_header(method, &uri_path) {
            let req = match method {
                "POST" => self.client.post(url),
                _ => self.client.get(url),
            };
            let resp = req.header("Authorization", auth_header).send().await?;

            if resp.status().is_success() {
                return Ok(resp);
            }
            // Nonce expired or stale — fall through to full challenge
            if resp.status() != reqwest::StatusCode::UNAUTHORIZED {
                anyhow::bail!("NVR request failed: {} {}", resp.status(), url);
            }
        }

        // Full challenge-response: send unauthenticated, get 401, retry
        let initial = match method {
            "POST" => self.client.post(url),
            _ => self.client.get(url),
        };
        let resp = initial.send().await?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            let www_auth = resp
                .headers()
                .get("www-authenticate")
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| anyhow::anyhow!("no WWW-Authenticate header in 401 response"))?;

            let challenge = parse_digest_challenge(www_auth)?;
            self.cache_challenge(&challenge);

            let nc = "00000001";
            let cnonce = format!("{:08x}", rand_cnonce());
            let auth_header = compute_digest_header(
                &self.username,
                &self.password,
                &challenge.realm,
                &challenge.nonce,
                &challenge.qop,
                method,
                &uri_path,
                nc,
                &cnonce,
            );

            let retry = match method {
                "POST" => self.client.post(url),
                _ => self.client.get(url),
            };
            let resp = retry
                .header("Authorization", auth_header)
                .send()
                .await?;

            if !resp.status().is_success() {
                anyhow::bail!(
                    "NVR request failed after digest auth: {} {}",
                    resp.status(),
                    url
                );
            }
            Ok(resp)
        } else if resp.status().is_success() {
            Ok(resp)
        } else {
            anyhow::bail!("NVR request failed: {} {}", resp.status(), url);
        }
    }
}

/// Parse the factory/create response to extract the search handle ID.
/// Response format: `result=12345` or `result=N`
fn parse_factory_result(body: &str) -> anyhow::Result<String> {
    for line in body.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("result=") {
            return Ok(val.to_string());
        }
    }
    anyhow::bail!("failed to parse factory result from NVR response: {body}")
}

/// Parse the findNextFile response into a list of NvrFileInfo.
/// Dahua returns key=value lines with indexed status entries.
fn parse_file_list(body: &str) -> Vec<NvrFileInfo> {
    let mut files: Vec<NvrFileInfo> = Vec::new();
    let mut current: Option<NvrFileInfoBuilder> = None;

    for line in body.lines() {
        let line = line.trim();

        // Check for "found=N" to know how many results
        if line.starts_with("found=") {
            let count: usize = line
                .strip_prefix("found=")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            if count == 0 {
                return files;
            }
            continue;
        }

        // Parse "items[N].key=value" format
        if let Some(rest) = line.strip_prefix("items[") {
            if let Some(dot_pos) = rest.find("].") {
                let idx: usize = rest[..dot_pos].parse().unwrap_or(0);
                let kv = &rest[dot_pos + 2..];

                // Ensure we have a builder for this index
                while files.len() + (if current.is_some() { 1 } else { 0 }) <= idx {
                    if let Some(builder) = current.take() {
                        if let Some(info) = builder.build() {
                            files.push(info);
                        }
                    }
                    current = Some(NvrFileInfoBuilder::default());
                }
                if current.is_none() {
                    current = Some(NvrFileInfoBuilder::default());
                }

                if let Some(ref mut builder) = current {
                    if let Some((key, value)) = kv.split_once('=') {
                        match key {
                            "FilePath" => builder.file_path = Some(value.to_string()),
                            "Channel" => builder.channel = value.parse().ok(),
                            "StartTime" => builder.start_time = Some(value.to_string()),
                            "EndTime" => builder.end_time = Some(value.to_string()),
                            "Length" => builder.file_size = value.parse().ok(),
                            "Type" => builder.file_type = Some(value.to_string()),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // Don't forget the last builder
    if let Some(builder) = current {
        if let Some(info) = builder.build() {
            files.push(info);
        }
    }

    files
}

#[derive(Default)]
struct NvrFileInfoBuilder {
    channel: Option<u32>,
    start_time: Option<String>,
    end_time: Option<String>,
    file_path: Option<String>,
    file_size: Option<u64>,
    file_type: Option<String>,
}

impl NvrFileInfoBuilder {
    fn build(self) -> Option<NvrFileInfo> {
        Some(NvrFileInfo {
            channel: self.channel.unwrap_or(0),
            start_time: self.start_time?,
            end_time: self.end_time?,
            file_path: self.file_path?,
            file_size: self.file_size.unwrap_or(0),
            file_type: self.file_type.unwrap_or_else(|| "dav".to_string()),
        })
    }
}

/// Parse a WWW-Authenticate: Digest header into its components.
fn parse_digest_challenge(header: &str) -> anyhow::Result<DigestChallenge> {
    let header = header.strip_prefix("Digest ").unwrap_or(header);

    let realm = extract_quoted_value(header, "realm")
        .ok_or_else(|| anyhow::anyhow!("missing realm in digest challenge"))?;
    let nonce = extract_quoted_value(header, "nonce")
        .ok_or_else(|| anyhow::anyhow!("missing nonce in digest challenge"))?;
    let qop = extract_quoted_value(header, "qop").unwrap_or_else(|| "auth".to_string());

    Ok(DigestChallenge { realm, nonce, qop })
}

/// Extract a quoted value from a comma-separated key="value" string.
fn extract_quoted_value(s: &str, key: &str) -> Option<String> {
    let pattern = format!("{}=\"", key);
    if let Some(start) = s.find(&pattern) {
        let value_start = start + pattern.len();
        if let Some(end) = s[value_start..].find('"') {
            return Some(s[value_start..value_start + end].to_string());
        }
    }
    // Also try unquoted: key=value
    let pattern_unquoted = format!("{}=", key);
    if let Some(start) = s.find(&pattern_unquoted) {
        let value_start = start + pattern_unquoted.len();
        let rest = &s[value_start..];
        let end = rest.find(',').unwrap_or(rest.len());
        let val = rest[..end].trim().trim_matches('"');
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }
    None
}

/// Compute the full Authorization: Digest header value.
fn compute_digest_header(
    username: &str,
    password: &str,
    realm: &str,
    nonce: &str,
    qop: &str,
    method: &str,
    uri: &str,
    nc: &str,
    cnonce: &str,
) -> String {
    let ha1 = md5_hex(&format!("{username}:{realm}:{password}"));
    let ha2 = md5_hex(&format!("{method}:{uri}"));
    let response = md5_hex(&format!("{ha1}:{nonce}:{nc}:{cnonce}:{qop}:{ha2}"));

    format!(
        "Digest username=\"{username}\", realm=\"{realm}\", nonce=\"{nonce}\", \
         uri=\"{uri}\", qop={qop}, nc={nc}, cnonce=\"{cnonce}\", response=\"{response}\""
    )
}

/// Compute MD5 hex digest of a string.
fn md5_hex(input: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    format!("{:032x}", result)
}

/// Generate a simple pseudo-random cnonce value.
fn rand_cnonce() -> u32 {
    // Use a simple time-based approach; no need for crypto-random here
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    (now.as_nanos() & 0xFFFF_FFFF) as u32
}
