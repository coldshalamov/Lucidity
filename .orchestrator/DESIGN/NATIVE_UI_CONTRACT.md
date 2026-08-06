# Lucidity Native UI Contract

Status: **accepted implementation contract**
Date: 2026-08-06
Authority: controller synthesis of AT-093, AT-094, live-source reproduction, ADR-001 through ADR-004, and the HACKATHON source plan.

## 1. Authority and scope

This file is the in-repository design authority for Lucidity's native Windows MVP. If the external Opus artifacts conflict with this file, this file and the accepted ADRs win.

Provenance:

- Opus v1: `C:\Users\93rob\.codex\delegations\lucidity-hackathon-20260806\AT-093\final-design.md`, SHA-256 `76B50F5B5879BA5AEC129C9941C96D61B8432B853E02DCBB87B1C6BB6F0CB7AE`.
- Opus v2: `C:\Users\93rob\.codex\delegations\lucidity-hackathon-20260806\AT-094\final-design-v2.md`, SHA-256 `2990F41FB0CC19F0E26248494E966352AEE801937704B258ACD8E9B1C73A0D7A`.
- Controller adjudication: `.orchestrator/REVIEWS/AT-094-opus-design.md`.

The product remains a native Rust/WezTerm application. It does not add Electron, Tauri, a WebView, a second renderer, a general widget toolkit, WSL, tmux, external UI fonts, or runtime-downloaded visual assets.

## 2. Product signature

North star: **a flight-strip board bolted to a terminal**. The native harness TUI remains the chat surface. Lucidity adds a concise instrument panel around it.

The signature has two edges:

1. **State Rail:** a 3-logical-pixel column at the far left. Runtime state is encoded by shape, text, and color.
2. **Weld:** the one conversation attached to the visible terminal interrupts the sidebar/terminal seam with a 3-pixel achromatic bar. Attachment is structural and never reuses a state color.

The application/poly icon is the frozen **Weld Frame**, built on a 16×16 square grid with no curves:

- background: `surface.window` over `[0,16) × [0,16)`;
- state rail: `text.primary` rectangle `[2,5) × [2,14)`;
- top plate: `text.primary` rectangle `[5,12) × [2,5)`;
- bottom plate: `text.primary` rectangle `[5,12) × [11,14)`;
- terminal cursor: `text.primary` rectangle `[7,9) × [6,10)`;
- Weld: `text.primary` rectangle `[11,14) × [6,10)`.

The geometry scales from normalized coordinates and snaps to the target pixel grid. The checked-in Windows ICO contains at least 16, 24, 32, 48, 64, and 256 pixel planes; the same geometry is used for the title-bar `Poly`. It is achromatic and original, so it neither consumes a runtime-state color nor copies a vendor mark.

Binding anti-goals:

- no gradients, blur, glass, shadow, decorative alpha, cards, pills, avatars, hero art, emoji, or vendor-logo imitation;
- radius zero for all Lucidity chrome;
- no ambient/scheduled Lucidity animation;
- no general-purpose accent token; saturated color communicates runtime state only;
- no forced terminal palette.

## 3. Window anatomy and layout authority

One `AppLayout`, computed in physical pixels from logical inputs and current DPI, owns:

- window/client rectangle;
- 32-logical-pixel title/truth bar;
- sidebar and its 2-pixel composite seam;
- terminal viewport and its inner padding;
- optional modal/overlay rectangle;
- initial rows/columns, PTY resize, render bounds/origins, mouse-to-cell mapping, terminal selection, hyperlinks, scrollbar, cursor/IME position, drag/drop, Win11 non-client hit geometry, maximize/restore, resize increments, and DPI transitions.

Padding inflation is forbidden as a sidebar mechanism. Every existing `padding_left_top`-style consumer that affects terminal geometry must derive from the same `AppLayout` origin.

Logical geometry at 96 DPI:

| Token | Value |
|---|---:|
| title bar height | 32 |
| sidebar widths | 200 compact / 264 default / 320 wide |
| sidebar seam | 2 |
| terminal inset | left 8 / right 8 / top 6 / bottom 6 |
| section header | 24 |
| active row | 44; compact density 36 |
| settled row | 28; compact density 24 |
| action bar | 36 |
| minimum client | 600 × 320 |
| minimum button hit target | 28 × 28 |
| spacing scale | 4 / 8 / 12 / 16 / 24 |

