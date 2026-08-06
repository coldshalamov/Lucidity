# AT-094 Controller Review — Native Design v2

Disposition: **accepted after controller corrections**
Reviewed: 2026-08-06, America/New_York
Controller: root conductor

## Evidence received

- Canonical artifact: `C:\Users\93rob\.codex\delegations\lucidity-hackathon-20260806\AT-094\final-design-v2.md`
- Artifact SHA-256: `2990F41FB0CC19F0E26248494E966352AEE801937704B258ACD8E9B1C73A0D7A`
- Receipt SHA-256: `3AA08625363F7F5365AFA3EAEC74A76D7A8FB4B07CA4938025091B88C27D4ED3`
- Claude Code `2.1.220`; first-party `claude-opus-5`; effort `xhigh`; persisted session `f924b216-ede7-4df5-81fe-03eb299abbfd`.
- The controller-caused CLI interruption and same-session resume are disclosed in the receipt. Part 2 usage is exact; part 1 usage/cost is reconstructed from its last complete events because its raw stream ends with a 117-byte partial event. Combined observed cost is USD `5.518908`; exact full-session billing is unavailable.
- The artifact was read completely through line 947. HEAD remained `4b1c3c151eb530e569f867e1461693c56fe89695`; tracked and staged diffs and `git diff --check` were empty.
- The artifact's lines 1–3 claim that an advisor tool returned overload errors. Raw receipts expose only Read, Grep, and Glob and show no advisor call. That claim is unsupported and is struck from the accepted record. The design begins at its `# Lucidity Native Design v2` heading.
- ADR-004 was not model input and is not represented as such. The controller reconciled it below.

## Direct controller reproduction

The controller reproduced the load-bearing source claims rather than accepting narration:

1. `resolve_ui_item` reverse-searches registered items, while an uncovered pixel falls through to `mouse_event_terminal`; its coordinate mapping clamps negative x/y to zero. Full-bleed chrome hit coverage is therefore mandatory.
2. `UIItem::hit_test` is inclusive on both edges. Half-open layout rectangles must use a checked `width - 1` / `height - 1` adapter and touching rectangles; deliberately introduced gaps are forbidden.
3. `TermWindow` has one `Option<Rc<dyn Modal>>`, and `set_modal` replaces it. Lucidity must own one modal with an internal view stack.
4. `WindowOps` exposes show/hide/close/maximize/restore/focus but no minimize, while the Windows `hide()` implementation currently schedules `ShowWindow(Minimize)`. A distinct generic minimize operation is necessary before hide becomes true hide.
5. `render_element` resolves `hover_colors` from the current mouse event at paint time. Hover does not require reshaping/rebuilding the cached chrome tree.
6. `paint_tab_bar` is guarded by `show_tab_bar`; its progress-frame scheduling call is absent from that path when the tab bar is disabled. Runtime zero-additional-frame evidence remains mandatory.
7. `use_resize_increments` is a `#[dynamic(default)] bool`, so the live default is false. The deterministic fixture must still pin the value explicitly.

## Accepted design

The v2 aesthetic and interaction direction is accepted and frozen in `.orchestrator/DESIGN/NATIVE_UI_CONTRACT.md`:

- a native, square-edged “flight-strip board bolted to a terminal” rather than a second app/render stack;
- one 32-logical-pixel title/truth bar, a fixed left conversation sidebar, and a terminal viewport computed by one authoritative `AppLayout`;
- the 3-pixel State Rail for runtime truth and the achromatic row-to-terminal Weld for the unique current attachment;
- full-bleed typed chrome hit regions, no chrome input holes, and no stock hit-behaviour changes;
- 200/264/320 logical-pixel sidebar widths and a hidden sidebar with an explicit restore control below 820 logical pixels; the proposed 48-pixel rail remains deleted;
- Roboto for human chrome, JetBrains Mono for machine truth, bundled fonts only, no external asset or UI toolkit;
- eight runtime presentations independent of Active/Settled organization, including settled Working/Waiting/Approval and a fixed History attention counter;
- exactly two default global product chords, `Ctrl+Shift+A` and `Ctrl+Shift+B`; F6 and ordinary keys are chrome-local only; no raw-TUI pass-through mode;
- one `LucidityModal` with Usage, Settings, Confirm, and Select views on an internal stack;
- separate account quota and conversation context, stale-on-open/manual refresh, last-good-on-error, and no idle polling or hidden TUI injection;
- zero Lucidity animation/timers, event-driven tally motion only, list virtualization, and no transcript bytes in row view models;
- deterministic semantic IDs and fixture IDs without claiming a Windows accessibility provider in the MVP.

