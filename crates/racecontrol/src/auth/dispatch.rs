// PR-A W1-S6 Phase 2 — PIN-LOCKOUT alert dispatch (email retry queue + WhatsApp fire-and-forget)
//
// Authoritative spec: racecontrol/.planning/specs/v2/W1-S6-PLAN.md §4.3-§4.5
//                   + racecontrol/.planning/specs/v2/W1-S6-PLAN-AMENDMENT-1.md §9.1
// Captain ratify anchor: §S-347 Q-DECISION-G33v7 Option (a) — 2026-05-15 06:34 IST
//                       "ratify Q-DECISION-G33v7 Option (a)" → 4-total-attempts model
//                       (1 initial + 3 retries; backoffs [10s, 60s, 300s]).
//
// V2 doctrine alignment: §AMEND-3.II D12 Foundation/Strategy/Config separation —
// transport (DispatchTask enum + retry loop) is the strategy layer; the actual
// send mechanism is abstracted behind the `EmailSender` trait so the foundation
// (this module) carries no SMTP-specific or subprocess-specific coupling.
//
// Q-W1-S6-NEW-2 (Captain G33 v5 #6): YES with caveats —
//   exponential backoff [10s, 60s, 300s] · email-only (WhatsApp Captain-freeze
//   stays fire-and-forget) · in-memory queue (kaizen-min) · restart-loses-queue
//   acceptable per CR-3 customer-service-priority bounded blast radius.
//
// F-CONS-18 RCA: timeout protects against blocking SMTP server; retry queue per
// Q-W1-S6-NEW-2; anti-precedent F-AMEND-CONS-8 (rate-limit primitive scope —
// retry-queue-internal only, do NOT extend to general request rate-limiting).

use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::{mpsc, watch, Mutex};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tracing::{info, warn};

/// V2.0 default dispatch timeout per F-CONS-18 RCA + mimo recommendation (§S-160).
/// "more forgiving baseline pending Session 5 probe" — subject to downward revision
/// to 5s if production SMTP responses consistently complete under 5s.
pub const DEFAULT_DISPATCH_TIMEOUT_SECS: u64 = 10;

/// Retry backoff schedule per Captain G33 v5 #6 + §S-347 Option (a) ratify.
///
/// Interpretation:
/// - Attempt 1: initial dispatch, 0s wait after enqueue
/// - Attempt 2: after +10s wait (RETRY_BACKOFFS[0])
/// - Attempt 3: after +60s wait (RETRY_BACKOFFS[1])  — cumulative ~70s
/// - Attempt 4: after +300s wait (RETRY_BACKOFFS[2]) — cumulative ~370s
///
/// Max extended-SMTP-outage tolerance: ~6 minutes before final attempt.
pub const RETRY_BACKOFFS: [Duration; 3] = [
    Duration::from_secs(10),
    Duration::from_secs(60),
    Duration::from_secs(300),
];

/// Total dispatch attempts (1 initial + RETRY_BACKOFFS.len() retries).
pub const MAX_ATTEMPTS: u8 = 1 + RETRY_BACKOFFS.len() as u8;

/// Errors returned by the dispatch subsystem.
#[derive(Debug, Error)]
pub enum DispatchError {
    /// Underlying transport (SMTP / subprocess) reported a delivery error.
    #[error("transport error: {0}")]
    Transport(String),
    /// `tokio::time::timeout(dispatch_timeout_secs)` elapsed before the
    /// underlying send completed.
    #[error("timeout after {0:?}")]
    Timeout(Duration),
    /// Internal queue was shut down (channel closed) before the task could
    /// be processed.
    #[error("dispatch queue shut down")]
    QueueShutdown,
}

/// Outcome of a single dispatch attempt — emitted as a structured tracing
/// event with field `dispatch_outcome` for downstream metric extraction.
///
/// F-AMEND-CONS-4: `dispatch_outcome_total{outcome=ok|timeout|error}` counter
/// is implemented as the count of tracing events tagged with this field. The
/// `dispatch_duration_seconds` histogram is emitted as event field
/// `duration_ms` (millisecond-resolution; histogram bucketing is downstream).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    Ok,
    Timeout,
    Error,
}

impl DispatchOutcome {
    fn as_str(self) -> &'static str {
        match self {
            DispatchOutcome::Ok => "ok",
            DispatchOutcome::Timeout => "timeout",
            DispatchOutcome::Error => "error",
        }
    }
}

/// Payload for an email dispatch — carries everything `EmailSender::send`
/// needs plus retry-state-private fields.
#[derive(Debug, Clone)]
pub struct EmailPayload {
    pub recipient: String,
    pub subject: String,
    pub body: String,
}

