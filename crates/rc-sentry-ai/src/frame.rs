use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct FrameData {
    pub data: Vec<u8>,
    pub timestamp: Instant,
    pub frame_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CameraFrameStatus {
    pub last_frame_secs_ago: f64,
    pub frames_total: u64,
}

#[derive(Debug, Clone)]
pub struct FrameBuffer {
    inner: Arc<RwLock<HashMap<String, FrameData>>>,
}

impl FrameBuffer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update(&self, camera_name: &str, data: Vec<u8>) {
        let mut map = self.inner.write().await;
        let entry = map.entry(camera_name.to_string()).or_insert_with(|| FrameData {
            data: Vec::new(),
            timestamp: Instant::now(),
            frame_count: 0,
        });
        entry.data = data;
        entry.timestamp = Instant::now();
        entry.frame_count += 1;
    }

    pub async fn get(&self, camera_name: &str) -> Option<FrameData> {
        let map = self.inner.read().await;
        map.get(camera_name).cloned()
    }

    pub async fn status(&self) -> HashMap<String, CameraFrameStatus> {
        let map = self.inner.read().await;
        let now = Instant::now();
        map.iter()
            .map(|(name, frame)| {
                let status = CameraFrameStatus {
                    last_frame_secs_ago: now.duration_since(frame.timestamp).as_secs_f64(),
                    frames_total: frame.frame_count,
                };
                (name.clone(), status)
            })
            .collect()
    }
}
