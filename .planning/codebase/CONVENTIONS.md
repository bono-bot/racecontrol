# RaceControl Codebase Conventions

This document outlines coding style, naming conventions, error handling, logging, module organization, and configuration patterns across the Rust and TypeScript projects.

---

## Rust Conventions

### Module Organization

**Structure**: The project follows a workspace layout with three crates:
- `rc-common`: Shared types, protocols, and message definitions
- `rc-core`: Backend server (Axum web framework, SQLite database)
- `rc-agent`: Pod-side agent (Windows services, hardware monitoring)

**Module Pattern**: Each crate's `src/main.rs` declares internal modules:

```rust
mod ac_camera;
mod ac_server;
mod accounting;
mod billing;
mod config;
mod db;
// ... etc
```

Modules are typically one file per module (`src/module_name.rs`) unless they have submodules (e.g., `src/api/mod.rs`, `src/api/routes.rs`, `src/auth/mod.rs`, `src/sims/mod.rs`).

### Code Style

**Formatting**: Follows Rust standard conventions (enforced by `rustfmt`).

**Naming**:
- **Functions**: `snake_case` (e.g., `compute_dynamic_price`, `tick_all_timers`, `parse_openffboard_report`)
- **Types/Structs**: `PascalCase` (e.g., `BillingTimer`, `DrivingDetector`, `DetectorConfig`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `HEARTBEAT_PORT`, `PING_INTERVAL_SECS`, `MISS_THRESHOLD`)
- **Modules**: `snake_case` (e.g., `billing`, `ac_server`, `pod_monitor`)

**Structs and Impl Blocks**:
```rust
#[derive(Debug, Clone)]
pub struct BillingTimer {
    pub session_id: String,
    pub driver_id: String,
    pub allocated_seconds: u32,
    // ...
}

impl BillingTimer {
    pub fn new() -> Self { /* ... */ }
    pub fn remaining_seconds(&self) -> u32 { /* ... */ }
    pub fn to_info(&self) -> BillingSessionInfo { /* ... */ }
}
```

**Serde Derives**: Common across the codebase:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum AgentMessage {
    // ...
}
```

### Error Handling

**Philosophy**: No `.unwrap()` or `.expect()` in production code. Use Result types and the `?` operator.

**Patterns**:
- **Internal errors**: Use `anyhow::Result<T>` for most async functions
- **Domain-specific errors**: Use `thiserror::Error` for custom error types (not extensively used yet, but encouraged)
- **DB queries**: Chain with `.await?` to propagate errors:

```rust
let rules = sqlx::query_as::<_, (String, f64, i64)>(
    "SELECT rule_type, multiplier, flat_adjustment_paise FROM pricing_rules WHERE ..."
)
.bind(dow)
.fetch_optional(&state.db)
.await
.ok()          // Convert sqlx::Error → None
.flatten();    // Flatten Option<Option<T>> → Option<T>
```

**Unwrapping in Tests**: Tests use `.unwrap()` and `.expect()` freely, as they represent test failures.

**Channel Operations**:
```rust
match result_tx.send(AiDebugSuggestion { /* ... */ }).await {
    Ok(()) => tracing::info!("[ai-debug] Suggestion sent"),
    Err(e) => tracing::error!("[ai-debug] Failed to send: {}", e),
}
```

**serde_json unwrap in protocol tests**:
```rust
let json = serde_json::to_string(&action).unwrap();
let parsed: CloudAction = serde_json::from_str(&json).unwrap();
```
This is acceptable in tests where serialization failures are test failures.

### Logging

**Framework**: `tracing` crate with `tracing-subscriber`.

**Levels** (in order of severity):
- **error**: Non-recoverable failures, requires immediate attention
- **warn**: Potentially problematic situations (e.g., pod offline, fallback triggered)
- **info**: Important milestones and state changes
- **debug**: Detailed diagnostic info, function entries
- **trace**: Very fine-grained context (rarely used)

**Patterns**:
```rust
// Info: milestone
tracing::info!("[ai-debug] Ollama responded: {} chars", suggestion.len());