## Controller corrections and rulings

These override conflicting prose in the external artifact.

### R1 — ADR-004 and minimize

Accepted with corrected ownership. `WindowOps::minimize()` or an equivalent generic native-window hook belongs to AT-105's leased `window`/GUI seam. It is not a `CloseIntent`, not a `WindowClosePolicy` variant, and does not introduce a product dependency into `wezterm-gui`. AT-108 later maps product close intent; the caption minimize button calls the generic minimize operation. ADR-004, AT-105, and `INTERFACES.md` are amended accordingly.

### R2 — keybindings are defaults, not universal claims

`Ctrl+Shift+A/B` are unclaimed by the pinned stock default map and are accepted as Agent-mode defaults. They are not claimed free under arbitrary user key tables. Agent configuration must expose them as remappable/disableable, detect an explicit user conflict while materializing the effective map, and preserve the user's explicit binding unless they deliberately opt into the Lucidity chord. The chrome-focus layer and its stock-forward allowlist require exhaustive key-routing tests; no allowlisted action is inferred merely from chord spelling.

### R3 — runtime staleness is evidence-derived

The proposed universal `120 s` runtime-staleness threshold is rejected. An Agent-owned live attachment is current while its owned process/pane evidence is current; typed exit changes it, and a new `HostInstanceId` invalidates it immediately. Imported/external observations remain explicitly uncertain. Adapters may define evidence-specific expiry only when their signal semantics justify it. Usage snapshots retain the plan's separate five-minute stale-on-open threshold. No timer or poll is added to manufacture runtime state.

### R4 — context-pressure warning is staged, not cut

The per-row context-pressure treatment may be fixture-only in the first viewport slice, but it remains a required live Wave 2 behavior once AT-203 supplies a trusted per-conversation context source. It is not accepted as permanently fixture-only because the source plan requires a relevant row warning.

### R5 — adaptive seam

The two-pixel adaptive seam is accepted provisionally. AT-101 must prove the actual adjacent resolved terminal background source on both render paths, test dark/light/mid-grey palettes, and fall back to a deterministic high-contrast boundary if the proposed palette read is not the true adjacent color. It must not force the user's terminal palette.

### R6 — native evidence remains a gate

The design artifact is not runtime evidence. Cell metrics at 96/144/192 DPI, chrome-focus IME isolation, Medium font-face resolution, drag/double-click maximize, Win11 snap flyout, explicit minimize vs true hide, hover invalidation, idle-frame parity, resize/DPI round trip, renderer containment, and exact terminal input mapping remain native acceptance gates. No fixture golden may be derived from the implementation formula it is meant to test.

### R7 — responsive user intent

Automatic hide below 820 logical pixels applies only to `Auto`. An explicit `Shown` preference is honored down to the 600-pixel minimum; layout saturates safely and never produces a negative or overlapping terminal rectangle. Hidden mode must have mouse and keyboard restore paths and perform exactly one PTY resize per actual geometry transition.

## Required downstream ownership

- AT-105: true hide, generic minimize, show/focus/explicit-close hooks, reconciliation suppression, and stock parity.
- AT-101: `AppLayout`, library extraction, chrome renderer/cache/hit routing, responsive geometry, title-bar drag/snap, keyboard/IME routing, terminal containment, DPI and stock-parity evidence.
- AT-102: evidence-derived freshness representation, usage snapshot age/source/error data, and host view models without a universal runtime timeout.
- AT-201/203: complete row model and live context-pressure/usage surfaces.
- AT-108: product intent mapping, tray/reopen/Quit, and native end-to-end lifecycle proof.

No product mutation is authorized by this review. Implementation remains gated by HG-001 and the control-plane freeze commit.
