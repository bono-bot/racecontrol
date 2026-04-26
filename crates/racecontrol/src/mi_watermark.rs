//! PACT-091 — Mesh Intelligence edit watermark (v1).
//!
//! Every autonomous Mesh Intelligence (MI) write to the database, source files,
//! or runtime state should leave a 4-layer attribution trail so forensic queries
//! ("which subsystem made this edit?") become a single grep / single SQL query
//! instead of a 5-file source archeology dive.
//!
//! Layers:
//! - **L1 git trailer** — when MI auto-commits source/config (no MI auto-commit
//!   path exists today; design-only until that capability ships).
//! - **L2 in-file marker comment** — for auto-edited blocks; emitted by callers
//!   via [`mi_edit_marker`].
//! - **L3 log line prefix** — every MI runtime write logs through `mi_edit`
//!   target; see [`LOG_TARGET_MI_EDIT`] and [`log_mi_edit`].
//! - **L4 audit_log row** — every MI DB write inserts a paired audit row via
//!   [`audit_log_mi_edit`].
//!
//! Functional MI taxonomy (sub= enum, see [`MiSubsystem`]) — 7 subsystems:
//! me, re, pa, rc, fi, pe, ad.

use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Shared tracing target for L3 log-line prefix. Keep this stable so JSONL
/// grep across racecontrol logs is independent of caller subsystem.
pub const LOG_TARGET_MI_EDIT: &str = "mi_edit";

/// Functional Mesh Intelligence subsystem identifier (sub= enum).
///
/// Mapped to a 2-letter code that survives JSON roundtrips and grep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiSubsystem {
    /// `me` — mesh_intelligence (diagnostic engine + gossip)
    MeshIntelligence,
    /// `re` — race_engineer (auto-relaunch on crash, game_launcher_state.rs)
    RaceEngineer,
    /// `pa` — pod_agent local healer (rc-agent/event_loop.rs)
    PodAgent,
    /// `rc` — recovery (recovery.rs)
    Recovery,
    /// `fi` — fleet_intelligence (Phase 366 health scoring)
    FleetIntelligence,
    /// `pe` — policy_engine
    PolicyEngine,
    /// `ad` — ai_debugger (rc-agent pod-local Tier 0-4 healer, ai_debugger.rs).
    /// Added per PACT-104 finding 2026-04-27 (destructive-fix-during-launch race).
    AiDebugger,
}

impl MiSubsystem {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MeshIntelligence => "me",
            Self::RaceEngineer => "re",
            Self::PodAgent => "pa",
            Self::Recovery => "rc",
            Self::FleetIntelligence => "fi",
            Self::PolicyEngine => "pe",
            Self::AiDebugger => "ad",
        }
    }

    /// Inverse of [`code`] — parse a 2-letter sub code back to the enum.
    /// Returns `None` for unknown codes (including `"human"`, which is the
    /// non-MI default for `created_by_agent` columns).
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "me" => Some(Self::MeshIntelligence),
            "re" => Some(Self::RaceEngineer),
            "pa" => Some(Self::PodAgent),
            "rc" => Some(Self::Recovery),
            "fi" => Some(Self::FleetIntelligence),
            "pe" => Some(Self::PolicyEngine),
            "ad" => Some(Self::AiDebugger),
            _ => None,
        }
    }
}

/// Context for a single MI edit. Pass to [`mi_edit_marker`], [`log_mi_edit`],
/// and [`audit_log_mi_edit`] to ensure a single edit produces consistent
/// attribution across L2/L3/L4.
#[derive(Debug, Clone)]
pub struct MiEditCtx {
    pub sub: MiSubsystem,
    /// Diagnostic tier 0-4 (0=deterministic, 4=cloud LLM).
    pub tier: u8,
    /// Solution UUID if this edit is bound to a fleet_solutions row;
    /// otherwise a deterministic identifier ("deterministic", "config-set", etc).
    pub solution_id: String,
    /// Confidence 0.0..1.0. Use 1.0 for deterministic actions.
    pub confidence: f64,
    /// Source node — fleet pod-id or "server" or "rc-agent".
    pub src_node: String,
    /// Model name if LLM-tier ("qwen3-coder-235b"), else "deterministic".
    pub model: String,
    /// Optional incident-id linking this edit to a fleet_incidents row.
    pub incident_id: Option<String>,
}

/// Build the canonical single-line marker (L2) for in-file annotation.
///
/// Format: `[MI-EDIT v1 sub=<sub> t=<tier> c=<conf> s=<sol> n=<node> m=<model> i=<incident> @ <iso>]`.
pub fn mi_edit_marker(ctx: &MiEditCtx) -> String {
    let now = chrono::Utc::now().to_rfc3339();
    format!(
        "[MI-EDIT v1 sub={} t={} c={:.2} s={} n={} m={} i={} @ {}]",
        ctx.sub.code(),
        ctx.tier,
        ctx.confidence,
        ctx.solution_id,
        ctx.src_node,
        ctx.model,
        ctx.incident_id.as_deref().unwrap_or("none"),
        now,
    )
}