// Warn: fallback or degradation
tracing::warn!("Auto-ending stuck billing session {} (offline >60s)", timer.session_id);

// Debug: detailed
tracing::debug!("[ai-debug] Prompt length: {} chars", prompt.len());

// Error: failure
tracing::error!("[ai-debug] Failed to send suggestion: {}", e);
```

**Prefixes**: Some modules use log prefixes like `[ai-debug]`, `[billing]`, `[pod-monitor]` to ease filtering.

### Configuration

**Pattern**: TOML-based configuration loaded at startup.

**Example** (`racecontrol.toml`):
```toml
[venue]
name = "Racing Point eSports"
location = "Bangalore, India"
timezone = "Asia/Kolkata"

[server]
host = "0.0.0.0"
port = 8080

[database]
path = "racecontrol.db"

[cloud]
enabled = true
turso_url = "libsql://..."
api_url = "https://app.racingpoint.cloud/api/v1"
sync_interval_secs = 30
terminal_secret = "secure-secret"
terminal_pin = "1234"

[ai_debugger]
enabled = true
ollama_url = "http://localhost:11434"
ollama_model = "racing-point-ops"
anthropic_api_key = "sk-ant-..."
```

**Loading**:
```rust
pub struct Config {
    pub venue: VenueConfig,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub cloud: CloudConfig,
    // ...
}

impl Config {
    pub fn load_or_default() -> Self {
        // Load from racecontrol.toml or use defaults
    }
}
```

**Defaults**: Use `#[serde(default)]` and default functions:
```rust
#[serde(default = "default_ollama_url")]
pub ollama_url: String,

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
```

### Comments and Documentation

**Doc Comments**: Public items include `///` comments:
```rust
/// Analyze a crash/error and produce a debug suggestion.
/// Runs as a spawned async task — makes HTTP calls to Ollama/Anthropic.
pub async fn analyze_crash(
    config: AiDebuggerConfig,
    // ...
) {
```

**Block Comments**: Complex algorithms get `// ─── Header ───` comments:
```rust
// ─── BillingTimer ───────────────────────────────────────────────────────────

pub struct BillingTimer {
    // ...
}
```

**Inline Comments**: Explain non-obvious logic:
```rust
// Magic bytes: "RP" (0x52, 0x50) — reject stray packets
const MAGIC: [u8; 2] = [0x52, 0x50];

// Auto-end if offline > 60 seconds
if (Utc::now() - since).num_seconds() > 60 {
    // ...
}
```

### Async/Await

**Pattern**: Use `tokio::spawn` for background tasks and `async fn` for endpoints:

```rust
// Billing tick loop (1 second interval)
let tick_state = state.clone();
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        billing::tick_all_timers(&tick_state).await;
    }
});

// Async endpoint handler
pub async fn handle_request(
    State(state): State<Arc<AppState>>,
) -> Json<Response> {
    // ...
}
```

**Locking**: Use `tokio::sync::RwLock` for async-safe shared state:
```rust
pub struct BillingManager {
    pub active_timers: RwLock<HashMap<String, BillingTimer>>,
}

// Read
let timers = state.billing.active_timers.read().await;

// Write
let mut timers = state.billing.active_timers.write().await;
timers.insert(pod_id, timer);
```

### Serialization Patterns

**Protocol Enums**: Use serde tag + content for discriminated unions:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum AgentMessage {
    Register(PodInfo),           // {"type": "register", "data": {...}}
    Heartbeat(PodInfo),          // {"type": "heartbeat", "data": {...}}
    DrivingStateUpdate { pod_id: String, state: DrivingState }, // {"type": "driving_state_update", ...}
}
```

**Optional Fields**: Skip serialization of None values:
```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub metadata: Option<serde_json::Value>,
```

---

## TypeScript Conventions

### Module Organization

**Structure**: Next.js 15+ with App Router.

```
web/src/
├── app/
│   ├── layout.tsx
│   ├── page.tsx
│   ├── pods/page.tsx
│   ├── billing/page.tsx
│   └── ...
├── components/
│   ├── PodCard.tsx
│   ├── DashboardLayout.tsx
│   └── ...
├── hooks/
│   └── useWebSocket.ts
└── lib/
    └── api.ts