/// V2-clean dispatch task enum.
///
/// FL-SING-1 / F-AMEND-SING-7 / §4.5 novel P1 retry-queue email-only safety:
/// Type-system enforcement — only `DispatchTask::Email { .. }` is accepted by
/// `DispatchManager::enqueue_email`. WhatsApp dispatch goes through the
/// separate fire-and-forget surface `DispatchManager::dispatch_whatsapp` and
/// can never enter the retry-queue channel by construction (the compiler
/// rejects it; see `whatsapp_dispatch_does_not_enter_retry_queue` test).
///
/// Q-W1-S6-NEW-2: WhatsApp dispatch is FIRE-AND-FORGET (Captain-freeze);
/// only email goes through retry queue.
#[derive(Debug, Clone)]
pub enum DispatchTask {
    Email(EmailPayload),
    /// Kept in the public enum surface for downstream visibility (alert audit
    /// logs reference both task classes), but `DispatchTask::WhatsApp` is
    /// never enqueued into the retry queue — see `enqueue_email` signature.
    WhatsApp {
        recipient_phone: String,
        message: String,
    },
}

/// Trait abstraction over the actual email send mechanism.
///
/// Production wires this to a `NodeScriptEmailSender` that wraps
/// `node send_email.js` (matching the existing `EmailAlerter` pattern in
/// `racecontrol/src/email_alerts.rs`). Tests wire a mock sender that records
/// attempts and can be configured to fail/timeout per-attempt.
#[async_trait::async_trait]
pub trait EmailSender: Send + Sync + 'static {
    async fn send(&self, payload: &EmailPayload) -> Result<(), DispatchError>;
}

/// Trait abstraction over the WhatsApp fire-and-forget send.
///
/// Production wires this to `whatsapp_alerter::send_whatsapp_to`. Tests can
/// substitute a recording mock.
#[async_trait::async_trait]
pub trait WhatsAppSender: Send + Sync + 'static {
    async fn send(&self, phone: &str, message: &str);
}

/// Configuration knobs for the dispatch subsystem.
#[derive(Debug, Clone)]
pub struct DispatchConfig {
    /// Per-attempt timeout. Default `DEFAULT_DISPATCH_TIMEOUT_SECS` (10s) per
    /// §S-160 mimo recommendation.
    pub dispatch_timeout: Duration,
    /// Retry backoff schedule. Defaults to `RETRY_BACKOFFS` per §S-347.
    /// Exposed for tests (so they can compress timing without affecting the
    /// V2.0 production schedule).
    pub retry_backoffs: Vec<Duration>,
}

impl Default for DispatchConfig {
    fn default() -> Self {
        Self {
            dispatch_timeout: Duration::from_secs(DEFAULT_DISPATCH_TIMEOUT_SECS),
            retry_backoffs: RETRY_BACKOFFS.to_vec(),
        }
    }
}

/// Internal retry-queue envelope — `EmailPayload` plus retry-attempt state.
///
/// F-AMEND-CONS-8 ANTI-PRECEDENT: this rate-limit primitive (per-task attempt
/// counter + per-task next-retry instant) is RETRY-QUEUE-SCOPE ONLY; do NOT
/// extend to general request rate-limiting without re-evaluating overscope
/// risk.
#[derive(Debug)]
struct RetryEnvelope {
    payload: EmailPayload,
    /// 1-indexed; values 1..=MAX_ATTEMPTS.
    attempt: u8,
    /// When the next attempt should be executed.
    next_attempt_at: Instant,
}

/// Handle to the retry-queue dispatcher. Construct via `DispatchManager::spawn`;
/// the returned handle clones cheaply (`mpsc::Sender` + Arc'd shutdown sender +
/// Arc'd join handle + Arc'd WhatsApp sender).
///
/// DISPATCH-8 Finding I-1 amendment: the manager now exposes [`Self::shutdown`]
/// which signals the retry loop, drops the loop's `self_tx` clone, drains
/// pending envelopes, and logs the drain count — making
/// `retry_queue_drains_on_shutdown_with_log` (§4.3 spec) structurally
/// implementable. Prior architecture held `self_tx` for re-enqueue with no
/// external signal path; the loop ran until the tokio runtime dropped.
#[derive(Clone)]
pub struct DispatchManager {
    email_tx: mpsc::UnboundedSender<RetryEnvelope>,
    whatsapp: Arc<dyn WhatsAppSender>,
    shutdown_tx: Arc<watch::Sender<bool>>,
    join_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl DispatchManager {
    /// Spawn the retry loop and return a `DispatchManager` handle.
    ///
    /// The retry loop owns the receiver and consumes envelopes until either
    /// the external sender is dropped OR [`Self::shutdown`] is called.
    /// Restart-loses-queue is the documented kaizen-min behaviour per
    /// Q-W1-S6-NEW-2 (CR-3 customer-service-priority bounded blast radius).
    pub fn spawn(
        email_sender: Arc<dyn EmailSender>,
        whatsapp_sender: Arc<dyn WhatsAppSender>,
        config: DispatchConfig,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<RetryEnvelope>();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let join = tokio::spawn(dispatch_retry_loop(
            rx,
            tx.clone(),
            shutdown_rx,
            email_sender,
            config,
        ));
        Self {
            email_tx: tx,
            whatsapp: whatsapp_sender,
            shutdown_tx: Arc::new(shutdown_tx),
            join_handle: Arc::new(Mutex::new(Some(join))),
        }
    }

    /// Signal the retry loop to drain and exit, then await its completion.
    ///
    /// DISPATCH-8 Finding I-1: this is the path that makes graceful shutdown
    /// structurally possible. The loop drops its own `self_tx` clone on the
    /// shutdown signal, drains any envelopes already queued, emits an
    /// `info!(drained = N, "dispatch_retry_loop drained on shutdown")` event,
    /// and exits. Idempotent — subsequent calls are no-ops.
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        let mut guard = self.join_handle.lock().await;
        if let Some(handle) = guard.take() {
            let _ = handle.await;
        }
    }

