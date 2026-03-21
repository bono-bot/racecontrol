use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Face recognition tracker that suppresses redundant recognitions.
///
/// Enforces a per-person cooldown period to prevent logging the same
/// person repeatedly when they remain in camera view.
pub struct FaceTracker {
    last_seen: Mutex<HashMap<i64, Instant>>,
    cooldown: Duration,
}

impl FaceTracker {
    /// Create a new tracker with the given cooldown in seconds.
    pub fn new(cooldown_secs: u64) -> Self {
        Self {
            last_seen: Mutex::new(HashMap::new()),
            cooldown: Duration::from_secs(cooldown_secs),
        }
    }

    /// Check if a person should be reported (not seen within cooldown period).
    ///
    /// Returns `true` if this is the first sighting or the cooldown has elapsed.
    /// Updates the last-seen timestamp when returning `true`.
    pub fn should_report(&self, person_id: i64) -> bool {
        let mut map = self.last_seen.lock().expect("tracker lock poisoned");
        let now = Instant::now();

        match map.get(&person_id) {
            Some(last) if now.duration_since(*last) < self.cooldown => false,
            _ => {
                map.insert(person_id, now);
                true
            }
        }
    }

    /// Remove entries older than the cooldown period to prevent memory leaks.
    ///
    /// Call periodically (e.g., every 5 minutes).
    pub fn cleanup(&self) {
        let mut map = self.last_seen.lock().expect("tracker lock poisoned");
        let now = Instant::now();
        map.retain(|_, last| now.duration_since(*last) < self.cooldown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_report_returns_true() {
        let tracker = FaceTracker::new(60);
        assert!(tracker.should_report(1), "first sighting should be reported");
    }

    #[test]
    fn test_second_report_within_cooldown_returns_false() {
        let tracker = FaceTracker::new(60);
        assert!(tracker.should_report(1));
        assert!(
            !tracker.should_report(1),
            "second call within cooldown should return false"
        );
    }

    #[test]
    fn test_different_person_returns_true() {
        let tracker = FaceTracker::new(60);
        assert!(tracker.should_report(1));
        assert!(
            tracker.should_report(2),
            "different person_id should be reported"
        );
    }
}
