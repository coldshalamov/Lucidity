# AT-093 Controller Review — Opus Native Design v1

## Verdict

**ACCEPTED AS THE GENERATIVE DIRECTION; SIX CONTROLLER RULINGS ARE PROVISIONAL UNTIL AT-094 ATTACKS THEM.**

The artifact is specific to the live WezTerm renderer and Windows windowing seams. It does not introduce a web view, second renderer, polling loop, scheduled chrome animation, or unbundled asset. The strongest accepted idea is the two-edge conversation strip: a left state rail carries runtime truth by shape, text, and colour, while a right attachment marker identifies the one conversation shown in the terminal. The terminal remains the dominant surface.

## Receipt

- Canonical artifact: `C:\Users\93rob\.codex\delegations\lucidity-hackathon-20260806\AT-093\final-design.md`
- SHA-256: `76B50F5B5879BA5AEC129C9941C96D61B8432B853E02DCBB87B1C6BB6F0CB7AE`
- Exact route: Claude Code `2.1.220`, first-party `claude-opus-5`, effort `xhigh`, fresh session `85f5f3ce-891a-47c5-a8f9-bd0c8e0a5032`
- Run: 81 turns, 1,642.725 seconds, USD 10.0241555
- Usage: 729 input, 288,036 cache creation, 7,969,201 cache read, 126,222 output tokens
- Executed tools: 54 Read, 22 Grep, 3 Glob; no MCP, web, or model switching
- One attempted Write to a Claude plan path was rejected because Write was unavailable. The target was not created and no write-capable operation executed.
- Repository HEAD, status, tracked diff, and staged diff were unchanged.

## Accepted direction

1. One native 32 logical-pixel title/truth bar instead of a second session header.
2. Native `Element`/quad/poly rendering only, with a single `AppLayout` authority for render, mouse, selection, cursor, IME, drag/drop, and PTY geometry.
3. Shape + text + colour for every runtime state; organization and runtime remain orthogonal, including Settled + Working.
4. Zero scheduled chrome frames. Progress changes only when structured evidence changes.
5. Bundled Roboto, JetBrains Mono, and Symbols Nerd Font assets; original in-house poly agent marks; no SVG dependency in MVP.
6. Durable `ConversationId` view-model identity and live `TabId` dispatch, never positional row or tab indexes.
7. Deterministic logical-pixel/DPI fixtures, explicit clipping-boundary assertions, Win11 snap preservation, and future-UIA semantic IDs without claiming a current provider.
8. Usage and context remain separate, stale-aware, manual/on-open only, with last-good values preserved.

## Controller rulings for AT-094 to attack

### C1 — UI font entities: accept the small MVP mapping

Use the four already-sized UI font entities in the Agent-materialized config. The fonts and required weights are compiled into the current default feature set. Do not widen the Wave 1 lease into `wezterm-font` merely to create a fifth size. Agent-mode stock overlays affected by the remapping require fixtures. A later font-entity lease needs visual evidence that the four-entity scale is inadequate.

### C2 — shortcut overrides: reject global replacement of stock bindings

Agent mode must preserve stock copy/paste/new-window/new-tab/close behavior and must not globally steal the proposed `Ctrl+Shift+*` set while the terminal owns focus. Product commands may be active when chrome or a Lucidity modal owns focus. AT-094 must reduce the global keyboard entry surface to the smallest credible focus-transfer/pass-through contract, test it against real TUI expectations, and keep keyboard-only access possible. `Alt+Space` remains the OS system menu.

### C3 — modal scrim: accept provisionally

One static 55% alpha quad behind a blocking sheet is functional focus separation, not decorative glass. It is allowed only if it leaves `has_animation == None`, adds no blur/shadow, and remains exactly one quad. AT-094 may replace it only with a clearer and cheaper treatment.

### C4 — terminal palette: reject product ownership

Lucidity must honor the user's terminal color scheme. The protected screenshot harness may pin `#0B0D10`, but the product may not. Chrome tokens are the default Instrument theme, and the seam/focus treatment must remain legible against the actual adjacent terminal background.

### C5 — typed chrome hit regions: accept

Add a typed `UIItemType::Chrome(ChromeItem)` in AT-101's existing GUI lease. Do not silently change stock hit behavior. `AppLayout` rectangles are half-open; any adapter into the legacy inclusive `UIItem::hit_test` representation must avoid overlaps without gaps and must have boundary tests.

### C6 — 48 px rail: reject as the automatic small-window mode

At 600–819 logical px, default to a fully hidden sidebar with a visible title-bar restore control. A rail that shows unidentified glyphs, omits History, and has no tooltip is not yet a credible navigation surface. AT-094 may retain a user-requested rail only if it proves discoverability, keyboard access, all required organization/runtime signals, and useful terminal width. Frame C must otherwise become the hidden-sidebar proof.

## Additional defects AT-094 must resolve

- `HISTORY · N▶` conflates Working, WaitingForInput, and AwaitingApproval and may hide the most important settled attention state. Keep organization fixed, but produce a more truthful compact signal.
- The settings sheet's stated 640×400 minimum conflicts with the 600×320 window minimum; define a usable compact behavior or a defensible minimum-window restriction.
- The visibility table says usage is always visible while Frame C hides it; one contract must win.
- `accent == state.working` plus an accent attachment notch risks meaning collision, especially in the collapsed/unfocused cases.
- The global F6 and raw-pass-through proposal remains unaccepted until it is tested against terminal/TUI key ownership.
- Cell metrics at 96/144/192 DPI are not yet receipts. AT-094 may specify assertions, but must not promote derived guesses to goldens.

## AT-094 acceptance bar

Return a corrected v2 that is smaller or clearer than v1, explicitly retains or rejects each ruling above, and labels any claim that still requires native evidence. It is not allowed to praise v1 without attempting to falsify idle invalidation, keyboard ownership, Settled + Working/Needs visibility, compact-window usability, and render/input feasibility.