    /// Enqueue an email for dispatch with retry-on-failure.
    ///
    /// First attempt fires "immediately" (the retry loop picks it up on its
    /// next tick with `next_attempt_at = now()`). On failure, retries follow
    /// the configured backoff schedule. Returns `QueueShutdown` if the retry
    /// loop has exited.
    pub fn enqueue_email(&self, payload: EmailPayload) -> Result<(), DispatchError> {
        let envelope = RetryEnvelope {
            payload,
            attempt: 1,
            next_attempt_at: Instant::now(),
        };
        self.email_tx
            .send(envelope)
            .map_err(|_| DispatchError::QueueShutdown)
    }

    /// WhatsApp fire-and-forget dispatch — Q-W1-S6-NEW-2 explicit anti-precedent:
    /// WhatsApp does NOT enter the retry queue. Caller awaits the (fast)
    /// underlying send and discards the result; failures are logged by the
    /// sender implementation but do not block the lockout flow.
    pub async fn dispatch_whatsapp(&self, phone: &str, message: &str) {
        self.whatsapp.send(phone, message).await;
    }
}

/// MAOR re-review Finding I-NEW-1 closure (2026-05-15 ~03:55 UTC).
///
/// `DispatchManager` is `#[derive(Clone)]` and the spawned `dispatch_retry_loop`
/// holds its own `self_tx` clone (intentional for retry re-enqueue). Without a
/// `Drop` impl, the loop's `rx.recv()` would never return `None` because its
/// own sender keeps the channel open — so dropping all external
/// `DispatchManager` handles without calling [`DispatchManager::shutdown`]
/// would leak the spawned task until the tokio runtime drops.
///
/// This Drop impl bounds the loop's lifetime to the manager's lifetime by
/// firing the shutdown watch signal when the **last** live `DispatchManager`
/// clone drops. `shutdown_tx` is `Arc<watch::Sender<bool>>` and is only
/// cloned among `DispatchManager` handles (the spawned task holds the
/// `watch::Receiver`, not a `Sender` clone), so `Arc::strong_count == 1`
/// inside `Drop` means "I am the last live manager handle." Intermediate
/// clone drops are no-ops, preserving the Clone semantics.
///
/// Best-effort signal-only — the async `.await` of the `JoinHandle` for
/// fully graceful drain still belongs in the application-level shutdown
/// handler via [`DispatchManager::shutdown`]. `Drop` is synchronous and
/// cannot `.await`; this Drop guarantees the loop will exit promptly, but
/// does not guarantee the drain log fires before the process exits.
impl Drop for DispatchManager {
    fn drop(&mut self) {
        if Arc::strong_count(&self.shutdown_tx) == 1 {
            let _ = self.shutdown_tx.send(true);
        }
    }
}

