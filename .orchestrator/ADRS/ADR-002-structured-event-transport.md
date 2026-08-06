# ADR-002 — Structured Agent Event Transport

Status: accepted for the protected demo.
Date: 2026-08-06
Decision owner: controller after AT-092 source reproduction.

## Context

The first draft proposed `OSC 777;agent-terminal://event;<raw JSON>`. The live parser maps OSC 777 to `RxvtExtension(Vec<String>)`, and the terminal performer handles only the `notify` subcommand. Every other 777 subcommand is silently discarded. Raw JSON also is not a stable OSC field because semicolons split parameters and the parser tracks at most 64 OSC fields.

WezTerm already has a pane-correlated structured path: OSC 1337 `SetUserVar`. Its value is base64-decoded into `Alert::SetUserVar`, `LocalPaneNotifHandler` publishes that alert as a `MuxNotification` with the emitting `PaneId`, and an in-process host can subscribe without changing the parser, terminal model, or mux notification shape.

## Decision

The protected demo transport is:

```text
OSC 1337 ; SetUserVar=lucidity.agent-event.v1=<base64(UTF-8 JSON envelope)> ST
```

- The reserved variable name is exactly `lucidity.agent-event.v1`.
- `MAX_AGENT_EVENT_JSON_BYTES` is 32,768 decoded bytes. Emitters must refuse a larger envelope; the host checks the decoded string length before deserialization and quarantines an oversized value.
- The mux callback performs only the length/name/owned-pane checks and a non-blocking enqueue into a 256-item host queue. JSON parsing, dedupe, store correlation, and SQLite work run off the GUI/mux callback thread. Overflow marks that attachment `event_resync_required` and schedules bounded adapter/store reconciliation; it never blocks terminal output.
- The envelope version and `eventId` remain mandatory. Structured-event dedupe is unique on `(adapter_id, profile_id, event_id)`.
- The Agent-mode host subscribes to `MuxNotification::Alert`, filters `Alert::SetUserVar` by the reserved name, and associates the untrusted value with the notification's `PaneId` and current `HostInstanceId` before reduction.
- Stock mode continues to treat the value as an ordinary user variable. No stock OSC behavior changes.
- A value from the terminal is evidence, not authority: it can update only the currently owned, adapter-compatible attachment for that pane. It cannot supply a trusted attachment generation, execute code, or select a different profile.
- Unknown versions, malformed JSON, duplicate IDs, invalid identity transitions, and values from an unowned pane are logged/quarantined without panicking or mutating current truth.

## Consequences

- AT-103 owns encoding/decoding and fixtures; AT-102 owns the Agent-mode mux subscriber and reducer handoff. AT-099 freezes the constant and envelope types in `agent-protocol`.
- No new `wezterm-escape-parser`, `term`, or GUI bridge is required for Wave 1.
- Acceptance still requires an end-to-end ConPTY receipt from the mock process through the existing alert pipeline and bounded queue into the host, including overflow/resync. Unit-testing only the JSON decoder is insufficient.
- The existing OSC parser may allocate for arbitrary third-party user variables before the Lucidity bound is checked. Lucidity does not widen that existing surface; a parser-wide global cap would be a separate upstream hardening proposal.
