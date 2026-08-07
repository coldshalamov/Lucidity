# Lucidity V2 — Initial Parallel Task Packets

These packets implement the first corrective gates. They assume the canonical `LUCIDITY_V2_RECOVERY_PLAN.md` has been committed and the baseline `b215b572dbe778fa634a9ee973a7d0d3ebab18c8` tagged.

---

# LUI-000 — Baseline, supersession, and demo-mode cleanup

**Recommended owner:** Conductor/integration session  
**Objective:** Establish a clean recoverable V2 baseline without changing product architecture.

## Allowed paths

```text
docs/**
.orchestrator/**
agent-terminal/src/main.rs
agent-terminal/src/chrome/mod.rs
agent-terminal/src/chrome/app.rs
packaging/**
scripts/**
```

## Forbidden paths

```text
wezterm-gui/src/termwindow/**
agent-protocol/**
agent-backends/**
```

## Work

1. Tag baseline commit.
2. Create `lucidity/v2-gui-shell`.
3. Commit the V2 plan.
4. Add `docs/SUPERSEDED_PLANS.md` naming the old hackathon plans as historical.
5. Add a release/demo switch:
   - packaged release does not auto-create the mock runtime;
   - `--demo` or `LUCIDITY_DEMO=1` enables deterministic mock behavior.
6. Capture:
   - screenshot of current UI;
   - release binary size;
   - cold startup time;
   - idle CPU over 60 seconds;
   - RSS with the current mock.
7. Verify the existing package remains runnable.

## Acceptance

- baseline tag exists;
- V2 branch exists;
- current package builds;
- ordinary launch no longer auto-seeds mock;
- explicit demo launch still works;
- measurements recorded in `docs/baselines/B215_BASELINE.md`.

---

# LUI-101 — Embed egui into WezTerm WebGPU product mode

**Recommended owner:** Separate Codex / GPT-5.6 Sol Max implementation session  
**Objective:** Prove a real GUI library can coexist with the existing terminal in one window and render pass.

## Sole-owned hot zone

```text
Cargo.toml
Cargo.lock
wezterm-gui/Cargo.toml
wezterm-gui/src/lib.rs
wezterm-gui/src/product_runtime.rs
wezterm-gui/src/product_ui/**
wezterm-gui/src/termwindow/mod.rs
wezterm-gui/src/termwindow/app_layout.rs
wezterm-gui/src/termwindow/keyevent.rs
wezterm-gui/src/termwindow/mouseevent.rs
wezterm-gui/src/termwindow/render/draw.rs
wezterm-gui/src/termwindow/webgpu.rs
```

No other implementation agent may edit these paths until Gate 1 is accepted.

## Dependencies

Pin:

```text
egui = 0.32.3
egui-wgpu = 0.32.3
egui-extras = 0.32.3
```

Do not add:

```text
eframe
egui-winit
Tauri
WebView
```

## Work

1. Add generic `ProductUiFactory` to `ProductGuiHooks`.
2. Add `ProductUiController` contract.
3. Add per-window `ProductUiHost`:
   - context;
   - renderer;
   - raw input state;
   - platform output;
   - controller.
4. Force `FrontEndSelection::WebGpu` only in product mode.
5. Modify the WebGPU draw path:
   - terminal draw remains first;
   - egui buffer/texture updates use existing device/queue/encoder;
   - egui renders in second pass with `LoadOp::Load`;
   - one submit and one present.
6. Render a temporary integration test panel:
   - button;
   - text field;
   - dropdown;
   - scroll area;
   - popover over terminal.
7. Implement repaint scheduling without a permanent animation loop.

## Required tests

- dependency tree contains one wgpu 25 generation;
- product mode selects WebGPU;
- stock WezTerm build path remains unaffected;
- ProductUiHost is absent in stock mode;
- texture set/free lifecycle;
- resize screen descriptor;
- DPI conversion.

## Manual verification

- button click;
- text typing;
- copy/paste;
- dropdown;
- scroll;
- popover;
- terminal output visible;
- terminal resize;
- no second window;
- no flicker/clear between passes.

## Stop condition

If a same-surface implementation cannot satisfy the complete Gate 1 interaction within the bounded spike, do not improvise another hand-painted widget system. Write an ADR comparing the observed blocker with the child-HWND fallback.

---

# LUI-102 — Product UI input, focus, clipboard, and IME

**Recommended owner:** Same LUI-101 owner; do not parallelize in the hot zone  
**Objective:** Establish correct arbitration between GUI controls and the terminal.

## Allowed paths

Same hot zone as LUI-101.

## Work

1. Offer mouse events to ProductUiHost before requiring an active pane.
2. Route pointer events by terminal/UI region.
3. Translate terminal coordinates relative to the actual terminal rectangle.
4. Offer key events to GUI when:
   - text field focused;
   - modal/popover open;
   - global shortcut matched.
5. Preserve terminal raw/composed input otherwise.
6. Implement clipboard bridge.
7. Implement IME cursor rectangle and composition.
8. Implement cursor icon and URL output.
9. Add default outer shortcut arbitration.

## Required manual matrix

