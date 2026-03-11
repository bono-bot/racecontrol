# RaceControl Testing Patterns and Framework

This document outlines testing approaches across the Rust and TypeScript codebases, including test locations, frameworks, mocking patterns, and coverage areas.

---

## Rust Testing

### Test Framework

**Primary Framework**: Rust's built-in `#[test]` macro with `#[cfg(test)]` modules.

**Standard Pattern**: Tests are defined inline in modules using `#[cfg(test)]` blocks:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        // Arrange
        let input = setup();

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

### Test Locations

Tests are colocated with source code in the same file, at the bottom in a `#[cfg(test)]` module:

**Examples**:
- `crates/rc-common/src/protocol.rs` (12 tests for message serialization)
- `crates/rc-agent/src/driving_detector.rs` (7 tests for driving state detection)
- `crates/rc-core/src/billing.rs` (tests for billing timer logic)
- `crates/rc-agent/src/sims/assetto_corsa.rs` (telemetry parsing tests)
- `crates/rc-agent/src/sims/f1_25.rs` (F1 25 parsing tests)

### Test Modules Found

The codebase includes **47 tests** across three crates:

1. **rc-common**: Protocol/serialization tests (message roundtrips)
2. **rc-agent**: Driving detection, UDP protocol parsing, telemetry parsing
3. **rc-core**: Billing timer behavior (to be added/expanded)

### Running Tests

**Command**:
```bash
# Run all tests across all crates
cargo test

# Run tests for specific crate
cargo test -p rc-common
cargo test -p rc-agent
cargo test -p rc-core

# Run with output
cargo test -- --nocapture

# Run single test
cargo test test_cloud_action_booking_roundtrip
```

**From Memory Documentation**:
```
Test commands:
  cargo test -p rc-common &&
  cargo test -p rc-agent &&
  cargo test -p rc-core
```

### Test Coverage Areas

#### 1. Protocol/Serialization Tests (`rc-common/src/protocol.rs`)

**Purpose**: Verify that message types serialize/deserialize correctly with serde JSON.

**Pattern** (roundtrip tests):
```rust
#[test]
fn test_cloud_action_booking_roundtrip() {
    let action = CloudAction::BookingCreated {
        booking_id: "book-123".to_string(),
        driver_id: "drv-456".to_string(),
        pricing_tier_id: "tier-30min".to_string(),
        experience_id: Some("exp-nurburgring".to_string()),
        pod_id: Some("pod_3".to_string()),
    };
    let json = serde_json::to_string(&action).unwrap();
    assert!(json.contains("booking_created"));
    let parsed: CloudAction = serde_json::from_str(&json).unwrap();
    if let CloudAction::BookingCreated { booking_id, .. } = parsed {
        assert_eq!(booking_id, "book-123");
    } else {
        panic!("Wrong variant");
    }
}

#[test]
fn test_cloud_action_wallet_roundtrip() {
    let action = CloudAction::WalletTopUp {
        driver_id: "drv-1".to_string(),
        amount_paise: 90000,
        transaction_id: "txn-abc".to_string(),
    };
    let json = serde_json::to_string(&action).unwrap();
    let parsed: CloudAction = serde_json::from_str(&json).unwrap();
    if let CloudAction::WalletTopUp { amount_paise, .. } = parsed {
        assert_eq!(amount_paise, 90000);
    } else {
        panic!("Wrong variant");
    }
}

#[test]
fn test_agent_message_roundtrip() {
    let msg = AgentMessage::PinEntered {
        pod_id: "pod_1".to_string(),
        pin: "1234".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("pin_entered"));
    let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
    if let AgentMessage::PinEntered { pod_id, pin } = parsed {
        assert_eq!(pod_id, "pod_1");
        assert_eq!(pin, "1234");
    } else {
        panic!("Wrong variant");
    }
}

#[test]
fn test_core_to_agent_billing_tick() {
    let msg = CoreToAgentMessage::BillingTick {
        remaining_seconds: 1500,
        allocated_seconds: 1800,
        driver_name: "Test Driver".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("billing_tick"));
    let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
    if let CoreToAgentMessage::BillingTick { remaining_seconds, .. } = parsed {
        assert_eq!(remaining_seconds, 1500);
    } else {
        panic!("Wrong variant");
    }
}

#[test]
fn test_pending_cloud_action_serde() {
    let pending = PendingCloudAction {
        id: "act-1".to_string(),
        action: CloudAction::BookingCancelled {
            booking_id: "book-999".to_string(),
        },
        created_at: "2026-03-07T12:00:00Z".to_string(),
    };
    let json = serde_json::to_string(&pending).unwrap();
    let parsed: PendingCloudAction = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.id, "act-1");
    if let CloudAction::BookingCancelled { booking_id } = parsed.action {
        assert_eq!(booking_id, "book-999");
    } else {
        panic!("Wrong variant");
    }
}
```

