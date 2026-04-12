# Phase 379: Mesh Event Bus — Execution Plan

## Architecture Decision: Comms-link as Event Hub

Comms-link (not racecontrol server) is the event hub because:
- Always running on Bono VPS
- All devices already connect to it
- Has HMAC auth + dedup + ACK tracking built in
- Neutral territory — neither server-side nor agent-side

## Two New Message Types in Protocol

Added to existing `shared/protocol.js`:

### `domain_event` (publish)
```json
{
  "type": "domain_event",
  "department": "game_launch",
  "event_name": "GameStarted",
  "payload": { "pod_id": 3, "game": "AssettCorsa", "driver_id": 42 },
  "correlation_id": "uuid-from-origin",
  "source_identity": "pod-3"
}
```

### `event_subscribe` (subscribe)
```json
{
  "type": "event_subscribe",
  "patterns": ["game_launch.*", "billing.*"],
  "source_identity": "racecontrol-server"
}
```

## Subscription-Based Fan-Out

Each client declares which departments it cares about. EventBus dispatches only matching events:
- `pod-N` subscribes to: `billing.BillingStarted`, `billing.BillingEnded` (for its own pod)
- `racecontrol-server` subscribes to: `*` (all events)
- `bono` subscribes to: `*` (monitoring)
- `kiosk-N` subscribes to: `billing.*`, `game_launch.*` (for real-time UI)
- `pos-dashboard` subscribes to: `billing.*`, `game_launch.*`, `cafe.*`

## Data Flow

```
Pod 3 game starts
  → rc-agent HTTP POST to local mesh-agent (:8766/publish)
    → mesh-agent forwards via WS to comms-link hub
      → comms-link EventBus matches subscribers
        → racecontrol-server receives (subscribed to *)
          → server starts billing, writes to DB
          → server proxies to kiosk/POS via dashboard WS
        → bono receives (subscribed to *)
          → logs, monitors, alerts
```

## 6 Implementation Plans

### Plan 1: Comms-link EventBus (~200 lines)
- New file: `comms-link/shared/event-bus.js`
- Subscription registry (Map of client → pattern list)
- Pattern matching (department.event_name glob)
- Fan-out to matched subscribers only
- Correlation ID tracking (store last 1000 for query)
- New handler in `bono/comms-server.js` for `domain_event` and `event_subscribe`

### Plan 2: Mesh-agent publisher (~80 lines)
- New endpoint: `POST /publish` on mesh-agent HTTP server
- Accepts JSON domain event, wraps in protocol envelope, sends via WS
- rc-agent calls this locally (no new WS client in Rust)

### Plan 3: rc-agent event publishing (~50 lines)
- After game state changes, HTTP POST to `http://localhost:8766/publish`
- Events: GameStarted, GameCrashed, GameEnded, LapCompleted
- Correlation ID from the LaunchRequest (passed through launch contract)

### Plan 4: Racecontrol server subscription (~100 lines)
- New WS client connecting to comms-link
- Subscribes to all events (`*`)
- Routes events to appropriate handlers (billing, lap ingestion, etc.)
- Proxies to dashboard WS as `DomainEventForward` for kiosk/POS

### Plan 5: Frontend subscription (~80 lines)
- Kiosk/POS already connect to racecontrol dashboard WS
- New message type `DomainEventForward` in WS handler
- Frontend React hooks: `useDomainEvent('billing.*', handler)`
- Real-time UI updates without polling

### Plan 6: Correlation ID query (~60 lines)
- New endpoint: `GET /relay/events?correlation_id=X`
- Returns all events in the chain for a customer action
- Useful for debugging: "what happened after this launch request?"

## Total: ~570 new lines across 6 files

## Deployment Order (Incremental)

1. Comms-link EventBus (no consumers yet — safe)
2. Mesh-agent publisher (no producers yet — safe)
3. rc-agent publishing (starts producing events — comms-link handles them)
4. Racecontrol subscription (starts consuming events)
5. Frontend hooks (UI updates in real-time)

Each step independently rollback-safe.

## Dependencies

- Phase 369 contract types (LaunchRequest correlation_id) — DONE
- Phase 379 domain_events.rs in rc-common — agent building this now
- Comms-link accessible from all devices (Tailscale mesh) — already working

## Risk

- **WS reconnect during event delivery** — use ACK tracking already in comms-link
- **Event ordering** — correlation_id + timestamp, not guaranteed ordering across departments
- **Volume** — 8 pods × telemetry events = high volume. Rate-limit telemetry to 1/sec per pod for mesh (full rate stays local).
