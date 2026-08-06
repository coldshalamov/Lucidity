# ADR-004 — Product/GUI Dependency Direction and Close Policy Boundary

Status: accepted architecture; implementation remains gated.
Date: 2026-08-06
Decision owner: controller after live manifest and target reproduction.

## Context

The provisional baseline placed `CloseIntent` in `agent-protocol` while AT-105 was expected to use it inside `wezterm-gui`. That would make the stock upstream GUI package depend on a Lucidity product protocol crate solely for close behavior. The live package also has no library target yet: Cargo accepts a path dependency on a binary-only package only by warning that the dependency is invalid and ignoring it. Predeclaring `agent-terminal -> wezterm-gui` at AT-099 would therefore make the supposedly clean baseline warning-dependent.

The two boundaries are different:

- the host decides product intent such as hide-to-tray or explicit quit;
- the reusable GUI decides how a native window close is enacted while preserving or killing its mux window.

They need an explicit mapping, not a reversed upstream dependency.

Controller reproduction used a temporary two-package Cargo workspace with a consumer path-depending on a binary-only package. `cargo metadata --format-version 1 --no-deps` accepted the manifest, but `cargo check -p consumer` emitted `ignoring invalid dependency 'binary-only' which is missing a lib target` and compiled only the consumer. The temporary probe was cleaned immediately; no repository product source or tracked file changed.

## Decision

- `agent-protocol` contains serializable host requests/views/events only. It does **not** own a GUI close-policy enum.
- `agent-terminal` owns the product lifecycle intent `CloseIntent::{StockClose, HideToTray, QuitOwned}` in its predeclared lifecycle/host module. `StockClose` exists for mapping and parity tests; the stock binary does not depend on this type.
- `wezterm-gui` owns a small product-agnostic close seam, provisionally `WindowClosePolicy::{Stock, HidePreservingMux}`, plus typed show/focus, explicit-close, and real minimize hooks. `minimize` is a generic native-window operation, not a Lucidity close-policy variant: it remains visually and semantically distinct from true hide. AT-105 may choose equivalent names, but it must not import `agent-protocol` or `agent-terminal`.
- AT-105 implements and tests the generic GUI/window behavior. AT-108 maps Lucidity host intent onto that seam: window close maps to hide-preserving-mux, tray Exit performs owned-job/persistence shutdown and then explicit GUI close, and stock keeps the default stock policy.
- AT-099 does not declare an invalid `agent-terminal -> wezterm-gui` dependency and does not create a placeholder GUI library.
- After AT-102/103 and the native foundations are integrated, AT-101 creates the real `wezterm-gui` library target. In the same serialized change it may add only the existing path dependency to `agent-terminal/Cargo.toml` and the mechanical `Cargo.lock` delta. No new external dependency or root workspace member is permitted.
- No worker may accept Cargo's “missing a lib target” warning as a baseline state.

## Consequences

- Stock WezTerm remains independent of Lucidity's product crates.
- The existing caption-button meaning remains intact after `hide()` becomes a true hide: minimize calls the explicit generic minimize hook, while close-to-tray maps through `HidePreservingMux`.
- AT-102 and AT-103 can implement against a clean AT-099 product baseline without racing a manifest that AT-101 later needs.
- AT-101 starts from the integrated host/mock/foundation base and receives an exact, serialized `agent-terminal/Cargo.toml` plus `Cargo.lock` sublease.
- The controller must verify the path-dependency/lock delta separately from GUI extraction and reject any unrelated manifest churn.