**Tested Message Types**:
- `CloudAction::BookingCreated`
- `CloudAction::WalletTopUp`
- `CloudAction::BookingCancelled`
- `CloudAction::QrConfirmed`
- `CloudAction::SettingsChanged`
- `CloudAction::Notification`
- `PendingCloudAction`
- `AgentMessage::PinEntered`
- `CoreToAgentMessage::BillingTick`

#### 2. Driving Detector Tests (`rc-agent/src/driving_detector.rs`)

**Purpose**: Verify hysteresis-based state machine for detecting active driving.

**Pattern**:
```rust
fn make_detector() -> DrivingDetector {
    DrivingDetector::new(&DetectorConfig::default())
}

#[test]
fn initial_state_is_no_device() {
    let d = make_detector();
    assert_eq!(d.state(), DrivingState::NoDevice);
}

#[test]
fn hid_active_transitions_to_active() {
    let mut d = make_detector();
    let (state, changed) = d.process_signal(DetectorSignal::HidActive);
    assert_eq!(state, DrivingState::Active);
    assert!(changed);
}

#[test]
fn udp_active_transitions_to_active() {
    let mut d = make_detector();
    let (state, changed) = d.process_signal(DetectorSignal::UdpActive);
    assert_eq!(state, DrivingState::Active);
    assert!(changed);
}

#[test]
fn stays_active_within_idle_threshold() {
    let mut d = make_detector();
    d.process_signal(DetectorSignal::HidActive);
    d.process_signal(DetectorSignal::HidIdle);
    let (state, _) = d.evaluate_state();
    assert_eq!(state, DrivingState::Active);
}

#[test]
fn hid_disconnect_without_udp_goes_to_no_device() {
    let mut d = make_detector();
    d.process_signal(DetectorSignal::HidDisconnected);
    let (state, _) = d.evaluate_state();
    assert_eq!(state, DrivingState::NoDevice);
}

#[test]
fn input_active_detection() {
    let config = DetectorConfig::default();
    let input = WheelbaseInput {
        steering: 0.0,
        throttle: 0.1,
        brake: 0.0,
    };
    assert!(is_input_active(&input, &config));

    let idle_input = WheelbaseInput {
        steering: 0.01,
        throttle: 0.01,
        brake: 0.01,
    };
    assert!(!is_input_active(&idle_input, &config));
}

#[test]
fn steering_movement_detection() {
    assert!(is_steering_moving(0.5, 0.0, 0.02));
    assert!(!is_steering_moving(0.01, 0.0, 0.02));
}
```

**Test Coverage**:
- Initial state verification
- State transitions (HID active, UDP active)
- Hysteresis behavior (stays active within threshold)
- Disconnect handling
- Input thresholds (throttle/brake)
- Steering movement detection

#### 3. UDP Protocol Tests (`rc-common/src/udp_protocol.rs`)

**Purpose**: Verify binary packet parsing for heartbeat ping/pong.

**Pattern** (bitfield operations):
```rust
#[test]
fn pod_status_bits_roundtrip() {
    let mut status = PodStatusBits::new();
    status.set_ws_connected(true);
    status.set_game_running(true);
    status.set_driving_active(true);
    status.set_billing_active(false);
    status.set_game_id(1);
    status.set_cpu_percent(75);
    status.set_gpu_percent(60);

    assert!(status.ws_connected());
    assert!(status.game_running());
    assert!(status.driving_active());
    assert!(!status.billing_active());
    assert_eq!(status.game_id(), 1);
    assert_eq!(status.cpu_percent(), 75);
    assert_eq!(status.gpu_percent(), 60);
}
```

#### 4. Telemetry Parsing Tests (`rc-agent/src/sims/assetto_corsa.rs`, `f1_25.rs`)

**Purpose**: Verify UDP telemetry packet parsing from game engines.

