//! Phase 335: Track outline extraction from AC fast_lane.ai files.
//!
//! AC stores track AI data in `content/tracks/{track_id}/ai/fast_lane.ai`.
//! Format: 4-byte header (point count as i32) + 4 padding bytes + array of
//! float32 triplets (x, y, z) with stride of 3 + extra fields per point.
//!
//! We extract x/z coordinates (y is elevation), normalize to 0.0-1.0 range,
//! and serve as JSON for the spectator circuit viewer.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

const LOG_TARGET: &str = "track-outline";

/// A track outline: array of normalized 2D points along the racing line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackOutline {
    pub track_id: String,
    /// Optional track config (e.g., "gp", "drift") — empty string for default layout.
    #[serde(default)]
    pub config: String,
    /// Points along the track spline, normalized to [0.0, 1.0] range.
    /// Each point is [x, z] where x is lateral and z is longitudinal.
    pub points: Vec<[f32; 2]>,
    /// Number of original points before downsampling.
    pub original_point_count: usize,
}

/// In-memory cache of parsed track outlines.
pub struct TrackOutlineCache {
    outlines: RwLock<HashMap<String, TrackOutline>>,
    data_dir: PathBuf,
}

impl TrackOutlineCache {
    /// Create a new cache. `data_dir` is the path to `data/track_outlines/`.
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            outlines: RwLock::new(HashMap::new()),
            data_dir,
        }
    }

    /// Load all pre-extracted JSON outlines from the data directory.
    pub fn load_preextracted(&self) {
        let Ok(entries) = std::fs::read_dir(&self.data_dir) else {
            tracing::warn!(target: LOG_TARGET, "Track outlines directory not found: {:?}", self.data_dir);
            return;
        };

        let mut count = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            match std::fs::read_to_string(&path) {
                Ok(data) => match serde_json::from_str::<TrackOutline>(&data) {
                    Ok(outline) => {
                        let key = if outline.config.is_empty() {
                            outline.track_id.clone()
                        } else {
                            format!("{}/{}", outline.track_id, outline.config)
                        };
                        tracing::debug!(
                            target: LOG_TARGET,
                            "Loaded track outline: {} ({} points)",
                            key,
                            outline.points.len()
                        );
                        if let Ok(mut cache) = self.outlines.write() {
                            cache.insert(key, outline);
                            count += 1;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(target: LOG_TARGET, "Failed to parse {:?}: {}", path, e);
                    }
                },
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, "Failed to read {:?}: {}", path, e);
                }
            }
        }
        tracing::info!(target: LOG_TARGET, "Loaded {} pre-extracted track outlines", count);
    }

    /// Get a track outline by track_id (and optional config).
    pub fn get(&self, track_id: &str, config: &str) -> Option<TrackOutline> {
        let guard = self.outlines.read().ok()?;
        let key = if config.is_empty() {
            track_id.to_string()
        } else {
            format!("{}/{}", track_id, config)
        };
        guard.get(&key).cloned()
    }

    /// Get all available track outlines.
    pub fn list(&self) -> Vec<TrackOutline> {
        let guard = self.outlines.read().unwrap_or_else(|e| e.into_inner());
        guard.values().cloned().collect()
    }

    /// Parse a fast_lane.ai file and store the result in cache + write to data_dir.
    pub fn extract_and_cache(&self, track_id: &str, config: &str, ai_path: &Path) -> Option<TrackOutline> {
        let outline = parse_fast_lane_ai(track_id, config, ai_path)?;

        // Store in memory cache
        let key = if config.is_empty() {
            track_id.to_string()
        } else {
            format!("{}/{}", track_id, config)
        };
        if let Ok(mut cache) = self.outlines.write() {
            cache.insert(key, outline.clone());
        }

        // Write to disk
        let filename = if config.is_empty() {
            format!("{}.json", track_id)
        } else {
            format!("{}_{}.json", track_id, config)
        };
        let out_path = self.data_dir.join(&filename);
        match serde_json::to_string_pretty(&outline) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&out_path, &json) {
                    tracing::warn!(target: LOG_TARGET, "Failed to write {:?}: {}", out_path, e);
                } else {
                    tracing::info!(target: LOG_TARGET, "Extracted and cached: {} -> {:?}", track_id, out_path);
                }
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "Failed to serialize outline for {}: {}", track_id, e);
            }
        }

        Some(outline)
    }
}