Every logical chrome size is converted with the existing point idiom: `Dimension::Points(logical_px * 0.75)`. `Dimension::Pixels` and `Dimension::Cells` are forbidden for Lucidity chrome.

Responsive behavior is based on client width:

| Width | `Auto` sidebar |
|---|---|
| 1000 or more | configured width, default 264 |
| 820–999 | 200 |
| below 820 | hidden; explicit restore control at title-bar x=12 |

An explicit `Shown` preference is honored down to 600 pixels; an explicit `Hidden` preference remains hidden. Geometry saturates without negative values or overlap. Each real sidebar geometry transition performs exactly one PTY resize. The discarded 48-pixel icon rail must not be implemented.

List rows are virtualized by visible range. Off-screen history owns no render widgets or terminal state, and row rendering never reads transcript bytes. No partially drawn bottom row is emitted; the remainder uses the sidebar surface.

## 4. Hit coverage and terminal containment

`AppLayout` rectangles are half-open. `UIItem::hit_test` in the pinned source is inclusive, so the only accepted adapter is equivalent to:

```rust
fn chrome_item(r: RectPhys, kind: ChromeItem) -> UIItem {
    assert!(r.width() >= 1 && r.height() >= 1);
    UIItem {
        x: r.min_x(),
        y: r.min_y(),
        width: r.width() - 1,
        height: r.height() - 1,
        item_type: UIItemType::Chrome(kind),
    }
}
```

Chrome rectangles touch. Deliberate one-pixel gaps are forbidden because an uncovered pixel falls through to terminal dispatch and clamps to cell zero.

Paint/hit emission order, with reverse search making the last item most specific:

1. full-bleed title-bar and sidebar background hit items;
2. section headers;
3. rows;
4. row sub-actions;
5. action bar;
6. title controls;
7. caption buttons;
8. modal/scrim last.

Mandatory pure and native assertions:

- every physical point in title bar or sidebar resolves to a typed `ChromeItem`;
- adjacent chrome boundaries resolve deterministically to the later, more specific item;
- the rightmost seam pixel resolves to chrome and never calls terminal mouse dispatch;
- the first terminal content pixel maps to terminal cell `(0,0)`;
- chrome clicks never start terminal selection or emit TUI mouse reports;
- stock mode retains every stock `UIItemType` behavior;
- containment is proved with direct quad extents or transparent-chrome mode on both renderers, not opaque overdraw.

The terminal remainder band uses the user's resolved terminal background. The chrome/terminal boundary is a two-pixel composite: one chrome-side `border.seam` pixel and one terminal-side adaptive pixel chosen for at least 3:1 contrast against the actual adjacent resolved terminal background. AT-101 must prove the palette source and dark/light/mid-grey cases; no terminal palette override is permitted.

## 5. Title bar and Windows behavior

The title bar replaces both stock tab chrome and a separate session header. Left to right:

- optional hidden-sidebar restore control;
- original Lucidity poly mark and wordmark;
- truth line: adapter, short native/session identity, project, runtime, and real event tally when space permits;
- usage control: full bucket chip when client width is at least 900, otherwise a compact 36×28 logical-pixel `USG` text button;
- settings gear;
- minimize, maximize/restore, close-to-tray buttons.

The truth line is the persistent explanation surface; no tooltip machinery is required.

Agent window decorations must preserve integrated buttons and resize behavior. A typed title-bar drag item must reproduce native drag and double-click maximize/restore. The maximize button must register the correct screen rectangle so Win11 snap layouts appear. The caption cluster retains stock-compatible width and a full 32-logical-pixel hit height.

Window operations are distinct:

- **minimize:** generic native minimize hook; window becomes iconic and remains normally discoverable;
- **hide:** true hide, not minimize; no visible/taskbar window and `IsIconic == false`, mux/process survives;
- **close-to-tray:** product intent mapped by AT-108 to hide-preserving-mux;
- **tray Exit:** terminate only Agent-owned Jobs, persist, then explicit GUI close;
- **stock close:** unchanged stock semantics.