```text
GUI text entry
GUI Ctrl+C/Ctrl+V
Terminal Ctrl+C with no selection
Terminal Ctrl+C with selection
Terminal Ctrl+V bracketed paste
Escape closes GUI popup first
Escape reaches terminal otherwise
Terminal mouse selection
Alternate-screen mouse application
Unicode
IME
125/150/200% DPI
```

## Acceptance

No visible GUI control requires terminal focus tricks, and no terminal interaction is broken by the GUI layer.

---

# LUI-110 — UX specification and screenshot acceptance matrix

**Recommended owner:** Cursor / Grok 4.5 High  
**Mode:** Read-only research/design; no renderer code  
**Objective:** Define the actual desktop interface before implementation drifts.

## Allowed paths

```text
docs/ui/**
```

## Forbidden paths

```text
wezterm-gui/**
agent-terminal/src/**
agent-protocol/**
```

## Inputs

- user's original product description;
- current screenshot;
- T3 Code SidebarV2/settings source;
- OpenCode Desktop/Hermes references;
- current Lucidity capabilities.

## Deliverables

### `docs/ui/LUCIDITY_UI_SPEC.md`

Must specify:

- main window regions;
- sidebar dimensions;
- row anatomy;
- agent icon treatment;
- active/history behavior;
- header;
- new-session dialog;
- settings navigation and rows;
- usage popover;
- hover/focus/pressed/error/loading states;
- narrow window behavior;
- keyboard behavior;
- empty/error states.

### `docs/ui/SCREENSHOT_MATRIX.md`

Deterministic scenes at required dimensions/DPI.

### `docs/ui/INTERACTION_ACCEPTANCE.md`

Exact click-by-click scripts.

## Constraints

- proportional application font;
- native TUI terminal remains center;
- no terminal-styled product chrome;
- no large custom chat renderer;
- no invented backend features as substitutes for visible controls.

## Review

Fable reviews this spec before LUI-201.

---

# LUI-120 — Event-driven UI bridge and cached view model

**Recommended owner:** Native Grok / Grok 4.5 High  
**Objective:** Stop querying persistence from the render path and remove the fixed 25 ms action pump.

## Allowed paths

```text
agent-terminal/src/chrome/app.rs
agent-terminal/src/ui_bridge.rs
agent-terminal/src/host/**
agent-terminal/src/runtime/**
agent-terminal/src/lib.rs
agent-protocol/src/lib.rs          # only agreed UI bridge contracts
agent-terminal/tests/**
```

## Forbidden paths

```text
wezterm-gui/src/termwindow/**
wezterm-gui/src/product_ui/**
lucidity-ui/**
```

## Work

1. Define immutable `UiSnapshot`.
2. Store `RwLock<Arc<UiSnapshot>>`.
3. Define `UiCommand`.
4. Add nonblocking UI command sender.
5. Rebuild view only on relevant state revision.
6. Replace `ACTION_PUMP_INTERVAL` loop with channel/event-driven wake.
7. Issue `request_product_redraw()` only when published view changes.
8. Add paginated history request shape, but do not implement harness importers yet.
9. Ensure callbacks never block the GUI thread.

## Tests

- snapshot pointer remains stable when no state changes;
- no catalog query on repeated render reads;
- UI command ordering;
- event revision rebuild;
- no lost mux events;
- idle host thread blocks;
- tray wake;
- process exit wake;
- graceful shutdown.

## Acceptance

Release build shows no periodic 25 ms wake when idle.

---

# LUI-201 — Build the actual Lucidity shell

**Recommended owner:** LUI-101 Sol implementation session  
**Depends on:** LUI-101, LUI-102, accepted LUI-110, LUI-120 interface  
**Objective:** Replace the current product chrome with the real desktop shell.

## Allowed paths

```text
lucidity-ui/**
agent-terminal/src/chrome/mod.rs
agent-terminal/src/chrome/app.rs       # bridge wiring only
wezterm-gui/src/product_ui/**
wezterm-gui/src/termwindow/app_layout.rs
assets/ui/**
```

Hot-zone edits remain single-owner.

## Work

1. Add `lucidity-ui` crate.
2. Add theme and proportional font.
3. Use native Windows title bar; delete internal fake title bar from product layout.
4. Build:
   - sidebar;
   - header;
   - welcome route;
   - session route;
   - settings route shell;
   - active/history list;
   - filter/search;
   - footer.
5. Render adapter icons from assets.
6. Use virtualized rows.
7. Implement keyboard focus order and tooltips.

## Acceptance screenshots

- welcome;
- one active session;
- multiple active sessions;
- history expanded;
- narrow/collapsed sidebar;
- settings shell.

## Prohibited shortcut

Do not draw these controls using WezTerm's terminal line renderer.

---

# LUI-202 — New session dialog and profile picker

**Recommended owner:** Same UI owner or a second agent restricted to `lucidity-ui/dialogs/**` after interfaces freeze  
**Objective:** Create sessions through a normal GUI workflow.

## Work

