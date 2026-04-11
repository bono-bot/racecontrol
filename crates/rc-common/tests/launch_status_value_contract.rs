// Phase 368 Plan 01 — Phase 62 enum-value contract tests.
//
// PURPOSE: These tests assert the EXACT JSON string values produced by serde for
// LaunchState and LaunchOrigin. The kiosk TypeScript literal union in Plan 04 MUST
// match these exact strings. Any variant rename will fail here first — making the
// breakage visible at compile/test time, not at runtime in the customer UI.
//
// Reference: Pod 8 pitlane incident 2026-04-08 — cross-boundary string enum
// contracts were value-tested, not just field-presence tested, which is what
// would have caught the original Phase 62 failure mode.

use rc_common::protocol::{LaunchOrigin, LaunchState};

/// Asserts all 5 LaunchState variants serialize to their exact snake_case string values.
/// Plan 04 TypeScript must have: type LaunchStateValue = "launch_started" | "ai_analysis_requested" | ...
#[test]
fn launch_status_value_contract() {
    // LaunchState — all 5 variants
    assert_eq!(
        serde_json::to_string(&LaunchState::LaunchStarted).expect("LaunchStarted serialize"),
        "\"launch_started\"",
        "LaunchState::LaunchStarted must serialize to exactly \"launch_started\""
    );
    assert_eq!(
        serde_json::to_string(&LaunchState::AiAnalysisRequested)
            .expect("AiAnalysisRequested serialize"),
        "\"ai_analysis_requested\"",
        "LaunchState::AiAnalysisRequested must serialize to exactly \"ai_analysis_requested\""
    );
    assert_eq!(
        serde_json::to_string(&LaunchState::IssueBeingFixed).expect("IssueBeingFixed serialize"),
        "\"issue_being_fixed\"",
        "LaunchState::IssueBeingFixed must serialize to exactly \"issue_being_fixed\""
    );
    assert_eq!(
        serde_json::to_string(&LaunchState::IssueFixed).expect("IssueFixed serialize"),
        "\"issue_fixed\"",
        "LaunchState::IssueFixed must serialize to exactly \"issue_fixed\""
    );
    assert_eq!(
        serde_json::to_string(&LaunchState::NeedsManualIntervention)
            .expect("NeedsManualIntervention serialize"),
        "\"needs_manual_intervention\"",
        "LaunchState::NeedsManualIntervention must serialize to exactly \"needs_manual_intervention\""
    );

    // LaunchOrigin — all 4 variants
    assert_eq!(
        serde_json::to_string(&LaunchOrigin::Customer).expect("Customer serialize"),
        "\"customer\"",
        "LaunchOrigin::Customer must serialize to exactly \"customer\""
    );
    assert_eq!(
        serde_json::to_string(&LaunchOrigin::Staff).expect("Staff serialize"),
        "\"staff\"",
        "LaunchOrigin::Staff must serialize to exactly \"staff\""
    );
    assert_eq!(
        serde_json::to_string(&LaunchOrigin::AutoToken).expect("AutoToken serialize"),
        "\"auto_token\"",
        "LaunchOrigin::AutoToken must serialize to exactly \"auto_token\""
    );
    assert_eq!(
        serde_json::to_string(&LaunchOrigin::Retry).expect("Retry serialize"),
        "\"retry\"",
        "LaunchOrigin::Retry must serialize to exactly \"retry\""
    );
}
