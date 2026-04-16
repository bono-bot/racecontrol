//! Per-pod violation store with time-based eviction and fingerprint dedup.
//!
//! Extracted from fleet_health.rs (Phase 385 ARCH-03).

use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};

use rc_common::types::ProcessViolation;

/// Per-pod violation history with time-based eviction and fingerprint dedup.
///
/// MMA-P1 fixes (4-model consensus):
/// - Time-based eviction (24h) instead of 100-entry FIFO cap — real violations
///   can no longer be evicted by high-frequency false positives (vendor schtasks).
/// - Fingerprint dedup: recurring "reported" violations for the same process/task
///   increment `seen_count` instead of creating new entries.
/// - repeat_offender_check works in report_only mode (not gated on "killed").
/// - Future timestamps rejected (clock skew protection).
/// - Hard cap at 1000 entries as safety net (time-based eviction is primary).
#[derive(Debug, Clone, Default)]
pub struct ViolationStore {
    entries: VecDeque<ProcessViolation>,
    /// Dedup index: fingerprint (lowercase name + violation_type) → index of last entry.
    /// Used to increment seen_count for recurring "reported" violations instead of duplicating.
    dedup_index: HashMap<String, usize>,
    /// Rolling counter of scan failures (not stored as violations).
    pub scan_failure_count: u32,
}

impl ViolationStore {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            dedup_index: HashMap::new(),
            scan_failure_count: 0,
        }
    }

    /// Fingerprint for dedup: lowercased name + violation type string.
    fn fingerprint(v: &ProcessViolation) -> String {
        format!("{}:{:?}", v.name.to_lowercase(), v.violation_type)
    }

    /// Insert a violation with dedup and time-based eviction.
    ///
    /// - "reported" violations with the same fingerprint increment `consecutive_count`
    ///   on the existing entry instead of creating duplicates.
    /// - "killed"/"disabled"/"removed" violations always create new entries (they represent actions).
    /// - Entries older than 24h are evicted on push.
    /// - Hard cap at 1000 entries as safety net.
    pub fn push(&mut self, v: ProcessViolation) {
        let now = Utc::now();

        // MMA-P2: Reject future timestamps (clock skew protection)
        if let Ok(parsed) = DateTime::parse_from_rfc3339(&v.timestamp) {
            let age_secs = (now - parsed.with_timezone(&Utc)).num_seconds();
            if age_secs < -60 {
                // More than 60s in the future — likely clock skew, drop silently
                return;
            }
        }

        // Time-based eviction: remove entries older than 24h
        self.evict_stale(now);

        // Hard cap safety net FIRST (before dedup lookup to keep indices valid)
        if self.entries.len() >= 1000 {
            if let Some(old) = self.entries.pop_front() {
                self.dedup_index.remove(&Self::fingerprint(&old));
            }
            // MMA-Iter2-P1: Rebuild dedup index after pop (indices shifted).
            // Must happen BEFORE dedup lookup to prevent stale index access.
            self.rebuild_dedup_index();
        }

        // Dedup: for "reported" violations, increment existing entry's count
        if v.action_taken == "reported" {
            let fp = Self::fingerprint(&v);
            if let Some(&idx) = self.dedup_index.get(&fp)
                && let Some(existing) = self.entries.get_mut(idx) {
                    existing.consecutive_count = existing.consecutive_count.saturating_add(1);
                    existing.timestamp = v.timestamp; // update to latest sighting
                    return;
                }
            // New fingerprint — track index AFTER push_back (done below)
        }

        self.entries.push_back(v);

        // Update dedup index for the newly pushed entry (if "reported")
        if let Some(last) = self.entries.back()
            && last.action_taken == "reported" {
                let fp = Self::fingerprint(last);
                self.dedup_index.insert(fp, self.entries.len() - 1);
            }
    }

    /// Record a scan failure (not a violation — tracked separately for OTA gating).
    pub fn record_scan_failure(&mut self) {
        self.scan_failure_count = self.scan_failure_count.saturating_add(1);
    }

    /// Evict entries older than 24 hours.
    fn evict_stale(&mut self, now: DateTime<Utc>) {
        let cutoff = now - chrono::Duration::hours(24);
        let before_len = self.entries.len();
        self.entries.retain(|v| {
            DateTime::parse_from_rfc3339(&v.timestamp)
                .map(|t| t.with_timezone(&Utc) >= cutoff)
                .unwrap_or(false) // drop unparseable timestamps
        });
        if self.entries.len() != before_len {
            self.rebuild_dedup_index();
        }
    }

    /// Rebuild dedup index after eviction (only for "reported" entries).
    fn rebuild_dedup_index(&mut self) {
        self.dedup_index.clear();
        for (idx, v) in self.entries.iter().enumerate() {
            if v.action_taken == "reported" {
                self.dedup_index.insert(Self::fingerprint(v), idx);
            }
        }
    }

    /// Count distinct violations within the last 24 hours.
    /// MMA-P1: Uses time-based window with future-timestamp rejection.
    /// MMA-Iter2-P2: Excludes guard degradation notifications (state changes, not violations).
    pub fn violation_count_24h(&self, now: DateTime<Utc>) -> u32 {
        let cutoff = now - chrono::Duration::hours(24);
        self.entries.iter().filter(|v| {
            // Skip guard degradation notifications — they're state changes, not violations
            if v.action_taken == "guard_degraded_to_report_only" {
                return false;
            }
            DateTime::parse_from_rfc3339(&v.timestamp)
                .map(|t| {
                    let ts = t.with_timezone(&Utc);
                    // MMA-P2: Reject future timestamps in count too
                    ts >= cutoff && ts <= now + chrono::Duration::seconds(60)
                })
                .unwrap_or(false)
        }).count() as u32
    }

    /// Timestamp of the most recently pushed violation, or None if empty.
    pub fn last_violation_at(&self) -> Option<&str> {
        self.entries.back().map(|v| v.timestamp.as_str())
    }

    /// Returns true if `violation` should trigger escalation:
    /// MMA-P1: Works in ALL modes (report_only + kill_and_report).
    /// Checks if the same process name has been seen >= threshold times
    /// in the last 300 seconds, regardless of action_taken.
    pub fn repeat_offender_check(&self, violation: &ProcessViolation, now: DateTime<Utc>) -> bool {
        let name_lower = violation.name.to_lowercase();
        let window_start = now - chrono::Duration::seconds(300);
        // MMA-Iter2-P2: Count DISTINCT entries, not consecutive_count sum.
        // Summing consecutive_count causes false positives on benign vendor tasks
        // that get re-reported hourly (consecutive_count grows to 100+).
        let distinct_entries: usize = self.entries.iter()
            .filter(|v| {
                v.name.to_lowercase() == name_lower
                    && DateTime::parse_from_rfc3339(&v.timestamp)
                        .map(|t| {
                            let ts = t.with_timezone(&Utc);
                            ts >= window_start && ts <= now
                        })
                        .unwrap_or(false)
            })
            .count();
        // Threshold: 5 distinct entries in 300s = repeat offender (works in report_only mode)
        distinct_entries >= 5
    }
}