`wezterm-gui` must not depend on Lucidity protocol/product crates for these generic window operations.

## 6. Typography and tokens

Rule: **machine truth is monospaced; human chrome is Roboto**.

- Roboto Medium: wordmark and section labels.
- Roboto Regular: buttons, settings labels, prose, empty/error text.
- JetBrains Mono Regular: titles, project/cwd, ages, IDs.
- JetBrains Mono Medium: runtime tokens, percentages, counts.
- Symbols Nerd Font Mono: generic controls only.
- User-configured terminal font: terminal only.

Only bundled font assets are allowed. The implementation must prove Medium face resolution rather than assume it from the bundled file.

Text is truncated in the pure view model. Monospaced available-character budget is `max(0, ceil(available_px / physical_advance_px) - 2)`, including the ellipsis. Titles truncate at the tail; paths at the head. Runtime tokens never truncate.

Instrument Dark core tokens:

| Token | Value | Use |
|---|---|---|
| `surface.window` | `#0C0E11` | title/action bar |
| `surface.sidebar` | `#101317` | sidebar |
| `surface.rowHover` | `#1B2027` | hover plus visible actions |
| `surface.rowSelected` | `#1C222A` | selection plus achromatic marker |
| `surface.overlay` | `#14181D` | modal |
| `surface.inputField` | `#0B0D10` | modal input only |
| `surface.track` | `#232A33` | usage/toggle track |
| `surface.scrim` | black at 55% | settings/confirm only |
| `text.primary` | `#E6EAF0` | primary text and Weld |
| `text.secondary` | `#A6B0BD` | settled title/labels |
| `text.tertiary` | `#7C8895` | metadata |
| `text.disabled` | `#545E6A` | disabled with non-color cue |
| `border.hairline` | `#232A33` | decoration only |
| `border.seam` | `#67727F` | chrome-side seam/outline |
| `border.focus` | `#F0F3F7` | achromatic focus ring |

No `accent` token exists. Selection, focus, attachment, and runtime state each have separate carriers. The Instrument High Contrast variant promotes secondary text, outlines rows, and widens the focus ring; it does not claim to honor Windows High Contrast in the MVP.

## 7. Organization, runtime, and row anatomy

Organization and runtime are independent axes:

- organization: `Active` or `Settled`;
- runtime: `NotRunning`, `Starting`, `Working`, `WaitingForInput`, `AwaitingApproval`, `CompletedIdle`, `Failed`, `UnknownExternal`.

Active rows are 44 logical pixels with mark/title and a meta line containing project, age, real event tally, and right-aligned state token. Settled rows are 28 logical pixels, one line, and retain the runtime cap/token. Settling never stops, detaches, changes runtime, or hides an attention state.

Runtime presentation:

| Runtime | Token | Color | State Rail cap |
|---|---|---|---|
| NotRunning | `IDLE` | `#7C8895` | centered hollow cap |
| Starting | `START` | `#58A6FF` | solid top third |
| Working | `WORK` | `#58A6FF` | full solid plus event-driven tick |
| WaitingForInput | `NEEDS` | `#E3B341` | full with one notch |
| AwaitingApproval | `APPR` | `#FF8A4C` | full with two notches |
| CompletedIdle | `DONE` | `#56D364` | centered solid tick |
| Failed | `FAIL` | `#F85149` | full plus cross |
| UnknownExternal | `EXT?` | `#9BA8B7` | dashed |

State is never color-only: token and cap shape are always present. The Working tick advances only when a real structured event arrives; it never advances on a timer.

The unique attached row draws the achromatic Weld. There is at most one Weld across Active and History. Stable identity is `ConversationId`; live activation carries a `TabId` resolved at dispatch, never a positional index.

History has a fixed header. When settled rows need attention, it names only the strongest present class, priority `APPR > NEEDS > FAIL > WORK`, with token, cap shape, color, and count. The counter is actionable and focuses the first matching durable row; it never re-sorts or unsets the row.

Orthogonal modifiers include pending identity, stale/uncertain evidence, resync required, opening, stopping, row action error, selection, keyboard focus, current attachment, and context pressure. A confirmed native A→B transition keeps exactly one row pending before confirmation and exactly one Weld after rebinding.

