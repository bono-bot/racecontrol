# RaceControl Codebase Conventions

## Rust Conventions

### Module Structure
- **Flat module organization**: Modules are declared in `main.rs` and live in `src/` as individual files or subdirectories. Example from rc-core:
  ```rust
  mod ac_camera;
  mod ac_server;
  mod accounting;
  mod billing;
  mod cloud_sync;
  mod config;
  mod db;
  // ...
  ```
- **Naming**: Modules use `snake_case` filenames matching the module name (`ac_server.rs`, `cloud_sync.rs`)
- **Submodules**: Grouped functionality uses subdirectories (e.g., `api/mod.rs`, `api/routes.rs`, `sims/mod.rs`)
- **Imports**: Use fully-qualified paths; avoid wildcard imports for clarity

### Naming Conventions
- **Variables & Functions**: `snake_case` (e.g., `billing_session_id`, `compute_dynamic_price`)
- **Types & Structs**: `PascalCase` (e.g., `BillingTimer`, `LockScreenState`, `AgentMessage`)
- **Enums**: `PascalCase` variants (e.g., `PodStatus::Offline`, `SessionStatus::Active`)
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Module names**: `snake_case` (e.g., `ac_server`, `lock_screen`)

### Error Handling Pattern
- **Primary pattern**: Use `anyhow::Result<T>` for fallible operations
  ```rust
  pub async fn stop_ac_server(state: &Arc<AppState>, session_id: &str) -> anyhow::Result<()>
  pub async fn compute_dynamic_price(state: &Arc<AppState>, base_price_paise: i64) -> i64
  ```
- **Error propagation**: Use `?` operator for propagation; `anyhow!()` for context
- **No custom error types**: Codebase uses anyhow for all errors, enabling easy context attachment
- **Logging**: Use `tracing` macros (`tracing::info!`, `tracing::warn!`, `tracing::error!`)
- **Default fallback pattern**: Functions that can't fail return the value directly (e.g., `compute_dynamic_price` returns `i64`, not `Result`)

### Workspace & Dependencies (from Cargo.toml)
```toml
[workspace]
members = ["crates/rc-common", "crates/rc-core", "crates/rc-agent"]
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
thiserror = "2"
sqlx = (implied, from code)
axum = (implied, for rc-core web framework)
```

### Common Patterns

#### State Management
- **Shared state**: Use `Arc<AppState>` passed through tokio spawned tasks
  ```rust
  let state = Arc::new(AppState::new(config, pool));
  let tick_state = state.clone();
  tokio::spawn(async move { ... });
  ```
- **Internal locks**: Use `RwLock<T>` for read-heavy data (pods, timers, etc.)
  ```rust
  pub struct BillingManager {
      pub active_timers: RwLock<HashMap<String, BillingTimer>>,
  }
  let mut timers = state.billing.active_timers.write().await;
  ```

#### Config Pattern
- **TOML deserialization with serde defaults**:
  ```rust
  #[derive(Debug, Deserialize)]
  pub struct Config {
      pub venue: VenueConfig,
      pub server: ServerConfig,
      #[serde(default)]
      pub cloud: CloudConfig,
  }

  #[derive(Debug, Deserialize)]
  pub struct ServerConfig {
      #[serde(default = "default_host")]
      pub host: String,
      #[serde(default = "default_port")]
      pub port: u16,
  }

  fn default_host() -> String { "0.0.0.0".to_string() }
  fn default_port() -> u16 { 8080 }
  ```
- **Loading**: `Config::load_or_default()` (implementation in rc-core/src/config.rs)

#### SQLx Patterns
- **Query binding with method chaining**:
  ```rust
  sqlx::query_as::<_, (String, f64, i64)>(
      "SELECT rule_type, multiplier, flat_adjustment_paise
       FROM pricing_rules
       WHERE is_active = 1 AND day_of_week = ? AND hour >= ?"
  )
  .bind(dow)
  .bind(hour)
  .fetch_optional(&state.db)
  .await?
  ```
- **Query-as pattern**: Destructure results directly into tuples for simple queries
- **Error handling**: Chain `.await?` for automatic Result propagation

### Serde Patterns

#### Rename Rules
- **`#[serde(rename_all = "snake_case")]`**: Convert enum variants to snake_case for JSON
  ```rust
  #[derive(Serialize, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub enum SimType {
      AssettoCorsa,      // → "assetto_corsa"
      AssettoCorsaEvo,   // → "assetto_corsa_evo"
  }
  ```