**Pattern** (binary parsing):
```rust
#[test]
fn parse_ac_telemetry() {
    let data = vec![/* binary packet data */];
    let frame = parse_assetto_corsa_telemetry(&data);
    assert!(frame.is_some());
    let f = frame.unwrap();
    assert_eq!(f.speed_kmh, expected_speed);
}
```

### Test Style Conventions

**Helper Functions**: Extract common setup into helper functions:
```rust
fn make_detector() -> DrivingDetector {
    DrivingDetector::new(&DetectorConfig::default())
}
```

**Assertion Patterns**:
```rust
// Simple equality
assert_eq!(result, expected);

// Boolean
assert!(condition);
assert!(!condition);

// Option/Result matching
if let Some(value) = result {
    assert_eq!(value, expected);
} else {
    panic!("Expected Some, got None");
}
```

**Test Naming**: Descriptive names following `test_<what>` pattern:
- `test_initial_state_is_no_device`
- `test_hid_active_transitions_to_active`
- `test_stays_active_within_idle_threshold`
- `test_cloud_action_booking_roundtrip`

### Mocking Patterns

**No formal mocking library**: The codebase relies on:
1. **Dependency injection**: Pass config/state to functions
2. **Pure functions**: Prefer stateless functions over mocking
3. **Helper functions**: Create small test fixtures

**Example** (DrivingDetector):
```rust
fn make_detector() -> DrivingDetector {
    DrivingDetector::new(&DetectorConfig::default())
}
// Now tests can create instances without DB/network
```

**Async Testing**: Not heavily used yet; async functions are integration-tested in `rc-core` via manual testing on Pod 8.

### Integration Testing

