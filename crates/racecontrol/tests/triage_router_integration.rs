//! Integration tests for the MMA Triage Router (Commit 1+2 follow-up).
//!
//! Closes NOT TESTED items (3) 4-verdict output against real DB rows and
//! (4) backtest against the 2026-04-17 Pattern C hallucination, from the
//! Commit 2 NOT TESTED list. Still NOT covered here (require deploy):
//! live HTTP endpoint on server .23, STAFF_TOKEN auth.
//!
//! Strategy: instantiate an in-memory SQLite pool, run fleet_kb::migrate to
//! create audit_known_issues, seed representative rows, then invoke the
//! router's inner composition (match_audit_known_issue_structured +
//! build_verdict) and assert verdict shape.

use racecontrol_crate::api::triage::{
    build_verdict, parse_query, TriageVerdict,
};
use racecontrol_crate::fleet_kb::{self, match_audit_known_issue_structured};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

async fn make_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");
    fleet_kb::migrate(&pool).await.expect("fleet_kb migrate");
    pool
}

async fn seed_inv_9(pool: &SqlitePool) {
    // Mirrors the seeded INV-9 entry from scripts/deploy/bug-tracker-mi-seed.js.
    // Pod 8 excluded per 2026-04-17 observation ("7 pods, all except Pod 8").
    fleet_kb::upsert_audit_known_issue(
        pool,
        "INV-9",
        "INV-9",
        "P2",
        &[
            "inv-9".to_string(),
            "steam dialog".to_string(),
            "dialog visible".to_string(),
        ],
        "Steam popup not dismissed — single-dismiss flag in steam_checks.rs",
        "Removed single-dismiss limit; retry every 2s poll cycle",
        "code_fixed",
        Some("68f4d61e"),
        &[
            "pod_1".to_string(), "pod_2".to_string(), "pod_3".to_string(),
            "pod_4".to_string(), "pod_5".to_string(), "pod_6".to_string(),
            "pod_7".to_string(),
        ],
        "2026-04-17",
        "Pattern C (2026-04-17). Fix shipped in 68f4d61e — verify build version on failing pod.",
    )
    .await
    .expect("seed INV-9");
}

async fn seed_inv_3(pool: &SqlitePool) {
    fleet_kb::upsert_audit_known_issue(
        pool,
        "INV-3",
        "INV-3",
        "P2",
        &[
            "inv-3".to_string(),
            "exit code 1".to_string(),
            "orphan".to_string(),
        ],
        "F1 25 orphan process blocking relaunch — EA AntiCheat respawns",
        "V-2 orphan cleanup adds EA Anti-Cheat + EA Desktop to kill list",
        "code_fixed",
        Some("68f4d61e"),
        &["pod_3".to_string(), "pod_4".to_string(), "pod_8".to_string()],
        "2026-04-16",
        "F1 25 orphan. V-2 deployed; if recurring, verify build.",
    )
    .await
    .expect("seed INV-3");
}

#[tokio::test]
async fn pattern_c_backtest_returns_triage_fast_exact_for_pod_3() {
    // The 2026-04-17 MMA hallucination: query "pod_3 iRacing Steam dialog
    // visible after 60s" should now return TRIAGE_FAST / INV-9 with EXACT hit
    // (Pod 3 is in INV-9's affects_targets).
    let pool = make_pool().await;
    seed_inv_9(&pool).await;

    let query = "pod_3 iRacing Steam dialog visible after 60s";
    let parsed = parse_query(query, None);

    // Sanity — parser should extract Pod 3 and the steam-dialog class.
    assert_eq!(parsed.pod_ids, vec!["pod_3".to_string()]);
    assert!(parsed.symptom_classes.contains(&"steam-dialog".to_string()));

    // Oracle lookup uses the expanded terms, mirroring diagnose_triage.
    let oracle_input = parsed.expanded_terms.join(" ");
    let hit = match_audit_known_issue_structured(&pool, &oracle_input).await;
    assert!(hit.is_some(), "expected oracle hit for steam dialog query");

    let verdict = build_verdict(&parsed, query, Some("ops-triage"), hit);
    match verdict {
        TriageVerdict::TriageFast {
            bug_id,
            hit_type,
            should_run_mma,
            affects_targets,
            fix_status,
            ..
        } => {
            assert_eq!(bug_id, "INV-9");
            assert_eq!(hit_type, "EXACT");
            assert!(!should_run_mma, "ops-triage on oracle hit should NOT run MMA");
            assert!(affects_targets.contains(&"pod_3".to_string()));
            assert_eq!(fix_status, "code_fixed");
        }
        other => panic!("expected TRIAGE_FAST, got {:?}", other),
    }
}

#[tokio::test]
async fn pattern_c_category_match_when_pod_outside_affected() {
    // Pod 8 is intentionally excluded from INV-9 (2026-04-17 observation).
    // A query about Pod 8 Steam dialog should return CATEGORY — the mechanism
    // may apply but the target differs.
    let pool = make_pool().await;
    seed_inv_9(&pool).await;

    let query = "pod_8 steam dialog visible";
    let parsed = parse_query(query, None);
    assert_eq!(parsed.pod_ids, vec!["pod_8".to_string()]);

    let oracle_input = parsed.expanded_terms.join(" ");
    let hit = match_audit_known_issue_structured(&pool, &oracle_input).await;
    let verdict = build_verdict(&parsed, query, Some("ops-triage"), hit);
    match verdict {
        TriageVerdict::TriageFast { hit_type, confidence, .. } => {
            assert_eq!(hit_type, "CATEGORY");
            assert!(confidence < 0.8, "CATEGORY confidence should be penalised");
        }
        other => panic!("expected TRIAGE_FAST (CATEGORY), got {:?}", other),
    }
}