- **Individual overrides**: `#[serde(rename = "...")]` for special cases
  ```rust
  #[serde(rename = "iracing")]
  IRacing,
  #[serde(rename = "f1_25")]
  F125,
  ```

#### Serialization Control
- **`#[serde(skip_serializing_if = "Option::is_none")]`**: Omit None fields from JSON
  ```rust
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub mac_address: Option<String>,
  ```
- **`#[serde(default)]`**: Allow missing fields during deserialization, use Default
  ```rust
  #[serde(default)]
  pub cloud: CloudConfig,
  ```
- **`#[serde(skip_serializing_if = "Vec::is_empty")]`**: Omit empty vectors
  ```rust
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub installed_games: Vec<SimType>,
  ```

#### Message Protocol Pattern
- **Tagged enums for WebSocket messages** using `#[serde(tag = "type", content = "data")]`
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  #[serde(tag = "type", content = "data")]
  #[serde(rename_all = "snake_case")]
  pub enum AgentMessage {
      Register(PodInfo),           // → {"type": "register", "data": {...}}
      Heartbeat(PodInfo),
      Telemetry(TelemetryFrame),
      SessionUpdate(SessionInfo),
      DrivingStateUpdate { pod_id: String, state: DrivingState },  // → {"type": "driving_state_update", "data": {...}}
  }
  ```
- **Variants can be unit, tuple, or struct variants** — serde handles tag + content automatically

### WebSocket Message Protocol

#### Core Message Types (rc-common/src/protocol.rs)
1. **`AgentMessage`** (Pod Agent → Core Server):
   - Register, Heartbeat, Telemetry, LapCompleted, SessionUpdate, DrivingStateUpdate, Disconnect, GameStateUpdate, AiDebugResult, PinEntered

2. **`CoreToAgentMessage`** (Core Server → Pod Agent):
   - Registered, StartSession, StopSession, Configure, BillingStarted, BillingStopped, SessionEnded, LaunchGame, StopGame, ShowPinLockScreen, ShowQrLockScreen, ClearLockScreen, BlankScreen, BillingTick, SubSessionEnded, ShowAssistanceScreen, EnterDebugMode, SetTransmission, SetFfb

#### Serialization Format
- All messages use `#[serde(tag = "type", content = "data", rename_all = "snake_case")]`
- JSON format: `{"type": "message_variant", "data": {...}}`
- Deserialization happens via WebSocket layer (axum extractors)

### HTML Template Pattern (lock_screen.rs)

**Inline HTML strings with template placeholders**:
```rust
fn render_pin_screen(
    driver_name: &str,
    allocated_seconds: u32,
    pin_error: Option<&str>,
) -> String {
    format!(
        r#"<!DOCTYPE html>
        <html>
        <head>
            <title>RaceControl</title>
            <style>{{ ... }}</style>
        </head>
        <body>
            <div class="pin-container">
                <h1>{{DRIVER_NAME}}</h1>
                <div class="timer">{{ALLOCATED_SECONDS}}s</div>
                {error_section}
            </div>
        </body>
        </html>"#,
        error_section = if let Some(e) = pin_error {
            format!("<div class=\"error\">{}</div>", e)
        } else {
            String::new()
        }
    )
}
```

**Pattern**:
1. Use `format!()` macro with `r#"..."#` raw strings (avoid escaping)
2. Template variables as `{{UPPERCASE_NAME}}`
3. Conditionally include sections using Rust string interpolation
4. Inline CSS in `<style>` tags for single-file portability

## Frontend Conventions (Next.js / TypeScript / Tailwind)

### Project Structure
- **Kiosk** (`/root/racecontrol/kiosk/src/`): Staff-facing terminal dashboard
  - App Router (Next.js 13+)
  - Components in `components/`
  - Hooks in `hooks/` (e.g., `useKioskSocket.ts`)
  - Lib utilities in `lib/` (e.g., `api.ts`, `types.ts`)

- **PWA** (`/root/racecontrol/pwa/`): Customer-facing Progressive Web App
  - Pages in `app/` with layout nesting
  - Similar structure: components, hooks, lib

### TypeScript Conventions
- **Type definitions**: Centralized in `lib/types.ts`
  ```typescript
  export type AuthTokenInfo = { ... };
  export type PanelMode = "setup" | "live_session" | "wallet_topup" | null;
  ```
