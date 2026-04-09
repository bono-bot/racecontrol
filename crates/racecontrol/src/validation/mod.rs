//! Server-side validity gates.
//!
//! Pre-DB, pre-lock validation primitives. Each submodule exposes pure
//! functions that return structured errors consumed by axum handlers and
//! serialized to HTTP 422 on the wire.
//!
//! Phase 361-01 (code-only session 2026-04-09).

pub mod session_validity;
