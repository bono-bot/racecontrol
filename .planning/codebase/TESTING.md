# RaceControl Testing Strategy & Framework

## Current Testing Status

### Rust (Cargo workspace)
**Status**: MINIMAL — No dedicated test files found in codebase

- **No test harness**: No `#[cfg(test)]` modules in crates/
- **No test framework**: Neither `cargo test` nor established unit tests
- **Manual testing**: All testing is integration/manual at runtime
- **Why**: Early-stage venue system with tight hardware coupling (pods, USB devices, UDP networking)

**Test capabilities available** (not used):
- `cargo test` would execute tests if they existed
- Unused dependencies in Cargo.toml: `thiserror` v2 (suggests potential error type testing)

### TypeScript/JavaScript (Kiosk & PWA)
**Status**: MINIMAL — No test configuration found

- **No test framework**: No Jest, Vitest, or Mocha configuration
  - No `jest.config.js`, `vitest.config.ts`, or `jest.setup.ts`
  - No test scripts in `package.json`
- **No test files**: No `*.test.ts`, `*.spec.ts`, `*.test.tsx` files in source directories
  - Node_modules contain test files from Next.js itself, but not application tests
- **Manual verification**: Tests performed via browser/kiosk at runtime

## Test Locations (Where Tests Would Go)

### Rust
- **Unit tests**: Colocate with modules using `#[cfg(test)]` blocks at end of each file
  ```rust
  // In crates/rc-core/src/billing.rs
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_billing_timer_tick() { ... }
  }
  ```
- **Integration tests**: Would go in `crates/rc-core/tests/` directory (not present)
- **Run**: `cargo test -p rc-core`

### TypeScript/JavaScript
- **Unit tests**: Colocate with modules
  - `src/hooks/useKioskSocket.test.ts`
  - `src/components/KioskPodCard.test.tsx`
  - `src/lib/api.test.ts`
- **Integration tests**: `tests/integration/` directory (not present)
- **E2E tests**: Playwright or Cypress (not configured)
- **Run**: `npm test` (would require jest/vitest configuration)

## Testing Patterns (If Implemented)

### Rust Testing Best Practices for This Codebase

#### Unit Tests
**For billing calculations**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_compute_dynamic_price_off_peak() {
        // Mock time to off-peak hours
        let state = Arc::new(test_state());
        let base_price = 70000; // ₹700 in paise

        let adjusted = compute_dynamic_price(&state, base_price).await;
        assert!(adjusted < base_price); // Off-peak should discount
    }

    #[tokio::test]
    async fn test_billing_timer_expiry() {
        let mut timer = BillingTimer {
            allocated_seconds: 60,
            driving_seconds: 0,
            // ... other fields
        };

        for _ in 0..60 {
            let expired = timer.tick();
            assert!(!expired);
        }
        assert_eq!(timer.remaining_seconds(), 0);
    }
}
```

**For serialization**:
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_agent_message_serialization() {
        let msg = AgentMessage::Heartbeat(PodInfo { ... });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"heartbeat\""));

        let deserialized: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, msg);
    }
}
```

#### Async Tests
- Use `#[tokio::test]` macro (from tokio dev-dependency)
- For anything touching `AppState`, database, or WebSocket operations

#### Mocking
- **sqlx**: Use in-memory SQLite (`:memory:`)
  ```rust
  let db = SqlitePool::connect("sqlite::memory:").await?;
  db.execute("CREATE TABLE ... ").await?;
  ```
- **State mocking**: Create test fixtures
  ```rust
  fn test_state() -> AppState {
      AppState::new(test_config(), in_memory_pool())
  }
  ```

### TypeScript Testing Patterns

#### Unit Tests (Hooks)
```typescript
// src/hooks/useKioskSocket.test.ts
import { renderHook, waitFor } from '@testing-library/react';
import { useKioskSocket } from './useKioskSocket';

describe('useKioskSocket', () => {
  it('should connect to WebSocket on mount', async () => {
    const { result } = renderHook(() => useKioskSocket());
    await waitFor(() => expect(result.current.connected).toBe(true));
  });

  it('should handle pod updates', async () => {
    const { result } = renderHook(() => useKioskSocket());
    // Simulate message from WebSocket
    // Assert pod state updated
  });
});
```

#### Component Tests
```typescript
// src/components/KioskPodCard.test.tsx
import { render, screen, fireEvent } from '@testing-library/react';
import { KioskPodCard } from './KioskPodCard';

describe('KioskPodCard', () => {
  it('should render pod number and status', () => {
    const pod = { number: 1, status: 'idle', ... };
    render(<KioskPodCard pod={pod} isSelected={false} onClick={jest.fn()} />);
    expect(screen.getByText('Pod 1')).toBeInTheDocument();
  });

  it('should call onClick when clicked', () => {
    const onClick = jest.fn();
    const pod = { ... };
    render(<KioskPodCard pod={pod} isSelected={false} onClick={onClick} />);
    fireEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalled();
  });
});
```