- **API responses**: Typed with optional fields
  ```typescript
  const res = await api.getWallet(driverId);
  if (res.wallet) { /* use res.wallet.balance_paise */ }
  ```

### React Hooks & State Management
- **`useKioskSocket`**: WebSocket connection to rc-core, manages pod state, billing timers, telemetry
  ```typescript
  const {
    connected,
    pods,
    billingTimers,
    gameStates,
    pendingAuthTokens,
    sendCommand,
  } = useKioskSocket();
  ```
- **`useState` for local UI state**: Panel modes, selected pod, form data
  ```typescript
  const [selectedPodId, setSelectedPodId] = useState<string | null>(null);
  const [panelMode, setPanelMode] = useState<PanelMode>(null);
  ```
- **`useCallback` for memoized callbacks**: Prevent unnecessary re-renders
  ```typescript
  const fetchWalletBalances = useCallback(async () => { ... }, [billingTimers]);
  useEffect(() => {
    fetchWalletBalances();
    const interval = setInterval(fetchWalletBalances, 15000);
    return () => clearInterval(interval);
  }, [fetchWalletBalances]);
  ```

### Tailwind CSS
- **Inline utility classes**: No external stylesheets; all styling via Tailwind classes
  ```typescript
  <div className="grid grid-cols-4 gap-4 p-6">
    <div className="bg-blue-600 text-white rounded-lg">Pod Card</div>
  </div>
  ```
- **Responsive design**: Prefix classes with breakpoints (`sm:`, `md:`, `lg:`)
- **Custom CSS**: Minimal; prefer Tailwind utilities

### Component Patterns
- **Functional components** with hooks
- **Props typed with TypeScript interfaces**
  ```typescript
  interface KioskPodCardProps {
    pod: PodInfo;
    isSelected: boolean;
    onClick: () => void;
  }

  export const KioskPodCard: React.FC<KioskPodCardProps> = ({ pod, isSelected, onClick }) => { ... };
  ```
- **Event handlers**: Typed as `React.MouseEventHandler`, `React.ChangeEventHandler`

### API Layer (lib/api.ts)
- **Centralized API client**: All HTTP calls go through `api` object
  ```typescript
  const res = await api.getWallet(driverId);
  const res = await api.startBilling(podId, driverId, pricingTierId);
  ```
- **Error handling**: Try-catch blocks, log errors, don't re-throw
  ```typescript
  try {
    const res = await api.fetch(...);
  } catch (error) {
    // handle error, don't propagate
  }
  ```

## Configuration Files

### racecontrol.toml
- TOML format with sections: `[venue]`, `[server]`, `[database]`, `[cloud]`, `[pods]`, `[branding]`, `[integrations]`, `[ai_debugger]`, `[ac_server]`, `[auth]`, `[watchdog]`
- Loaded via `Config::load_or_default()` at startup
- Example:
  ```toml
  [venue]
  name = "RacingPoint"
  location = "Hyderabad"
  timezone = "Asia/Kolkata"

  [server]
  host = "0.0.0.0"
  port = 8080

  [database]
  path = "racecontrol.db"

  [cloud]
  enabled = true
  api_url = "https://app.racingpoint.cloud/api/v1"
  sync_interval_secs = 30
  ```

### Environment Variables
- **JWT Secret** (auth.jwt_secret): Set in config or warn if default
- **Tracing level** (RUST_LOG): Defaults to "rc_core=info,tower_http=info"

## Documentation Patterns

### Comments
- **Module-level docs**: `//!` at top of file
  ```rust
  //! Lock screen UI for customer authentication on gaming PCs.
  //!
  //! Serves a fullscreen HTML page via a local HTTP server and launches
  //! Edge in kiosk mode to display PIN entry or QR code screens.
  ```
- **Function docs**: `///` with examples (rare in this codebase)
- **Inline comments**: `// ─── Section Header ───` for visual grouping
- **TODO comments**: Not found; use git issues instead

### Git Commits
- **Format**: Descriptive, action-oriented ("Fix billing timer sync", "Add cloud pricing rules sync")
- **No emoji prefixes** in this codebase
- **Co-authored commits**: Rare; this is single-author (Bono) except James contributions

---

**Last Updated**: March 2026 | **Codebase Version**: 0.1.0