**Strategy**: Limited integration tests in the codebase. Relies on:
1. **End-to-end testing on Pod 8**: Deploy and test before rolling out to all 8 pods
2. **Manual verification**: Verify on actual hardware before CI
3. **Characterization tests first**: Test the current behavior before refactoring (per Bono's TDD rule)

**From Memory Documentation**:
```
Rule (from Uday): Always run tests + verify on Pod 8 before claiming done
Test commands: cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core
Step-by-step: Backend → Frontend → Connect → Test → Deploy (one layer at a time)
Test First, Refactor Second (from Bono): ALWAYS test the current state BEFORE changing anything.
```

### Code Quality Standards

**No .unwrap() in Production**: All production code uses `?` operator or explicit error handling.
- `.unwrap()` is only acceptable in tests and examples.

**Database Queries**: Chain with `.await?`:
```rust
let rules = sqlx::query_as::<_, (String, f64, i64)>(...)
    .bind(value)
    .fetch_optional(&state.db)
    .await
    .ok()        // OK: test can use .unwrap() here
    .flatten();
```

---

## TypeScript Testing

### Test Framework

**Current State**: No dedicated test framework configured in the codebase.

**Typical Approach** (for future): Would use Jest or Vitest for:
- Component tests (React Testing Library)
- Hook tests (testing-library/react)
- API client tests

**Current Testing Method**: Manual testing during development and on deployed instances.

### Testing Patterns (TypeScript/React)

**Best Practices** (not yet implemented but should follow):

1. **Component Snapshot Tests**: Verify UI doesn't break unexpectedly
2. **Hook Tests**: Test `useWebSocket` behavior with mock WebSocket
3. **API Client Tests**: Mock fetch and verify request formatting

### Example (Hypothetical Hook Test)

```typescript
// __tests__/useWebSocket.test.ts
import { renderHook, act } from "@testing-library/react";
import { useWebSocket } from "@/hooks/useWebSocket";

describe("useWebSocket", () => {
  it("should connect and receive pod updates", async () => {
    const { result } = renderHook(() => useWebSocket());

    expect(result.current.connected).toBe(false);

    // Mock WebSocket connection
    // Verify connected state changes
    // Simulate message receipt
    // Verify state updates
  });

  it("should reconnect on disconnect", async () => {
    // Test auto-reconnect logic
  });
});
```

### Example (Hypothetical API Client Test)

```typescript
// __tests__/api.test.ts
import { api } from "@/lib/api";

describe("api client", () => {
  it("should format POST requests correctly", async () => {
    const payload = {
      pod_id: "pod_1",
      driver_id: "drv_123",
      pricing_tier_id: "tier-30min",
    };

    // Mock fetch
    global.fetch = jest.fn(() =>
      Promise.resolve({
        json: () => Promise.resolve({ ok: true }),
      })
    );

    await api.startBilling(payload);

    expect(global.fetch).toHaveBeenCalledWith(
      "http://localhost:8080/api/v1/billing/start",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify(payload),
      })
    );
  });
});
```

### Manual Testing Strategy

**Current Approach**: Manual testing on actual pods with structured verification:

1. **Component Testing**: Verify in browser dev tools
2. **Hook Testing**: Monitor WebSocket messages in Network tab
3. **API Testing**: Use curl or Postman to verify endpoints
4. **End-to-End**: Test full user journeys on deployment

---

## Deployment Testing Strategy

### Pre-Deployment Checklist (from Memory)

1. **Run tests locally**:
   ```bash
   cargo test -p rc-common
   cargo test -p rc-agent
   cargo test -p rc-core
   ```

2. **Check binary size**:
   ```bash
   ls -lh target/release/rc-agent.exe
   ```

3. **Deploy to Pod 8 first**: Isolated testing before rolling to all 8 pods

4. **Verify behavior**:
   - Check logs for errors
   - Test billing flow
   - Test game launch
   - Monitor resource usage

5. **Rollout**: If Pod 8 tests pass, deploy to remaining pods (1-7)

### From Memory Documentation

```
Rule: "Test before upload" means:
  - cargo test (unit tests)
  - check binary size
  - deploy to ONE pod first (Pod 8)
  - NEVER execute the binary locally on James's machine

Troubleshooting: See [debugging-playbook.md](debugging-playbook.md) and [pod-commands.md](pod-commands.md)
```

---

## Test Statistics

**Current Coverage**:
- **Total Tests**: 47 across all crates
- **rc-common**: 12+ protocol/serialization tests
- **rc-agent**: ~15 driving detector + telemetry parsing tests
- **rc-core**: ~20 (billing, accounting, AI, etc. — to be expanded)

**Critical Areas Tested**:
1. Protocol serialization (100% coverage of message types)
2. Driving state detection (hysteresis state machine)
3. UDP heartbeat protocol (bitfield operations)
4. Telemetry parsing (game-specific formats)

**Areas Needing More Tests**:
1. Async operations (channels, WebSocket, HTTP)
2. Database operations (billing persistence)
3. AI debugger (Ollama/Anthropic fallback logic)
4. Pod reservation (multi-session split billing)
5. TypeScript components (React Testing Library)

---

## Running Tests Locally

### Full Test Suite

```bash
cd /c/Users/bono/racingpoint/racecontrol

# Run all tests
cargo test

# Run with output
cargo test -- --nocapture --test-threads=1

# Run specific crate
cargo test -p rc-common
cargo test -p rc-agent
cargo test -p rc-core

# Run single test
cargo test test_driving_detector_state_transitions
```

### Typical Development Workflow

1. Write/modify code
2. Run focused tests:
   ```bash
   cargo test -p rc-agent test_
   ```
3. Run full suite before committing:
   ```bash
   cargo test
   ```
4. Deploy to Pod 8 for integration testing:
   ```bash
   # See [pod-commands.md] for deployment procedures
   ```
5. Verify on pod and roll out to others

---

## Summary Table

| Aspect | Framework | Pattern | Example |
|--------|-----------|---------|---------|
| Rust tests | `#[test]` macro | Inline in `#[cfg(test)]` | Protocol roundtrips, driving detector |
| Test runner | `cargo test` | Per-crate or all | `cargo test -p rc-common` |
| Naming | `test_<what>` | Descriptive | `test_cloud_action_booking_roundtrip` |
| Protocol tests | Serde roundtrip | Serialize → deserialize → compare | Verify message JSON format |
| Driver detection | State machine | Signal → state transitions | Test hysteresis behavior |
| Mocking | Dependency injection | Helper functions | `make_detector()` fixture |
| Error handling | No `.unwrap()` | Use `.ok().flatten()` or `?` | DB query chains |
| TS testing | Not yet implemented | Would use Jest + RTL | Manual testing + Pod 8 |
| Integration testing | Pod 8 deployment | Manual verification | Deploy, test, rollout |
| Pre-deployment | cargo test + Pod 8 | Isolate testing on Pod 8 | TDD cycle + pod validation |
