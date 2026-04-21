// packages/shared-types/src/index.ts — Phase 445 Wave 3 (D-10 dual-write)
//
// Migrated types re-export from ../generated/ (auto-generated from Rust via
// `cargo run --release --bin gen-types --features gen-types`).
// Un-migrated + D-19 permanent types stay hand-written in src/*.ts.
//
// Drift gate: bash scripts/check-generated-types-drift.sh (Plan 04 CI).
// Per-type verdicts: .planning/phases/445-typed-api-contract-rust-ts-codegen/445-03-DRIFT-AUDIT.md

// ==========  MIGRATED (D-10) — source: ../generated/ (auto-generated from Rust)  ==========

// Pod/game enums and catalog types (parity verified)
export type { PodStatus, SimType, DrivingState, GameState } from '../generated/index';

// Pod info (additive: admin hand-written `Pod` retained below for BC)
export type { PodInfo } from '../generated/index';

// Billing status enum (11 variants, parity verified byte-identical)
export type { BillingSessionStatus, PricingTier } from '../generated/index';

// Billing session info (additive: admin hand-written `BillingSession` retained below for BC;
// bigint drift on cost_paise/rate_per_min_paise — opt-in when admin handles bigint)
export type { BillingSessionInfo } from '../generated/index';

// Playable signal enum (new generated type, not previously hand-written)
export type { PlayableSignal } from '../generated/index';

// Inventory types (new generated types, not previously hand-written)
export type { PodInventory, GameInventory, ContentDirsResponse, GameDirs } from '../generated/index';

// ==========  HAND-WRITTEN (D-19 permanent OR held pending follow-on)  ==========

// Composite / superset / admin-only types retained from src/*.ts:
// - Pod: hand-written admin canonical; generated emits `PodInfo` (both exported for BC)
// - GameCatalogEntry: composite, no Rust struct (kiosk handler assembles it)
export type { Pod, GameCatalogEntry } from './pod';

// - BillingSession: hand-written superset of BillingSessionInfo; held pending bigint refactor
export type { BillingSession } from './billing';

// Driver: hand-written has admin-only `has_used_trial`; held pending bigint + Rust-side has_used_trial
export type { Driver } from './driver';

// Fleet types:
// - PodFleetStatus held pending bigint refactor on `uptime_secs` / `clock_drift_secs` / `crash_recovery_count`
//   (admin `fleet/page.tsx` does arithmetic on uptime_secs that would break with bigint).
// - FleetHealthResponse has no Rust struct (built via serde_json::json! inline).
// - ConfigMismatchDetected is a WS message type (D-19 permanent).
export type { PodFleetStatus, FleetHealthResponse, ConfigMismatchDetected } from './fleet';

// Admin-only DB-shape types (no Rust struct, admin manages schema)
export type { FeatureFlag, ConfigPush, ConfigAuditEntry } from './config';

// WS protocol — D-19 permanent (handle_ws_message mega-hub; separate phase for versioning)
export type { FlagSyncPayload, WsConfigPushPayload, OtaDownloadPayload, KillSwitchPayload,
              ConfigAckPayload, OtaAckPayload, FlagCacheSyncPayload, LaunchDiagnostics,
              BillingTick, GameStateChanged } from './ws-messages';

// Admin/kiosk-only composite handler responses (no Rust struct)
export type { RedeemPinResponse, RedeemPinStatus } from './reservation';

// Admin-only metrics response types (no Rust struct; admin DB queries)
export type { FailureMode, LaunchStatsResponse, BillingAccuracyResponse,
              AlternativeCombo, LaunchMatrixRow } from './metrics';
