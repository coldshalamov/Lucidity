//! Adapter, event, and deterministic harness implementation boundary.
//!
//! This package implements the AT-103 adapter contract:
//!
//! - [`manifest`]: declarative adapter package TOML parsing plus schema and
//!   semantic validation. Shell interpolation is rejected; launches are
//!   always direct executable-plus-argument construction.
//! - [`discovery`]: executable discovery and version probe abstractions with
//!   deterministic, test-injectable search environments.
//! - [`template`]: Windows-safe `{placeholder}` expansion that never routes
//!   through a shell and never quotes, escapes, or reinterprets values.
//! - [`events`]: the ADR-002 OSC 1337 `SetUserVar` encoder/decoder for the
//!   frozen `lucidity.agent-event.v1` envelope.
//! - [`reducer`]: the deterministic identity/evidence reducer covering
//!   `PendingIdentity` hints, confirmations, supersession, expiry, crash,
//!   duplicate/out-of-order/stale evidence, and old-ID-end-after-rebind.
//!
//! The `lucidity-mock-agent` binary target is the deterministic PTY-friendly
//! mock coding-agent TUI used by the protected lifecycle tests.

pub mod discovery;
pub mod events;
pub mod manifest;
pub mod reducer;
pub mod template;