/// Parse a fast_lane.ai binary file into a TrackOutline.
///
/// AC fast_lane.ai format (reverse-engineered):
/// - Bytes 0-3: point count (i32 little-endian)
/// - Bytes 4-7: padding/version
/// - Then for each point: 6 floats (24 bytes):
///   [x, y, z, dx, dy, dz] where dx/dy/dz are direction vectors
///
/// Some versions have additional data per point (length, id_left, id_right, etc.)
/// but the first 3 floats per point are always position.
pub fn parse_fast_lane_ai(track_id: &str, config: &str, path: &Path) -> Option<TrackOutline> {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "Cannot read {:?}: {}", path, e);
            return None;
        }
    };

    if data.len() < 8 {
        tracing::warn!(target: LOG_TARGET, "fast_lane.ai too small ({} bytes): {:?}", data.len(), path);
        return None;
    }

    // Read point count from header
    let point_count = i32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

    if point_count == 0 || point_count > 100_000 {
        tracing::warn!(
            target: LOG_TARGET,
            "Suspicious point count {} in {:?}",
            point_count,
            path
        );
        return None;
    }

    // AC fast_lane.ai: after the 8-byte header, each "node" has:
    // 4 floats position (x, y, z, length) = 16 bytes
    // then additional data depending on version.
    //
    // The most common format has per-node stride of:
    //   12 bytes (x,y,z) + 4 (length) + 12 (direction xyz) + 4*N extra fields
    //
    // We try multiple known strides to find one that fits the file size.
    let header_size = 8;
    let remaining = data.len() - header_size;

    // Known strides per point in various AC versions:
    // Stride 24: x,y,z + dx,dy,dz (6 floats)
    // Stride 28: x,y,z + length + dx,dy,dz (7 floats)
    // Stride 36: x,y,z + length + dx,dy,dz + id_left + id_right (9 floats)
    // Stride 44: 11 floats
    // Stride 48: 12 floats
    let possible_strides: &[usize] = &[48, 44, 36, 28, 24, 20, 16, 12];

    let stride = possible_strides
        .iter()
        .find(|&&s| remaining % s == 0 && remaining / s == point_count)
        .copied();

    let stride = match stride {
        Some(s) => s,
        None => {
            // Fallback: try to compute stride from file size
            if point_count > 0 {
                let computed = remaining / point_count;
                if computed >= 12 && remaining == computed * point_count {
                    tracing::debug!(
                        target: LOG_TARGET,
                        "Using computed stride {} for {:?}",
                        computed,
                        path
                    );
                    computed
                } else {
                    tracing::warn!(
                        target: LOG_TARGET,
                        "Cannot determine stride for {:?} (remaining={}, points={})",
                        path,
                        remaining,
                        point_count
                    );
                    return None;
                }
            } else {
                return None;
            }
        }
    };

    // Extract x, z coordinates (skip y which is elevation)
    let mut raw_points: Vec<(f32, f32)> = Vec::with_capacity(point_count);
    for i in 0..point_count {
        let offset = header_size + i * stride;
        if offset + 12 > data.len() {
            break;
        }
        let x = f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
        // y is at offset+4..offset+8 (elevation, skip)
        let z = f32::from_le_bytes([data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]]);

        if x.is_finite() && z.is_finite() {
            raw_points.push((x, z));
        }
    }

    if raw_points.len() < 3 {
        tracing::warn!(target: LOG_TARGET, "Too few valid points ({}) in {:?}", raw_points.len(), path);
        return None;
    }

    let original_count = raw_points.len();

    // Normalize to 0.0-1.0 range
    let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
    let (mut min_z, mut max_z) = (f32::MAX, f32::MIN);
    for &(x, z) in &raw_points {
        if x < min_x { min_x = x; }
        if x > max_x { max_x = x; }
        if z < min_z { min_z = z; }
        if z > max_z { max_z = z; }
    }

    let range_x = (max_x - min_x).max(1.0);
    let range_z = (max_z - min_z).max(1.0);
    // Use the larger range for both axes to preserve aspect ratio
    let range = range_x.max(range_z);
    // Center the smaller axis
    let offset_x = (range - range_x) / 2.0;
    let offset_z = (range - range_z) / 2.0;

    let normalized: Vec<[f32; 2]> = raw_points
        .iter()
        .map(|&(x, z)| {
            [
                ((x - min_x + offset_x) / range).clamp(0.0, 1.0),
                ((z - min_z + offset_z) / range).clamp(0.0, 1.0),
            ]
        })
        .collect();

    // Downsample if too many points (keep ~500 for smooth canvas rendering)
    let max_points = 500;
    let points = if normalized.len() > max_points {
        let step = normalized.len() as f64 / max_points as f64;
        (0..max_points)
            .map(|i| normalized[(i as f64 * step) as usize])
            .collect()
    } else {
        normalized
    };

    Some(TrackOutline {
        track_id: track_id.to_string(),
        config: config.to_string(),
        points,
        original_point_count: original_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_fake_fast_lane(point_count: usize, stride: usize) -> Vec<u8> {
        let mut data = Vec::new();
        // Header: point count (i32 LE) + 4 bytes padding
        data.extend_from_slice(&(point_count as i32).to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);

        // Points: x, y, z + padding to fill stride
        for i in 0..point_count {
            let angle = 2.0 * std::f32::consts::PI * (i as f32 / point_count as f32);
            let x = angle.cos() * 100.0;
            let y = 0.0f32; // elevation
            let z = angle.sin() * 100.0;

            data.extend_from_slice(&x.to_le_bytes());
            data.extend_from_slice(&y.to_le_bytes());
            data.extend_from_slice(&z.to_le_bytes());

            // Pad remaining bytes in stride
            let remaining = stride - 12;
            data.extend(std::iter::repeat(0u8).take(remaining));
        }

        data
    }

    #[test]
    fn test_parse_circular_track() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("fast_lane.ai");
        let data = create_fake_fast_lane(100, 24);
        std::fs::write(&path, &data).unwrap();

        let outline = parse_fast_lane_ai("test_track", "", &path).unwrap();
        assert_eq!(outline.track_id, "test_track");
        assert!(!outline.points.is_empty());
        assert!(outline.points.len() <= 500);

        // All points should be in [0, 1] range
        for p in &outline.points {
            assert!(p[0] >= 0.0 && p[0] <= 1.0, "x out of range: {}", p[0]);
            assert!(p[1] >= 0.0 && p[1] <= 1.0, "z out of range: {}", p[1]);
        }
    }

    #[test]
    fn test_parse_large_track_downsampled() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("fast_lane.ai");
        let data = create_fake_fast_lane(2000, 36);
        std::fs::write(&path, &data).unwrap();

        let outline = parse_fast_lane_ai("big_track", "", &path).unwrap();
        assert_eq!(outline.points.len(), 500); // downsampled
        assert_eq!(outline.original_point_count, 2000);
    }

    #[test]
    fn test_parse_too_small() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("fast_lane.ai");
        std::fs::write(&path, &[0u8; 4]).unwrap();

        assert!(parse_fast_lane_ai("tiny", "", &path).is_none());
    }

    #[test]
    fn test_cache_operations() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cache = TrackOutlineCache::new(tmp.path().to_path_buf());

        // Create a fake fast_lane.ai
        let ai_dir = tmp.path().join("ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        let ai_path = ai_dir.join("fast_lane.ai");
        let data = create_fake_fast_lane(50, 24);
        std::fs::write(&ai_path, &data).unwrap();

        // Extract and cache
        let outline = cache.extract_and_cache("monza", "", &ai_path).unwrap();
        assert_eq!(outline.track_id, "monza");

        // Should be in cache now
        let cached = cache.get("monza", "").unwrap();
        assert_eq!(cached.points.len(), outline.points.len());

        // Should have written JSON file
        let json_path = tmp.path().join("monza.json");
        assert!(json_path.exists());

        // Load preextracted should work
        let cache2 = TrackOutlineCache::new(tmp.path().to_path_buf());
        cache2.load_preextracted();
        assert!(cache2.get("monza", "").is_some());
    }
}