```

**Naming**:
- **Components**: `PascalCase` (e.g., `PodCard`, `DashboardLayout`, `BillingStartModal`)
- **Hooks**: `camelCase`, prefix with `use` (e.g., `useWebSocket`)
- **Utilities/lib**: `camelCase` (e.g., `api.ts`)
- **Pages**: `kebab-case` directories (e.g., `/pods`, `/billing/history`)

### Code Style

**Type Safety**: Always use explicit types:
```typescript
// Good
interface PodCardProps {
  pod: Pod;
  billingSession?: BillingSession;
  pendingToken?: AuthTokenInfo;
  onCancelToken?: (tokenId: string) => void;
}

function PodCard({ pod, billingSession, pendingToken, onCancelToken }: PodCardProps) {
  // ...
}
```

**Client Components**: Mark with `"use client"`:
```typescript
"use client";

import { useEffect, useRef, useState, useCallback } from "react";
```

**Avoid Hydration Issues**: Never read sessionStorage/localStorage in `useState` initializer. Use `useEffect`:

```typescript
// BAD ❌
const [theme, setTheme] = useState(localStorage.getItem("theme") || "dark");

// GOOD ✅
const [theme, setTheme] = useState<string | null>(null);
useEffect(() => {
  setTheme(localStorage.getItem("theme") || "dark");
}, []);
```

### Hooks

**useWebSocket**: Central connection management for dashboard real-time updates:

```typescript
export function useWebSocket() {
  const ws = useRef<WebSocket | null>(null);
  const [connected, setConnected] = useState(false);
  const [pods, setPods] = useState<Map<string, Pod>>(new Map());

  const sendCommand = useCallback(
    (command: string, data: Record<string, unknown>) => {
      if (ws.current?.readyState === WebSocket.OPEN) {
        ws.current.send(JSON.stringify({ command, data }));
      }
    },
    []
  );

  return {
    connected,
    pods: Array.from(pods.values()),
    sendCommand,
    // ...
  };
}
```

**useCallback and Dependencies**: Memoize event handlers and command senders to prevent stale closures.

### API Client

**Pattern** (`lib/api.ts`):
```typescript
const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";

export async function fetchApi<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}/api/v1${path}`, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });
  return res.json();
}

export const api = {
  health: () => fetchApi<{ status: string; version: string }>("/health"),
  listPods: () => fetchApi<{ pods: Pod[] }>("/pods"),
  startBilling: (data: BillingStartPayload) =>
    fetchApi<{ ok: boolean }>("/billing/start", {
      method: "POST",
      body: JSON.stringify(data),
    }),
  // ...
};
```

**Usage**:
```typescript
const { pods } = await api.listPods();
const ok = await api.startBilling({ pod_id: "pod_1", driver_id: "drv_123" });
```

### Components

**Props as Interfaces**: Always define props types explicitly:

```typescript
interface PodCardProps {
  pod: Pod;
  billingSession?: BillingSession;
  onCancelToken?: (tokenId: string) => void;
}

export default function PodCard({ pod, billingSession, onCancelToken }: PodCardProps) {
  return (
    <div className={`rounded-lg border p-4 transition-all ${/* class logic */}`}>
      {/* JSX */}
    </div>
  );
}
```

**State Management**: Use `useState` for local component state; `useWebSocket` hook for global server state.

### Styling

**Tailwind CSS**: Inline class strings with conditional logic:

