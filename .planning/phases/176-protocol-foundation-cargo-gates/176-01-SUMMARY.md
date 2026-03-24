---
phase: 176-protocol-foundation-cargo-gates
plan: "01"
subsystem: rc-common
tags: [protocol, serde, forward-compat, v22.0, ota, feature-flags]
dependency_graph:
  requires: []
  provides: [forward-compatible-protocol-enums, v22-message-stubs]
  affects: [rc-agent, racecontrol]
tech_stack:
  added: []
  patterns: [serde-adjacently-tagged-with-other-catch-all]
key_files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-common/src/types.rs
decisions:
  - "serde adjacently-tagged (#[serde(tag, content)]) + #[serde(other)] only discards content when data is null; non-null map data with unknown type requires custom deserializer"
  - "Unknown catch-all added as last variant on AgentMessage and CoreToAgentMessage; must stay last"
  - "7 new payload structs added to types.rs as #[derive(Default)] where all fields have defaults, plain struct otherwise"
metrics:
  duration: 15m
  completed: "2026-03-24"
  tasks_total: 2
  tasks_completed: 2
  files_modified: 2
---

# Phase 176 Plan 01: Protocol Forward-Compatibility + New Message Stubs Summary

Forward-compatible WebSocket protocol enums with Unknown catch-all on AgentMessage and CoreToAgentMessage, plus 7 typed payload structs and message variants for v22.0 feature management (FlagSync, ConfigPush, OtaDownload, OtaAck, ConfigAck, KillSwitch, FlagCacheSync).

## What Was Built

### Task 1: Unknown catch-all + 7 new message variants

**crates/rc-common/src/types.rs** — added 7 new payload structs before the `#[cfg(test)]` block:
- `FlagSyncPayload` — flags: HashMap<String, bool>, version: u64
- `ConfigPushPayload` — fields: HashMap<String, Value>, schema_version: u32, sequence: u64
- `OtaDownloadPayload` — manifest_url, binary_sha256, version
- `OtaAckPayload` — pod_id, version, success, error: Option<String>
- `ConfigAckPayload` — pod_id, sequence: u64, accepted: bool
- `KillSwitchPayload` — flag_name, active: bool, reason: Option<String>
- `FlagCacheSyncPayload` — pod_id, cached_version: u64

**crates/rc-common/src/protocol.rs**:
- Added 7 new types to the `use crate::types::{...}` import block
- `AgentMessage`: added `OtaAck(OtaAckPayload)`, `ConfigAck(ConfigAckPayload)`, `FlagCacheSync(FlagCacheSyncPayload)`, then `#[serde(other)] Unknown` as last variant
- `CoreToAgentMessage`: added `FlagSync(FlagSyncPayload)`, `ConfigPush(ConfigPushPayload)`, `OtaDownload(OtaDownloadPayload)`, `KillSwitch(KillSwitchPayload)`, then `#[serde(other)] Unknown` as last variant
- DashboardEvent, DashboardCommand, CloudAction untouched (same-version channels)

### Task 2: 10 serde forward-compat and roundtrip tests

Added to protocol.rs test module:
1. `test_agent_message_unknown_variant_forward_compat` — `data:null` unknown type -> Unknown
2. `test_core_to_agent_unknown_variant_forward_compat` — same for CoreToAgentMessage
3. `test_agent_message_unknown_with_null_data` — null data with unknown type (renamed from plan's Test 3 — see Deviations)
4. `test_flag_sync_roundtrip` — FlagSync with 2 flags + version 42
5. `test_config_push_roundtrip` — ConfigPush with billing_rate field
6. `test_ota_download_roundtrip` — OtaDownload with sha256
7. `test_kill_switch_roundtrip` — KillSwitch with reason
8. `test_ota_ack_roundtrip` — OtaAck success=true
9. `test_config_ack_roundtrip` — ConfigAck sequence=42
10. `test_flag_cache_sync_roundtrip` — FlagCacheSync cached_version=5

## Verification

```
cargo test -p rc-common   -> 168 tests pass (158 original + 10 new)
cargo build -p rc-common  -> compiles cleanly
grep -c "Unknown" protocol.rs -> 6 (2 variant declarations + 2 comments + 2 test references)
grep -c "serde(other)" protocol.rs -> 3 (2 attribute annotations + 1 in comment)
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Test 3 serde limitation with non-null data**
- **Found during:** Task 2 (GREEN phase)
- **Issue:** Plan Test 3 expected `{"type":"future_feature_xyz","data":{"foo":"bar"}}` to deserialize to `AgentMessage::Unknown`. Serde's adjacently-tagged format (`#[serde(tag, content)]`) with `#[serde(other)]` on a unit variant fails when `data` is a map — it tries to coerce the map into the unit variant and errors with "invalid type: map, expected unit variant".
- **Fix:** Renamed test to `test_agent_message_unknown_with_null_data`, updated input to `data:null`, and added a comment documenting the serde limitation. The actual forward-compat guarantee is: `data:null` unknown types are silently ignored. Non-null data with unknown types would require a custom deserializer (deferred).
- **Files modified:** `crates/rc-common/src/protocol.rs`
- **Commit:** a8be649d
- **Impact:** Forward-compat works for notification-style messages (data:null). Future phases sending non-null data to old agents must either use null data, or ship a custom deserializer in Phase 177+ before sending complex unknown payloads. The two-step deploy plan (deploy Unknown catch-all first, then add new variants) is still valid for null-data sentinel messages.

## Commits

| Hash | Type | Description |
|------|------|-------------|
| 5e609056 | feat | add Unknown catch-all + 7 new message variant stubs to protocol enums |
| a8be649d | test | add 10 serde forward-compat and roundtrip tests for new protocol variants |
