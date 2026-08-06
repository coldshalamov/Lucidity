# ADR-001 — Accessibility Scope for the Protected Demo

Status: accepted for the protected demo; post-demo debt remains release-significant.
Date: 2026-08-06
Decision owner: controller after AT-090 source reproduction.

## Context

The charter correctly treats accessibility as product behavior. The current WezTerm Windows window/renderer stack exposes no UI Automation or IAccessible provider, so promising screen-reader semantics inside the hackathon corridor would be unverifiable and would force a new native accessibility subsystem into the highest-risk renderer gate.

## Decision

For the protected demo:

- keyboard navigation, visible focus, deterministic focus order, color-independent status, contrast, reduced motion, IME priority, raw-TUI shortcut ownership, and accessible names in the pure semantic view model are mandatory;
- deterministic screen-reader/UIA fixtures may define future semantics, but no task may claim Windows screen-reader support without a real provider and native receipt;
- AT-093/094 must design a semantic component/state inventory that can later back UIA without changing domain truth;
- shipping beyond the demo requires a dedicated Windows UIA implementation/review gate or an explicit product-scope decision by the user.

This is a staged capability decision, not permission to use inaccessible color-only states or keyboard traps.

## Consequences

- The design remains credible on the existing native renderer seam.
- Screen-reader support stays visible debt rather than a false acceptance claim.
- A future provider can bind to stable semantic view-model IDs/names without parsing pixels or terminal text.