```typescript
className={`rounded-lg border p-4 transition-all ${
  billingSession
    ? "border-rp-red/50 bg-rp-red/5"
    : isPending
    ? "border-yellow-500/50 bg-yellow-500/5"
    : pod.status === "idle"
    ? "border-emerald-500/30 bg-rp-card"
    : "border-rp-border bg-rp-card"
}`}
```

**Color Tokens** (defined in Tailwind config):
- `rp-red` = `#E10600` (Racing Red)
- `rp-grey` = `#5A5A5A` (Gunmetal Grey)
- `rp-card` = `#222222` (Card background)
- `rp-border` = `#333333` (Border color)

### Error Handling

**Fetch Errors**: Wrap API calls in try-catch:
```typescript
try {
  const result = await api.startBilling(payload);
  setSuccess(true);
} catch (error) {
  console.error("Failed to start billing:", error);
  setError(error instanceof Error ? error.message : "Unknown error");
}
```

**WebSocket Errors**: Automatically reconnect:
```typescript
socket.onclose = () => {
  setConnected(false);
  setTimeout(connect, 3000); // Retry after 3s
};

socket.onerror = () => {
  socket.close();
};
```

### Type Definitions

**API Types** (`lib/api.ts`):
```typescript
export interface Pod {
  id: string;
  number: u32;
  name: string;
  status: "idle" | "in_session" | "error" | "offline";
  sim: string;
  // ...
}

export interface BillingSession {
  id: string;
  pod_id: string;
  driver_id: string;
  driver_name: string;
  allocated_seconds: number;
  driving_seconds: number;
  remaining_seconds: number;
  status: "active" | "paused" | "completed";
  // ...
}
```

---

## Cross-Crate Conventions

### Workspace Dependencies

All workspace crates share dependencies declared in `Cargo.toml`:
```toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
thiserror = "2"
tracing = "0.1"
```

Individual crates reference:
```toml
[dependencies]
serde = { workspace = true }
tokio = { workspace = true }
```

### Message Protocol

**Defined in `rc-common/src/protocol.rs`**: All message types used by agents, core, and dashboards:
- `AgentMessage` (Pod → Core)
- `CoreToAgentMessage` (Core → Pod)
- `DashboardEvent` (Core → Dashboard)
- `DashboardCommand` (Dashboard → Core)

All message enums use `#[serde(tag = "type", content = "data")]` for consistent JSON serialization.

### Type Sharing

**rc-common** provides:
- Core domain types: `Pod`, `BillingSession`, `DrivingState`, `SimType`
- Message enums
- UDP heartbeat protocol definitions
- No external dependencies (except serde)

**rc-core** and **rc-agent** depend on `rc-common` to share these types.

---

## Summary Table

| Aspect | Convention | Example |
|--------|-----------|---------|
| Rust functions | `snake_case` | `compute_dynamic_price`, `tick_all_timers` |
| Rust types | `PascalCase` | `BillingTimer`, `DrivingDetector` |
| Rust constants | `SCREAMING_SNAKE_CASE` | `HEARTBEAT_PORT`, `MISS_THRESHOLD` |
| TS components | `PascalCase` | `PodCard`, `DashboardLayout` |
| TS hooks | `camelCase`, `use*` prefix | `useWebSocket`, `useCallback` |
| TS utilities | `camelCase` | `api.ts`, `fetchApi` |
| Error handling (Rust) | `Result<T>`, `?` operator, no `.unwrap()` in prod | Use `anyhow::Result` |
| Error handling (TS) | try-catch, propagate via Promise | Wrap API calls |
| Logging (Rust) | `tracing` crate | `tracing::info!`, `tracing::warn!` |
| Configuration | TOML with defaults | `racecontrol.toml`, `#[serde(default)]` |
| Async (Rust) | `tokio::spawn`, `async fn`, `RwLock` | Background loops, endpoint handlers |
| State (TS) | `useState`, `useWebSocket` hook | Local + server state |
| Styling (TS) | Tailwind CSS inline classes | `className={`rounded-lg ${condition ? '...' : '...'}`}` |
