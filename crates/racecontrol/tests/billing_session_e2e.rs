//! Phase 414 E2E: stop_billing handler branches on elapsed_seconds.
//! Plan 04 implements; Wave 0 stubs the test surface so cargo test discovers it.

#[tokio::test]
#[ignore = "Phase 414 Plan 04 implements stop_billing branching"]
async fn stop_billing_branches_on_elapsed() {
    // 414-INTEGRATION-04: With DB fixture:
    //   - elapsed_seconds == 0 + status=WaitingForGame -> CancelledNoPlayable + full refund (existing behavior)
    //   - elapsed_seconds > 0 + status=WaitingForGame -> EndedEarly + bill cumulative cost
    // Mirror Phase 314 billing_atomicity integration patterns.
    panic!("Wave 0 stub — Plan 04 implements");
}