/// Emit an L3 tracing log row through the shared `mi_edit` target.
///
/// `action` is a short verb ("audit_seed", "config_set", "kb_upsert").
/// `target` is what was edited (table name, file path, etc).
pub fn log_mi_edit(ctx: &MiEditCtx, action: &str, target: &str) {
    tracing::info!(
        target: LOG_TARGET_MI_EDIT,
        sub = ctx.sub.code(),
        tier = ctx.tier,
        action = action,
        target = target,
        solution_id = %ctx.solution_id,
        confidence = ctx.confidence,
        model = %ctx.model,
        src_node = %ctx.src_node,
        incident_id = ctx.incident_id.as_deref().unwrap_or(""),
        "MI edit emitted"
    );
}

/// Insert an L4 audit_log row attributing the edit to its MI subsystem.
///
/// PROPOSAL DEVIATION (PACT-091 implementation note): the proposal spec used
/// `action='MI_EDIT'` + `action_type='mesh-intelligence'`. The existing
/// audit_log.action CHECK constraint limits action to `('create','update','delete')`,
/// and there is no action_type column in the base schema. Path of least
/// resistance: write `action='update'` (passes CHECK) and add a sibling
/// `action_type` column via migration to carry the MI marker. Forensic queries
/// use `WHERE action_type LIKE 'mi-edit:%'` instead of `WHERE action='MI_EDIT'`.
/// The new_values JSON still carries the full marker context.
pub async fn audit_log_mi_edit(
    db: &SqlitePool,
    table: &str,
    row_id: &str,
    ctx: &MiEditCtx,
) -> anyhow::Result<()> {
    let action_type = format!("mi-edit:{}", ctx.sub.code());
    let new_values = json!({
        "marker": mi_edit_marker(ctx),
        "sub": ctx.sub.code(),
        "tier": ctx.tier,
        "solution_id": ctx.solution_id,
        "confidence": ctx.confidence,
        "model": ctx.model,
        "src_node": ctx.src_node,
        "incident_id": ctx.incident_id,
    });

    sqlx::query(
        "INSERT INTO audit_log
         (id, table_name, row_id, action, action_type, staff_id, new_values, created_at)
         VALUES (?1, ?2, ?3, 'update', ?4, ?5, ?6, datetime('now'))",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(table)
    .bind(row_id)
    .bind(&action_type)
    .bind(format!("mesh-intelligence:{}", ctx.sub.code()))
    .bind(new_values.to_string())
    .execute(db)
    .await?;

    log_mi_edit(ctx, "audit_log_write", table);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_includes_all_fields() {
        let ctx = MiEditCtx {
            sub: MiSubsystem::AiDebugger,
            tier: 2,
            solution_id: "sol-123".to_string(),
            confidence: 0.95,
            src_node: "pod_3".to_string(),
            model: "qwen3-coder-235b".to_string(),
            incident_id: Some("inc-42".to_string()),
        };
        let m = mi_edit_marker(&ctx);
        assert!(m.starts_with("[MI-EDIT v1 sub=ad t=2 c=0.95"));
        assert!(m.contains("s=sol-123"));
        assert!(m.contains("n=pod_3"));
        assert!(m.contains("m=qwen3-coder-235b"));
        assert!(m.contains("i=inc-42"));
    }

    #[test]
    fn marker_handles_no_incident() {
        let ctx = MiEditCtx {
            sub: MiSubsystem::MeshIntelligence,
            tier: 0,
            solution_id: "deterministic".to_string(),
            confidence: 1.0,
            src_node: "server".to_string(),
            model: "deterministic".to_string(),
            incident_id: None,
        };
        let m = mi_edit_marker(&ctx);
        assert!(m.contains("i=none"));
        assert!(m.contains("sub=me t=0 c=1.00"));
    }

    #[test]
    fn subsystem_codes_are_two_letters() {
        for s in [
            MiSubsystem::MeshIntelligence,
            MiSubsystem::RaceEngineer,
            MiSubsystem::PodAgent,
            MiSubsystem::Recovery,
            MiSubsystem::FleetIntelligence,
            MiSubsystem::PolicyEngine,
            MiSubsystem::AiDebugger,
        ] {
            assert_eq!(s.code().len(), 2, "{s:?} code is not 2 letters");
        }
    }

    #[test]
    fn from_code_roundtrip_and_human_is_none() {
        for s in [
            MiSubsystem::MeshIntelligence,
            MiSubsystem::RaceEngineer,
            MiSubsystem::AiDebugger,
        ] {
            assert_eq!(MiSubsystem::from_code(s.code()), Some(s));
        }
        assert_eq!(MiSubsystem::from_code("human"), None);
        assert_eq!(MiSubsystem::from_code(""), None);
        assert_eq!(MiSubsystem::from_code("xx"), None);
    }
}