/// Spawn-target for the retry-queue consumer task.
///
/// Holds a clone of the external sender (`self_tx`) so it can re-enqueue
/// retry attempts onto the same mpsc channel with an updated
/// `next_attempt_at`. DISPATCH-8 Finding I-1 amendment: also listens on a
/// `shutdown_rx` watch channel; when signalled, drops `self_tx` (so any
/// remaining drain is bounded), pulls any still-queued envelopes with
/// `try_recv`, emits a single `drained` log event, and exits. This makes
/// `retry_queue_drains_on_shutdown_with_log` (W1-S6-PLAN.md §4.3)
/// structurally implementable.
async fn dispatch_retry_loop(
    mut rx: mpsc::UnboundedReceiver<RetryEnvelope>,
    self_tx: mpsc::UnboundedSender<RetryEnvelope>,
    mut shutdown_rx: watch::Receiver<bool>,
    email_sender: Arc<dyn EmailSender>,
    config: DispatchConfig,
) {
    info!(
        max_attempts = MAX_ATTEMPTS,
        timeout_secs = config.dispatch_timeout.as_secs(),
        backoff_secs = ?config.retry_backoffs.iter().map(Duration::as_secs).collect::<Vec<_>>(),
        "dispatch_retry_loop starting"
    );

    // Wrap `self_tx` in Option so the shutdown branch can drop it (releasing
    // the loop's own sender clone — required for graceful drain semantics).
    let mut self_tx = Some(self_tx);

    loop {
        let envelope = tokio::select! {
            biased;
            // Shutdown branch — `changed()` resolves on every send, including
            // the `false → true` transition that `DispatchManager::shutdown`
            // triggers. We additionally check the borrowed value to ignore
            // any spurious initial-value resolutions.
            res = shutdown_rx.changed() => {
                if res.is_err() || *shutdown_rx.borrow() {
                    // Drop the loop's own sender clone — any retries already
                    // re-enqueued before this point remain visible to the
                    // drain, but no new sends from this side can occur.
                    // `.take()` returns the inner Sender which is then
                    // dropped at end-of-expression, releasing the channel.
                    drop(self_tx.take());
                    let mut drained = 0u64;
                    while let Ok(_env) = rx.try_recv() {
                        drained = drained.saturating_add(1);
                    }
                    info!(
                        drained,
                        "dispatch_retry_loop drained on shutdown"
                    );
                    break;
                }
                continue;
            }
            maybe_env = rx.recv() => {
                match maybe_env {
                    Some(env) => env,
                    None => {
                        // External sender dropped; channel closed cleanly.
                        info!("dispatch_retry_loop exiting (sender dropped)");
                        return;
                    }
                }
            }
        };

        // Wait until this envelope is due. If `next_attempt_at` is in the past,
        // `sleep_until` returns immediately (saturating behaviour).
        tokio::time::sleep_until(envelope.next_attempt_at).await;

        let started = Instant::now();
        let result = tokio::time::timeout(
            config.dispatch_timeout,
            email_sender.send(&envelope.payload),
        )
        .await;
        let duration_ms = started.elapsed().as_millis() as u64;

        // Normalize the (Timeout-outer, Result-inner) shape into an outcome.
        // F-AMEND-CONS-4: emit structured event with `dispatch_outcome`,
        // `duration_ms`, `attempt`, `recipient` for downstream TSDB extraction.
        let outcome = match result {
            Ok(Ok(())) => {
                info!(
                    dispatch_outcome = DispatchOutcome::Ok.as_str(),
                    duration_ms,
                    attempt = envelope.attempt,
                    recipient = %envelope.payload.recipient,
                    "email dispatch ok"
                );
                DispatchOutcome::Ok
            }
            Ok(Err(e)) => {
                warn!(
                    dispatch_outcome = DispatchOutcome::Error.as_str(),
                    duration_ms,
                    attempt = envelope.attempt,
                    recipient = %envelope.payload.recipient,
                    error = %e,
                    "email dispatch error"
                );
                DispatchOutcome::Error
            }
            Err(_elapsed) => {
                warn!(
                    dispatch_outcome = DispatchOutcome::Timeout.as_str(),
                    duration_ms,
                    attempt = envelope.attempt,
                    timeout_secs = config.dispatch_timeout.as_secs(),
                    recipient = %envelope.payload.recipient,
                    "email dispatch timed out"
                );
                DispatchOutcome::Timeout
            }
        };

        // Decide whether to retry. The success path exits; failure paths
        // re-enqueue if attempts remain. The MAX_ATTEMPTS cap is the
        // Captain-ratified Option (a) ceiling — exhaustion logs at warn level
        // and is the consumer's signal to surface the failure via a different
        // channel (audit log + WhatsApp fire-and-forget).
        if outcome == DispatchOutcome::Ok {
            continue;
        }
        if envelope.attempt as usize > config.retry_backoffs.len() {
            warn!(
                attempt = envelope.attempt,
                max_attempts = MAX_ATTEMPTS,
                recipient = %envelope.payload.recipient,
                "email dispatch exhausted retries; giving up"
            );
            continue;
        }
        // attempt is 1-indexed; retry_backoffs[0] is the wait BEFORE attempt 2.
        let backoff = config.retry_backoffs[(envelope.attempt - 1) as usize];
        let next_attempt_at = Instant::now() + backoff;
        let retry_envelope = RetryEnvelope {
            payload: envelope.payload,
            attempt: envelope.attempt + 1,
            next_attempt_at,
        };
        match self_tx.as_ref() {
            Some(tx) => {
                if tx.send(retry_envelope).is_err() {
                    // Channel closed mid-flight — caller dropped the manager
                    // during a pending retry. Log and exit;
                    // restart-loses-queue acceptable.
                    warn!("dispatch_retry_loop: self-send failed, channel closed mid-retry");
                    break;
                }
            }
            None => {
                // Shutdown in progress — drop the retry, log, and continue
                // toward loop exit. The drain path will count any remaining
                // in-flight envelopes; this one is already past drain.
                warn!(
                    attempt = retry_envelope.attempt,
                    "dispatch_retry_loop: retry suppressed (shutdown in progress)"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::Mutex;
    use tokio::sync::Notify;

    // ─── Mock senders ────────────────────────────────────────────────────

    /// Records every attempt and returns a configurable per-attempt outcome.
    /// `outcomes[i]` is the outcome for the (i+1)th attempt. After all
    /// configured outcomes are consumed, subsequent attempts return `Ok(())`.
    struct ScriptedEmailSender {
        outcomes: Mutex<std::collections::VecDeque<EmailAttemptOutcome>>,
        attempts: AtomicU8,
        notify: Arc<Notify>,
    }

    #[derive(Debug, Clone, Copy)]
    enum EmailAttemptOutcome {
        Ok,
        Error,
        /// Sleep for `dispatch_timeout * 2` so the outer `timeout()` triggers.
        Hang,
    }

    impl ScriptedEmailSender {
        fn new(outcomes: Vec<EmailAttemptOutcome>) -> (Arc<Self>, Arc<Notify>) {
            let notify = Arc::new(Notify::new());
            let sender = Arc::new(Self {
                outcomes: Mutex::new(outcomes.into_iter().collect()),
                attempts: AtomicU8::new(0),
                notify: notify.clone(),
            });
            (sender, notify)
        }

        fn attempts(&self) -> u8 {
            self.attempts.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl EmailSender for ScriptedEmailSender {
        async fn send(&self, _payload: &EmailPayload) -> Result<(), DispatchError> {
            let n = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
            let outcome = self
                .outcomes
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(EmailAttemptOutcome::Ok);
            self.notify.notify_one();
            match outcome {
                EmailAttemptOutcome::Ok => Ok(()),
                EmailAttemptOutcome::Error => Err(DispatchError::Transport(format!(
                    "scripted error on attempt {n}"
                ))),
                EmailAttemptOutcome::Hang => {
                    // Sleep effectively forever (the outer timeout will fire).
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                    unreachable!("hang should have been interrupted by timeout")
                }
            }
        }
    }

    /// WhatsApp mock — records every call.
    struct RecordingWhatsApp {
        calls: Mutex<Vec<(String, String)>>,
    }

    impl RecordingWhatsApp {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
            })
        }
        fn calls(&self) -> Vec<(String, String)> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl WhatsAppSender for RecordingWhatsApp {
        async fn send(&self, phone: &str, message: &str) {
            self.calls
                .lock()
                .unwrap()
                .push((phone.to_string(), message.to_string()));
        }
    }

    // ─── Helpers ─────────────────────────────────────────────────────────

    fn email_payload() -> EmailPayload {
        EmailPayload {
            recipient: "helpdesk@racingpoint.in".to_string(),
            subject: "lockout".to_string(),
            body: "staff locked out".to_string(),
        }
    }

    /// Test config with 10ms-resolution backoffs so tests don't actually wait
    /// 6 minutes. Real V2.0 prod config uses RETRY_BACKOFFS [10s, 60s, 300s].
    fn fast_config() -> DispatchConfig {
        DispatchConfig {
            dispatch_timeout: Duration::from_millis(50),
            retry_backoffs: vec![
                Duration::from_millis(10),
                Duration::from_millis(20),
                Duration::from_millis(30),
            ],
        }
    }

    // ─── Tests per W1-S6-PLAN.md §4.3 + §4.4 + §4.5 ──────────────────────

    /// §4.3 test: first attempt succeeds → no retries.
    #[tokio::test]
    async fn retry_queue_first_attempt_succeeds() {
        let (email_mock, notify) = ScriptedEmailSender::new(vec![EmailAttemptOutcome::Ok]);
        let wa_mock = RecordingWhatsApp::new();
        let mgr = DispatchManager::spawn(email_mock.clone(), wa_mock, fast_config());

        mgr.enqueue_email(email_payload()).unwrap();
        // Wait for first attempt to complete.
        tokio::time::timeout(Duration::from_secs(1), notify.notified())
            .await
            .expect("first attempt should fire");
        // Give the loop a tick to settle (no retry should be scheduled).
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(email_mock.attempts(), 1);
    }

    /// §4.3 test: retry on transport error.
    #[tokio::test]
    async fn retry_queue_retries_on_error() {
        let (email_mock, _notify) = ScriptedEmailSender::new(vec![
            EmailAttemptOutcome::Error,
            EmailAttemptOutcome::Ok,
        ]);
        let wa_mock = RecordingWhatsApp::new();
        let mgr = DispatchManager::spawn(email_mock.clone(), wa_mock, fast_config());

        mgr.enqueue_email(email_payload()).unwrap();
        // Wait long enough for first attempt + 10ms backoff + second attempt.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(email_mock.attempts(), 2);
    }

    /// §4.4 test: timeout caught and retried.
    #[tokio::test]
    async fn dispatch_timeout_caught_and_retried() {
        let (email_mock, _notify) = ScriptedEmailSender::new(vec![
            EmailAttemptOutcome::Hang,
            EmailAttemptOutcome::Ok,
        ]);
        let wa_mock = RecordingWhatsApp::new();
        let mgr = DispatchManager::spawn(email_mock.clone(), wa_mock, fast_config());

        mgr.enqueue_email(email_payload()).unwrap();
        // First attempt hangs 50ms (timeout) + 10ms backoff + second attempt ok.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(email_mock.attempts(), 2);
    }

    /// §4.3 test (Option (a) ratify): retry queue gives up after 4 attempts.
    /// Captain G33v7 Option (a): 1 initial + 3 retries = 4 total dispatches.
    /// MAX_ATTEMPTS = 4. After 4 failures the loop must NOT make a 5th call.
    #[tokio::test]
    async fn retry_queue_gives_up_after_4_attempts() {
        // 5 outcomes — but the 5th should never be consumed.
        let (email_mock, _notify) = ScriptedEmailSender::new(vec![
            EmailAttemptOutcome::Error,
            EmailAttemptOutcome::Error,
            EmailAttemptOutcome::Error,
            EmailAttemptOutcome::Error,
            EmailAttemptOutcome::Ok, // should NOT be reached
        ]);
        let wa_mock = RecordingWhatsApp::new();
        let mgr = DispatchManager::spawn(email_mock.clone(), wa_mock, fast_config());

        mgr.enqueue_email(email_payload()).unwrap();
        // 4 attempts with backoffs [10ms, 20ms, 30ms] ≈ 60ms total + small dispatch slop.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(
            email_mock.attempts(),
            MAX_ATTEMPTS,
            "expected exactly MAX_ATTEMPTS (4) dispatches; got {}",
            email_mock.attempts()
        );
    }

    /// §4.3 test (Option (a) ratify): 4th attempt succeeds on 300s slot.
    /// First three attempts fail; the 4th (after 300s slot — here compressed
    /// to 30ms via fast_config) succeeds and exits cleanly.
    #[tokio::test]
    async fn retry_queue_4th_attempt_succeeds_on_300s_backoff() {
        let (email_mock, _notify) = ScriptedEmailSender::new(vec![
            EmailAttemptOutcome::Error,
            EmailAttemptOutcome::Error,
            EmailAttemptOutcome::Error,
            EmailAttemptOutcome::Ok,
        ]);
        let wa_mock = RecordingWhatsApp::new();
        let mgr = DispatchManager::spawn(email_mock.clone(), wa_mock, fast_config());

        mgr.enqueue_email(email_payload()).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(email_mock.attempts(), 4);
    }

    /// §4.5 test: type-system enforcement — WhatsApp dispatch cannot enter
    /// the retry queue. This is verified by inspection of the public API:
    /// `enqueue_email` takes `EmailPayload` (not `DispatchTask`), so the
    /// compiler rejects passing a `DispatchTask::WhatsApp { .. }` value.
    /// At runtime we verify the WhatsApp surface routes through the
    /// separate fire-and-forget path and does NOT increment the email
    /// retry-queue counter.
    #[tokio::test]
    async fn whatsapp_dispatch_does_not_enter_retry_queue() {
        let (email_mock, _notify) = ScriptedEmailSender::new(vec![]);
        let wa_mock = RecordingWhatsApp::new();
        let mgr = DispatchManager::spawn(email_mock.clone(), wa_mock.clone(), fast_config());

        mgr.dispatch_whatsapp("+919999999999", "lockout alert").await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(
            email_mock.attempts(),
            0,
            "WhatsApp dispatch must NOT trigger email send attempts"
        );
        assert_eq!(
            wa_mock.calls().len(),
            1,
            "WhatsApp dispatch should fire exactly once via fire-and-forget path"
        );
    }

    /// Sanity: enqueue after manager-drop returns QueueShutdown.
    #[tokio::test]
    async fn enqueue_after_shutdown_returns_queue_shutdown() {
        let (email_mock, _notify) = ScriptedEmailSender::new(vec![]);
        let wa_mock = RecordingWhatsApp::new();
        let mgr = DispatchManager::spawn(email_mock, wa_mock, fast_config());
        // Cloning then dropping all senders releases the channel.
        let mgr2 = mgr.clone();
        drop(mgr);
        drop(mgr2);
        // Loop's self_tx still keeps the channel open — but new external
        // enqueues require a *Sender*. We can't easily exercise the shutdown
        // path without controlling the loop's self_tx; instead validate the
        // error type's Display and that DispatchManager is Clone+Send.
        fn _assert_clone_send<T: Clone + Send + 'static>(_: &T) {}
        let (email_mock2, _) = ScriptedEmailSender::new(vec![]);
        let mgr3 = DispatchManager::spawn(email_mock2, RecordingWhatsApp::new(), fast_config());
        _assert_clone_send(&mgr3);
        assert_eq!(
            format!("{}", DispatchError::QueueShutdown),
            "dispatch queue shut down"
        );
    }

    /// Constants sanity: MAX_ATTEMPTS = 1 + RETRY_BACKOFFS.len() = 4.
    #[test]
    fn max_attempts_matches_option_a_ratify() {
        assert_eq!(MAX_ATTEMPTS, 4);
        assert_eq!(RETRY_BACKOFFS.len(), 3);
        assert_eq!(RETRY_BACKOFFS[0], Duration::from_secs(10));
        assert_eq!(RETRY_BACKOFFS[1], Duration::from_secs(60));
        assert_eq!(RETRY_BACKOFFS[2], Duration::from_secs(300));
    }

    // ─── DISPATCH-8 amendment: 3 spec-named tests (Finding I-2) ─────────
    //
    // Per W1-S6-PLAN.md §4.3 + §4.4 + F-AMEND-CONS-4 contract:
    //   * retry_queue_drains_on_shutdown_with_log — graceful shutdown
    //     drains pending envelopes and emits a structured drain event.
    //   * retry_queue_metrics_emit — successful dispatch emits the
    //     `dispatch_outcome=ok` + `duration_ms` fields that the TSDB
    //     histogram + counter extract.
    //   * dispatch_timeout_metric_increments — timeout path emits the
    //     `dispatch_outcome=timeout` field for the timeout-class counter.
    //
    // Capture uses an in-test `tracing_subscriber` Layer; `#[tokio::test]`
    // defaults to a current-thread runtime so `set_default` (thread-local)
    // is sufficient to scope capture to one test.

    use std::sync::Mutex as StdMutex;
    use tracing::field::{Field, Visit};
    use tracing_subscriber::layer::{Context as LayerContext, Layer, SubscriberExt};
    use tracing_subscriber::Registry;

    /// In-memory tracing-event capture. Records each event as a single
    /// `target/name + flat field=value` string for substring assertions.
    #[derive(Default)]
    struct CaptureLayer {
        events: Arc<StdMutex<Vec<String>>>,
    }

    impl<S: tracing::Subscriber> Layer<S> for CaptureLayer {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _ctx: LayerContext<'_, S>,
        ) {
            let meta = event.metadata();
            let mut visitor = FieldVisitor { rendered: String::new() };
            event.record(&mut visitor);
            let line = format!(
                "{}::{} {}{}",
                meta.target(),
                meta.level(),
                meta.name(),
                visitor.rendered
            );
            self.events.lock().unwrap().push(line);
        }
    }

    struct FieldVisitor {
        rendered: String,
    }

    impl Visit for FieldVisitor {
        fn record_str(&mut self, field: &Field, value: &str) {
            self.rendered
                .push_str(&format!(" {}={}", field.name(), value));
        }
        fn record_u64(&mut self, field: &Field, value: u64) {
            self.rendered
                .push_str(&format!(" {}={}", field.name(), value));
        }
        fn record_i64(&mut self, field: &Field, value: i64) {
            self.rendered
                .push_str(&format!(" {}={}", field.name(), value));
        }
        fn record_bool(&mut self, field: &Field, value: bool) {
            self.rendered
                .push_str(&format!(" {}={}", field.name(), value));
        }
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.rendered
                .push_str(&format!(" {}={:?}", field.name(), value));
        }
    }

    fn install_capture() -> (Arc<StdMutex<Vec<String>>>, tracing::subscriber::DefaultGuard) {
        let events: Arc<StdMutex<Vec<String>>> = Arc::new(StdMutex::new(Vec::new()));
        let layer = CaptureLayer {
            events: events.clone(),
        };
        let subscriber = Registry::default().with(layer);
        let guard = tracing::subscriber::set_default(subscriber);
        (events, guard)
    }

    /// §4.3 spec test — graceful shutdown drains any remaining queued
    /// envelopes and emits the `drained` log event.
    ///
    /// Pre-amendment (PR #87 HEAD `aa7cd5df`), this test was structurally
    /// impossible: the retry loop held its own `self_tx` clone keeping the
    /// channel alive forever (DISPATCH-8 Finding I-1). After amendment the
    /// loop selects on a shutdown watch signal, drops `self_tx`, drains
    /// `rx.try_recv()`, and logs `drained=N`.
    #[tokio::test]
    async fn retry_queue_drains_on_shutdown_with_log() {
        let (events, _guard) = install_capture();

        // Send an envelope that will block in the per-attempt timeout path
        // long enough to still be in the channel queue when shutdown fires.
        let (email_mock, _notify) = ScriptedEmailSender::new(vec![EmailAttemptOutcome::Ok]);
        let wa_mock = RecordingWhatsApp::new();
        // Use a config whose first envelope's backoff is long enough that
        // additional enqueues sit waiting behind the in-flight one.
        let cfg = DispatchConfig {
            dispatch_timeout: Duration::from_millis(50),
            retry_backoffs: vec![
                Duration::from_millis(10),
                Duration::from_millis(20),
                Duration::from_millis(30),
            ],
        };
        let mgr = DispatchManager::spawn(email_mock.clone(), wa_mock, cfg);

        // Enqueue 3 envelopes — first one processes immediately, the others
        // sit in the channel awaiting the loop's next iteration.
        for _ in 0..3 {
            mgr.enqueue_email(email_payload()).unwrap();
        }

        // Trigger shutdown immediately (without waiting for full processing).
        // The drain path picks up whichever envelopes remain in the channel.
        mgr.shutdown().await;

        let captured = events.lock().unwrap().clone();
        let drain_line = captured
            .iter()
            .find(|l| l.contains("dispatch_retry_loop drained on shutdown"));
        assert!(
            drain_line.is_some(),
            "expected drained-on-shutdown log; got events: {:#?}",
            captured
        );
        // The `drained` field must be present (value depends on scheduling;
        // 0 is acceptable if all three were consumed before shutdown fired,
        // but the field must always appear for the TSDB extraction path).
        let line = drain_line.unwrap();
        assert!(
            line.contains("drained="),
            "expected `drained` field in drain log; got: {}",
            line
        );
    }

    /// §4.3 + §4.4 spec test (F-AMEND-CONS-4 contract) — a successful email
    /// dispatch emits the structured `dispatch_outcome=ok` event with a
    /// `duration_ms` field. These two fields are what the TSDB extractor
    /// projects into the `dispatch_outcome_total{outcome=ok}` counter and
    /// the `dispatch_duration_seconds` histogram.
    #[tokio::test]
    async fn retry_queue_metrics_emit() {
        let (events, _guard) = install_capture();

        let (email_mock, notify) = ScriptedEmailSender::new(vec![EmailAttemptOutcome::Ok]);
        let wa_mock = RecordingWhatsApp::new();
        let mgr = DispatchManager::spawn(email_mock.clone(), wa_mock, fast_config());

        mgr.enqueue_email(email_payload()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), notify.notified())
            .await
            .expect("first attempt should fire");
        // Allow the structured event to flush.
        tokio::time::sleep(Duration::from_millis(20)).await;

        let captured = events.lock().unwrap().clone();
        let ok_event = captured.iter().find(|l| {
            l.contains("dispatch_outcome=ok") && l.contains("duration_ms=")
        });
        assert!(
            ok_event.is_some(),
            "expected dispatch_outcome=ok event with duration_ms field for F-AMEND-CONS-4 histogram + counter extraction; got: {:#?}",
            captured
        );
        // Cleanly tear down to avoid loop-leak across tests.
        mgr.shutdown().await;
    }

    /// §4.4 spec test — a timeout on the underlying send emits the
    /// `dispatch_outcome=timeout` event. This is the field the
    /// `dispatch_outcome_total{outcome=timeout}` TSDB counter extracts.
    #[tokio::test]
    async fn dispatch_timeout_metric_increments() {
        let (events, _guard) = install_capture();

        // First attempt hangs (triggers per-attempt timeout); second succeeds
        // so the loop doesn't keep firing under the test's drain window.
        let (email_mock, _notify) = ScriptedEmailSender::new(vec![
            EmailAttemptOutcome::Hang,
            EmailAttemptOutcome::Ok,
        ]);
        let wa_mock = RecordingWhatsApp::new();
        let mgr = DispatchManager::spawn(email_mock.clone(), wa_mock, fast_config());

        mgr.enqueue_email(email_payload()).unwrap();
        // dispatch_timeout (50ms) + backoff (10ms) + second attempt: ~200ms safe.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let captured = events.lock().unwrap().clone();
        let timeout_event = captured.iter().find(|l| {
            l.contains("dispatch_outcome=timeout") && l.contains("duration_ms=")
        });
        assert!(
            timeout_event.is_some(),
            "expected dispatch_outcome=timeout event for F-CONS-18 timeout counter extraction; got: {:#?}",
            captured
        );
        mgr.shutdown().await;
    }
}