Runtime freshness is evidence-derived. A live Agent-owned pane/process is current until typed process/pane evidence changes it. A new `HostInstanceId` invalidates prior attachments immediately. External/imported observations remain `EXT?`. No universal time-only runtime-stale timer is allowed. Usage freshness is separate and defaults to five minutes for stale-on-open refresh.

## 8. Keyboard, focus, mouse, and IME

Priority:

1. open `LucidityModal`;
2. chrome focus layer;
3. stock effective key tables/input map;
4. terminal/TUI.

Agent-mode default global chords:

- `Ctrl+Shift+A`: focus sidebar list; toggle back to terminal if already focused;
- `Ctrl+Shift+B`: show/hide sidebar.

They are defaults, not universally free keys. User keymaps may remap/disable them. Effective-map materialization must detect conflicts and preserve an explicit user binding unless the user opts into Lucidity's chord.

No other global product shortcut exists. In particular, global F6, Ctrl+Shift+N/F/U/K/R/Space, and a raw-TUI pass-through mode are forbidden. `Alt+Space` remains the Windows system menu.

While chrome owns focus, Tab/Shift+Tab and F6/Shift+F6 may cycle chrome targets. The terminal is not in that cycle; Escape returns to the terminal. Ordinary chrome navigation/actions are consumed and never leak into the TUI. Explicitly tested stock actions, including copy/paste and normal stock window/tab/font/palette actions, are forwarded through the effective stock map; the allowlist must be validated against actual action resolution.

List focus is stored by `ConversationId`. Arrow/Home/End/Page keys navigate; Enter opens/focuses and returns to terminal; Space or S settles/unsettles; X opens stop confirmation. No destructive one-keystroke action is allowed.

Escape hierarchy:

1. pop internal modal view; close/restore invoker when empty;
2. chrome focus returns to terminal;
3. terminal-focused Escape is delivered verbatim to the TUI.

Terminal selection is not cleared by chrome clicks. Focus rings appear only for keyboard-visible focus and disappear after mouse press. IME composition with chrome focus and no text field must be natively tested before the key path is accepted. If composition leaks to the pane, the focus implementation must use the existing modal isolation seam or another proven equivalent. Cursor/IME and mouse mapping share the same `AppLayout` origin and border inputs.

## 9. Modal, usage, and settings

The source has one modal slot. Lucidity owns one `LucidityModal` with an internal stack of views: Usage, Settings(section), Confirm(kind), Select(field). Calling `set_modal` while Lucidity's modal is already open is forbidden.

Usage:

- always reachable through the single semantic/focus target `lucidity.titlebar.usage`: at client width 900 or greater it renders the named bucket/value chip; below 900 it renders a compact 36×28 logical-pixel `USG` text button in the same title-bar slot. The compact control carries no percentage, so it never presents an unlabeled or ambiguous value;
- activating either visual form opens `LucidityModal::Usage`; Escape/close returns focus to the exact triggering form, and a resize while open preserves the semantic focus target across the visual swap;
- account quota and current-conversation context are separate labelled groups and never combined;
- numeric values accompany every bar;
- opening refreshes only when older than 300 seconds; manual Refresh always requests one update;
- no polling while open or closed, no idle provider call, and no hidden TUI command injection;
- loading preserves/dims last good values; stale labels age/source; failure preserves last good values and shows age/source/error/retry; absent provider is explicit.

Settings/Confirm use one black 55% scrim quad plus a full-window scrim hit item so no click leaks to the terminal. Usage is a non-scrim popover. The settings sheet uses proportional insets and remains operable at 600×320. Four sections ship: General, Agents, Appearance, Keys (read-only initially). Below a 560-pixel sheet width, navigation becomes an internal Select view.

Only Toggle, Select, and Stepper control types ship, plus one modal free-text field for an adapter executable path. Validation occurs on commit, invalid values are not applied, and controls clearly label live/session/restart scope.

The per-row context-pressure marker may be fixture-only in the first viewport slice but must be wired to trusted live context data in Wave 2; it is not a permanent cut.

## 10. Performance and invalidation