#### API Tests
```typescript
// src/lib/api.test.ts
describe('API Client', () => {
  beforeEach(() => {
    global.fetch = jest.fn();
  });

  it('should fetch wallet balance', async () => {
    (global.fetch as jest.Mock).mockResolvedValueOnce({
      json: async () => ({ wallet: { balance_paise: 100000 } }),
    });

    const result = await api.getWallet('driver-123');
    expect(result.wallet?.balance_paise).toBe(100000);
  });
});
```

## Test Fixtures & Utilities

### Rust Fixtures (Would Go in tests/fixtures.rs or similar)
```rust
// Common test data builders
pub fn test_pod_info() -> PodInfo {
    PodInfo {
        id: "test-pod-1".to_string(),
        number: 1,
        name: "Pod 1".to_string(),
        ip_address: "192.168.31.89".to_string(),
        sim_type: SimType::AssettoCorsa,
        status: PodStatus::Idle,
        // ...
    }
}

pub async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    pool
}
```

### TypeScript Fixtures (Would Go in tests/fixtures.ts)
```typescript
export const mockPodInfo: PodInfo = {
  id: 'pod-1',
  number: 1,
  name: 'Pod 1',
  status: 'idle',
  // ...
};

export const mockBillingTimer: BillingSessionInfo = {
  id: 'billing-123',
  driver_id: 'driver-456',
  // ...
};
```

## Coverage Status

**Estimated Coverage**: <5%
- **No automated coverage tracking** (no Istanbul/nyc config)
- **Tested paths**:
  - Manual system tests at venue (pods, billing, game launches)
  - WebSocket connectivity verified via dashboard UI
- **Untested paths**:
  - Billing edge cases (timer sync, offline recovery)
  - SQLx query error handling
  - Cloud sync conflict resolution
  - React hook state transitions
  - API error responses

## Integration Testing (Manual / At Venue)

The codebase relies on **integration testing at the venue**:

1. **Billing System**: Staff launches session → Timer ticks → Pod blanks on expiry
2. **Pod Communication**: Core sends `StartSession` → Agent receives via WebSocket → Game launches
3. **Kiosk Dashboard**: Shows live pod grid, session timers, billing warnings
4. **Wallet System**: Customer topup → Credits reflect → Session starts
5. **Game Telemetry**: UDP frames received → Telemetry endpoint returns lap data
6. **Cloud Sync**: Venue laptop pulls pricing rules → Applies to new sessions

**Result**: System verified working end-to-end as of March 2026 (latest commit: `22c7f8d`)

## Recommendations for Adding Tests

### Phase 1: High-Value Unit Tests (Billing)
- Billing timer calculations (edge cases: expiry, multi-split)
- Dynamic pricing logic (time-of-day, discount chains)
- Wallet debit/credit (verify paise calculations)
- Effort: ~40 test cases, 4 hours

### Phase 2: Serialization & Protocol Tests
- AgentMessage/CoreToAgentMessage round-trip JSON
- Config TOML parsing with defaults
- Telemetry frame validation
- Effort: ~30 test cases, 2 hours

### Phase 3: API Route Tests (Axum)
- Status codes (200, 400, 401, 404)
- JWT validation
- Request/response serialization
- Effort: ~50 test cases, 6 hours

### Phase 4: React Hook Tests
- useKioskSocket connection/reconnection
- WebSocket message handling
- State updates (pods, billing, telemetry)
- Effort: ~30 test cases, 4 hours

## Test Dependencies (If Added)

### Rust
```toml
[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt"] }
sqlx = { version = "0.7", features = ["runtime-tokio-native-tls", "sqlite"] }
serde_json = "1"
mockall = "0.12"  # For mocking async functions
tempfile = "3"    # For temporary files in tests
```

### TypeScript
```json
{
  "devDependencies": {
    "@testing-library/react": "^14",
    "@testing-library/jest-dom": "^6",
    "@testing-library/user-event": "^14",
    "jest": "^29",
    "jest-environment-jsdom": "^29",
    "ts-jest": "^29",
    "@types/jest": "^29"
  }
}
```

---

**Last Updated**: March 2026
**Testing Philosophy**: **Manual integration testing at venue** — Codebase design emphasizes runtime correctness over unit test coverage due to tight hardware coupling.