#[tokio::test]
async fn exit_code_1_class_matches_inv_3() {
    // "pod_4 F1 25 exit code 1" → TRIAGE_FAST with INV-3 EXACT.
    let pool = make_pool().await;
    seed_inv_3(&pool).await;

    let query = "pod_4 F1 25 exit code 1";
    let parsed = parse_query(query, None);
    assert!(parsed.symptom_classes.contains(&"exit-code-1".to_string()));

    let oracle_input = parsed.expanded_terms.join(" ");
    let hit = match_audit_known_issue_structured(&pool, &oracle_input).await;
    let verdict = build_verdict(&parsed, query, Some("ops-triage"), hit);
    match verdict {
        TriageVerdict::TriageFast { bug_id, hit_type, .. } => {
            assert_eq!(bug_id, "INV-3");
            assert_eq!(hit_type, "EXACT");
        }
        other => panic!("expected TRIAGE_FAST for exit-code-1, got {:?}", other),
    }
}

#[tokio::test]
async fn audit_deep_when_symptom_class_matches_but_no_seeded_entry() {
    // DB has INV-9 only. A zero-laps query has a recognized symptom class but
    // no matching oracle entry → AUDIT_DEEP, should run MMA with focused bundle.
    let pool = make_pool().await;
    seed_inv_9(&pool).await;

    let query = "pod_2 0 laps recorded since morning";
    let parsed = parse_query(query, None);
    assert!(parsed.symptom_classes.contains(&"zero-laps".to_string()));

    let oracle_input = parsed.expanded_terms.join(" ");
    let hit = match_audit_known_issue_structured(&pool, &oracle_input).await;
    assert!(hit.is_none(), "no INV-* seeded for zero-laps in this test");

    let verdict = build_verdict(&parsed, query, Some("ops-triage"), hit);
    match verdict {
        TriageVerdict::AuditDeep {
            relevant_symptom_classes,
            should_run_mma,
            ..
        } => {
            assert!(relevant_symptom_classes.contains(&"zero-laps".to_string()));
            assert!(should_run_mma);
        }
        other => panic!("expected AUDIT_DEEP, got {:?}", other),
    }
}

#[tokio::test]
async fn research_new_when_nothing_matches() {
    // Query has pod but no recognized symptom class → RESEARCH_NEW.
    let pool = make_pool().await;
    seed_inv_9(&pool).await;

    let query = "pod_5 has been behaving weirdly for the last 45 minutes";
    let parsed = parse_query(query, None);
    assert!(parsed.symptom_classes.is_empty());

    let oracle_input = if parsed.expanded_terms.is_empty() {
        query.to_string()
    } else {
        parsed.expanded_terms.join(" ")
    };
    let hit = match_audit_known_issue_structured(&pool, &oracle_input).await;

    let verdict = build_verdict(&parsed, query, Some("ops-triage"), hit);
    match verdict {
        TriageVerdict::ResearchNew { should_run_mma, .. } => {
            assert!(should_run_mma);
        }
        other => panic!("expected RESEARCH_NEW, got {:?}", other),
    }
}

#[tokio::test]
async fn reject_ambiguous_for_short_gibberish() {
    let pool = make_pool().await;
    seed_inv_9(&pool).await;

    let query = "broken";
    let parsed = parse_query(query, None);

    let oracle_input = if parsed.expanded_terms.is_empty() {
        query.to_string()
    } else {
        parsed.expanded_terms.join(" ")
    };
    let hit = match_audit_known_issue_structured(&pool, &oracle_input).await;

    let verdict = build_verdict(&parsed, query, Some("ops-triage"), hit);
    match verdict {
        TriageVerdict::RejectAmbiguous {
            clarifying_questions,
            should_run_mma,
            ..
        } => {
            assert!(!clarifying_questions.is_empty());
            assert!(!should_run_mma);
        }
        other => panic!("expected REJECT_AMBIGUOUS, got {:?}", other),
    }
}

#[tokio::test]
async fn gsd_debug_caller_forces_mma_on_oracle_hit() {
    let pool = make_pool().await;
    seed_inv_9(&pool).await;

    let query = "pod_3 steam dialog visible";
    let parsed = parse_query(query, None);

    let oracle_input = parsed.expanded_terms.join(" ");
    let hit = match_audit_known_issue_structured(&pool, &oracle_input).await;

    let verdict = build_verdict(&parsed, query, Some("gsd-debug"), hit);
    match verdict {
        TriageVerdict::TriageFast { should_run_mma, hit_type, .. } => {
            assert!(should_run_mma, "gsd-debug must force MMA even on oracle hit");
            assert_eq!(hit_type, "EXACT");
        }
        other => panic!("expected TRIAGE_FAST with should_run_mma=true, got {:?}", other),
    }
}

#[tokio::test]
async fn targets_hint_augments_pod_extraction() {
    let pool = make_pool().await;
    seed_inv_9(&pool).await;

    // Query without explicit pod, but caller passes targets_hint = [pod_3].
    let query = "steam dialog visible";
    let hint = vec!["pod_3".to_string()];
    let parsed = parse_query(query, Some(&hint));
    assert_eq!(parsed.pod_ids, vec!["pod_3".to_string()]);

    let oracle_input = parsed.expanded_terms.join(" ");
    let hit = match_audit_known_issue_structured(&pool, &oracle_input).await;
    let verdict = build_verdict(&parsed, query, Some("ops-triage"), hit);
    match verdict {
        TriageVerdict::TriageFast { hit_type, affects_targets, .. } => {
            assert_eq!(hit_type, "EXACT");
            assert!(affects_targets.contains(&"pod_3".to_string()));
        }
        other => panic!("expected TRIAGE_FAST (EXACT) via targets_hint, got {:?}", other),
    }
}