Lucidity chrome has zero transitions and never calls `update_next_frame_time`. It does not use `InheritableColor::Animated`, a progress spinner, hover-driven tree rebuild, or a leader-key timer.

The computed chrome tree is cached and invalidated only by:

- catalog/conversation/runtime/usage/adapter data change;
- key action that changes chrome state;
- resize, DPI, or window-state change;
- window focus change;
- config reload;
- atlas regrow/shape-cache clear.

Hover invalidates the window only; colors resolve at render time from current mouse state. `show_tab_bar = false` removes the stock tab-bar progress scheduling path only after real OS-input sidebar activation is proven. Agent-mode materialized config pins no leader.

The honest metric is zero **additional** idle frames/CPU over the matched stock configuration. Cursor blink, visual bell, animated terminal images, and other stock terminal-side sources are not relabeled as Lucidity work.

## 11. Deterministic fixtures and semantic IDs

Durable semantic prefixes:

- `lucidity.titlebar`, `.identity`, `.usage`, `.restore`, `.settings`, `.button.{minimize,maximize,close}`;
- `lucidity.sidebar`, `.section.active`, `.section.history`, `.section.history.attention`;
- `lucidity.sidebar.row.{ConversationId}` and `.state`, `.agent`, `.attach`, `.action.{settle,unsettle,stop}`;
- `lucidity.sidebar.action.new`;
- `lucidity.modal.{usage,settings,confirm}`;
- `lucidity.notice.{offline,recovery,resync,row}`;
- `lucidity.terminal`.

These are pure view-model semantics and fixture IDs. The MVP does not claim Windows UI Automation, IAccessible, or screen-reader support. Visual and logical order must agree.

Required fixture families:

- all 16 organization/runtime combinations;
- selected, focused, hovered, attached, pending identity, stale/uncertain, opening, stopping, row error, partial data, long title, context pressure;
- fresh launch, working event tally, hide/reopen preservation, pending and confirmed `/new`, settled-running/approval attention, usage fresh/stale/error, history reopen, settings default/compact, failure, recovery/resync, inactive, high contrast;
- client frames 600×320 hidden, 900×600 compact, 1280×800 default, 1920×1080 default, and maximized OS-reported client size;
- F2 at 96/120/144/192 DPI, but terminal row/column goldens only after independent pinned-base capture at each scale;
- seam against dark, light, and mid-grey user terminal backgrounds.

Screenshots are evidence only when paired with deterministic state/geometry assertions. Fixture palette may be pinned; product palette may not.

## 12. Native acceptance gates

Before Gate 1 is accepted, evidence must cover:

- pinned-base cell metrics at 96, 120, 144, and 192 DPI where the environment can expose those scales;
- 96→144→96 logical/physical round trip without cumulative chrome loss or oscillation;
- real mouse, wheel, selection, hyperlink, clipboard, cursor, both IME preedit modes, wide glyphs/images, splits, tiny windows, maximize/restore, cross-monitor DPI;
- title drag, double-click maximize, maximize-button Win11 snap flyout;
- explicit minimize versus true hide and same-pane/process reopen;
- no chrome hit holes and no terminal bleed in both renderers;
- Medium font face resolution;
- hover without chrome tree rebuild;
- zero additional idle frames/CPU and no spinner/leader scheduling;
- stock binary geometry/input/close parity at the pinned base.

Evidence generated by the same formula under test cannot serve as its oracle. Unknown behavior remains an open gate rather than an inferred pass.

## 13. Task ownership and sequencing

- AT-105 owns product-agnostic true hide, explicit minimize, show/focus/explicit close, reconciliation suppression, and stock parity.
- AT-101 owns GUI library extraction, `AppLayout`, renderer/hit routing, title bar/sidebar skeleton, responsive geometry, focus/IME, snap/drag, fonts, and Gate 1 evidence.
- AT-102 owns host/catalog/runtime/IPC and evidence-derived freshness data.
- AT-201 owns the complete pure sidebar/query model.
- AT-203 owns live usage/context and the per-row context-pressure warning.
- AT-108 maps host close intent to generic GUI seams and proves tray/reopen/Quit.

No task may widen these leases because this document names a downstream feature. Contract changes stop for an ADR and controller adjudication.