1. `+ New` primary button.
2. quick-launch dropdown.
3. adapter/profile cards.
4. recent projects.
5. native Windows folder picker.
6. optional title.
7. optional launch args.
8. missing executable/configure state.
9. validation and loading/error state.
10. dispatch `UiCommand::NewConversation`.

## Acceptance

A user can create a mock, Claude, Codex, or other configured session without typing a shell command or knowing an executable path.

---

# LUI-203 — Session row actions and history organization

**Recommended owner:** Lucidity UI owner  
**Objective:** Implement the sidebar workflow that motivated the product.

## Work

- click open/focus/resume;
- Settle;
- Unsettle;
- Stop/X;
- Resume/play;
- rename;
- context menu;
- active/history sections;
- runtime status remains visible after settle;
- search/filter;
- keyboard selection;
- empty/error states.

## Invariants

- Settle does not stop.
- Stop does not settle.
- Stop retains the row.
- History rows allocate no PTY.
- click never spawns duplicate runtime if attachment exists.

## Acceptance

Run the complete Gate 2 mock workflow.

---

# LUI-210 — Gate 2 adversarial UI review

**Recommended owner:** Claude / Fable 5 Extra High  
**Mode:** Review only  
**Objective:** Prevent architecture receipts from being mistaken for the requested product again.

## Inputs

- packaged application;
- current and new screenshots;
- UI spec;
- user product charter;
- code diff.

## Required review

- visual desktop-app legibility;
- actual click behavior of every visible action;
- focus and keyboard navigation;
- terminal input regression;
- settle/stop independence;
- narrow/DPI behavior;
- error states;
- performance red flags;
- missing acceptance steps.

## Verdict

`ACCEPT`, `ACCEPT WITH REQUIRED FIXES`, or `REJECT`.

No “mostly complete” verdict.

---

# LSET-301 — App and terminal settings

**Recommended owner:** Sol or native Grok after Gate 2  
**Objective:** Implement real settings, not only schemas.

## Work

- app settings file;
- atomic write/backup;
- terminal typed settings;
- map to WezTerm config overrides;
- live reload;
- Appearance and Terminal pages;
- reset controls;
- restart-required labels.

## Acceptance

Mouse-driven font, size, color scheme, cursor, padding, scrollback, and copy/paste settings work in the packaged app.

---

# LSET-302 — Keybindings

**Recommended owner:** OpenCode/GLM for tests + Sol for integration  
**Objective:** Provide graphical control of Lucidity and terminal shortcuts.

## Work

- key recorder;
- conflict detector;
- scopes;
- apply/reset;
- terminal/UI focus-aware defaults.

## Acceptance

Record, conflict, save, apply, reset.

---

# LSET-303 — Agent profiles and native settings bindings

**Recommended owner:** Native Grok or Kimi  
**Objective:** Turn minimal adapter manifests into configurable user profiles.

## Work

- profile persistence;
- executable/version test;
- launch/resume args;
- environment values;
- config root;
- settings schema rendering;
- safe JSON/JSONC/TOML binding engine;
- backup/rollback.

## Acceptance

Configure two profiles for one adapter and launch each correctly.

---

# LCAT-401 — Codex history importer

**Recommended owner:** Kimi K3 Max or Sol  
**Objective:** Import and resume Codex native conversations.

## Constraints

- primary sources/installed version only;
- metadata-first;
- paginated;
- no transcript load for list;
- imported rows settled;
- authoritative native IDs.

## Acceptance

Existing Codex threads appear, filter correctly, and resume the intended thread.

---

# LCAT-402 — Claude history importer and event bridge

**Recommended owner:** Different model from LCAT-401  
**Objective:** Import Claude history and correlate native session changes.

## Acceptance

Existing Claude sessions appear; app-created session receives authoritative ID; `/new` rebinds pane to new row.

---

# LUSE-501 — Account usage and conversation context

**Recommended owner:** GLM for schema/tests, Sol/Grok for provider implementations  
**Objective:** Implement real usage/context data and UI.

## Work

- protocol types;
- migrations;
- Codex structured account provider;
- one TUI provider;
- usage popover;
- context bar;
- stale-on-open;
- failure retention.

## Acceptance

Account and context display separately and refresh without background spam.

---

# LPERF-601 — Performance and accessibility hardening

**Recommended owner:** Native Grok + Fable review  
**Objective:** Meet measured lightweight behavior.

## Work

- profile render and host loops;
- eliminate frame-time DB work;
- virtualized 10k list;
- AccessKit;
- keyboard-only;
- release benchmarks;
- DPI matrix;
- terminal conformance.

## Acceptance

Evidence recorded against both stock WezTerm and `b215` baseline.

---

# Integration sequence

```text
LUI-000
  ↓
LUI-101 → LUI-102
  ↓          ↘
LUI-110       LUI-120
   \           /
    → LUI-201 → LUI-202 → LUI-203 → LUI-210
                    ↓
       LSET-301 / LSET-302 / LSET-303
                    ↓
       LCAT-401 / LCAT-402 / additional importers
                    ↓
                  LUSE-501
                    ↓
                  LPERF-601
```

The desktop GUI vertical slice is the spine. Backend breadth never outranks it again.
