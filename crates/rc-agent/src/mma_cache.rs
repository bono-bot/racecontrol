//! MMA Consensus Cache — avoids re-diagnosing the same problem twice.
//!
//! When an MMA protocol completes successfully, the consensus result is cached
//! by problem_hash. On cache hit, Steps 1-3 are skipped and only Step 4 (VERIFY)
//! re-runs with a fresh adversarial model to confirm the fix still applies.
//!
//! Cache invalidation:
//! - TTL of 24 hours (configurable via DEFAULT_TTL_HOURS)
//! - Build-based: cache entries created with a different build_id are ignored
//!
//! Stored in mesh_kb.db alongside knowledge_base, model_reputation, budget_state.

use rusqlite::{params, Connection};

use crate::knowledge_base::KB_PATH;

const LOG_TARGET: &str = "mma-cache";

/// Default TTL for cached consensus results (hours).
const DEFAULT_TTL_HOURS: i64 = 24;

/// MMA consensus cache backed by SQLite.
pub struct MmaCache {
    conn: Connection,
}

/// A cached MMA consensus entry.
#[derive(Debug)]
pub struct CacheEntry {
    pub consensus_json: String,
    pub total_cost: f64,
    pub build_id: String,
    pub created_at: String,
}

impl MmaCache {
    /// Open the cache (creates table if needed). Safe to call on existing DB.
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS mma_cache (
                problem_hash TEXT PRIMARY KEY,
                consensus_json TEXT NOT NULL,
                total_cost REAL NOT NULL DEFAULT 0.0,
                build_id TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                ttl_hours INTEGER NOT NULL DEFAULT 24
            );",
        )?;
        tracing::debug!(target: LOG_TARGET, "mma_cache schema migration complete");
        Ok(Self { conn })
    }

    /// Look up a cached consensus for the given problem hash.
    ///
    /// Returns `Some(entry)` if a valid (non-expired, matching build_id) cache entry exists.
    pub fn get(&self, problem_hash: &str, current_build_id: &str) -> Option<CacheEntry> {
        let mut stmt = match self.conn.prepare(
            "SELECT consensus_json, total_cost, build_id, created_at, ttl_hours
             FROM mma_cache WHERE problem_hash = ?1",
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, error = %e, "Failed to prepare cache lookup");
                return None;
            }
        };

        let result = stmt.query_row(params![problem_hash], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        });

        match result {
            Ok((consensus_json, total_cost, build_id, created_at, ttl_hours)) => {
                // Check build_id match — environment changed = cache invalid
                if build_id != current_build_id {
                    tracing::debug!(
                        target: LOG_TARGET,
                        cached_build = %build_id,
                        current_build = %current_build_id,
                        "Cache miss: build_id changed"
                    );
                    return None;
                }

                // Check TTL
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&created_at) {
                    let age_hours = (chrono::Utc::now() - ts.with_timezone(&chrono::Utc)).num_hours();
                    if age_hours > ttl_hours {
                        tracing::debug!(
                            target: LOG_TARGET,
                            age_hours,
                            ttl_hours,
                            "Cache miss: expired ({}h > {}h TTL)",
                            age_hours, ttl_hours
                        );
                        return None;
                    }
                }

                tracing::info!(
                    target: LOG_TARGET,
                    problem_hash,
                    prior_cost = total_cost,
                    "Cache HIT — skipping Steps 1-3 (saved ${:.2})",
                    total_cost
                );

                Some(CacheEntry {
                    consensus_json,
                    total_cost,
                    build_id,
                    created_at,
                })
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, error = %e, "Cache lookup error");
                None
            }
        }
    }

    /// Store a consensus result in the cache.
    pub fn put(
        &self,
        problem_hash: &str,
        consensus_json: &str,
        total_cost: f64,
        build_id: &str,
    ) -> anyhow::Result<()> {
        let created_at = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO mma_cache (problem_hash, consensus_json, total_cost, build_id, created_at, ttl_hours)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(problem_hash) DO UPDATE SET
               consensus_json = excluded.consensus_json,
               total_cost = excluded.total_cost,
               build_id = excluded.build_id,
               created_at = excluded.created_at",
            params![problem_hash, consensus_json, total_cost, build_id, created_at, DEFAULT_TTL_HOURS],
        )?;
        tracing::info!(
            target: LOG_TARGET,
            problem_hash,
            cost = total_cost,
            "Cached MMA consensus (TTL={}h)",
            DEFAULT_TTL_HOURS
        );
        Ok(())
    }

    /// Remove expired entries (housekeeping).
    #[allow(dead_code)]
    pub fn cleanup_expired(&self) -> anyhow::Result<u64> {
        let deleted = self.conn.execute(
            "DELETE FROM mma_cache WHERE datetime(created_at, '+' || ttl_hours || ' hours') < datetime('now')",
            [],
        )?;
        if deleted > 0 {
            tracing::info!(target: LOG_TARGET, deleted, "Cleaned up expired cache entries");
        }
        Ok(deleted as u64)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_and_put_get() {
        let cache = MmaCache::open(":memory:").unwrap();
        cache.put("hash_1", r#"{"test":true}"#, 3.50, "abc123").unwrap();

        let entry = cache.get("hash_1", "abc123").expect("should find entry");
        assert_eq!(entry.consensus_json, r#"{"test":true}"#);
        assert!((entry.total_cost - 3.50).abs() < 0.001);
    }

    #[test]
    fn test_cache_miss_different_build() {
        let cache = MmaCache::open(":memory:").unwrap();
        cache.put("hash_1", r#"{"test":true}"#, 3.50, "build_old").unwrap();

        let entry = cache.get("hash_1", "build_new");
        assert!(entry.is_none(), "different build_id should miss");
    }

    #[test]
    fn test_cache_miss_nonexistent() {
        let cache = MmaCache::open(":memory:").unwrap();
        let entry = cache.get("nonexistent", "abc123");
        assert!(entry.is_none());
    }

    #[test]
    fn test_put_overwrites() {
        let cache = MmaCache::open(":memory:").unwrap();
        cache.put("hash_1", r#"{"v":1}"#, 2.0, "abc").unwrap();
        cache.put("hash_1", r#"{"v":2}"#, 4.0, "abc").unwrap();

        let entry = cache.get("hash_1", "abc").unwrap();
        assert_eq!(entry.consensus_json, r#"{"v":2}"#);
        assert!((entry.total_cost - 4.0).abs() < 0.001);
    }
}
