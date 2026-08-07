# Lucidity V2 — Product Recovery and Completion Plan

**Status:** Canonical replacement plan  
**Baseline repository:** `coldshalamov/Lucidity`  
**Baseline commit:** `b215b572dbe778fa634a9ee973a7d0d3ebab18c8`  
**Primary platform:** Native Windows  
**Product foundation:** Existing Rust host + existing WezTerm fork  
**License target:** MIT  
**Supersedes:** The prior `HACKATHON/AGENT_TERMINAL_*` implementation plans as execution authority

---

## 0. The single architectural correction

Lucidity must be a **real desktop GUI application containing a WezTerm terminal surface**.

It must not continue as WezTerm drawing product labels and sidebar text with its terminal-oriented quad/text machinery.

The existing terminal, mux, process, catalog, tray, IPC, and adapter work is retained. The current hand-painted product chrome is replaced by a proper Rust GUI layer rendered into the same native window and GPU surface.

The selected implementation path is:

> **Embed `egui` 0.32.3 directly into Lucidity's existing WezTerm WebGPU render loop, using `egui-wgpu` 0.32.3. Do not use `eframe`, `winit`, a WebView, Electron, or a second desktop event loop.**

This choice is not an ideological restriction. It is the lowest-risk route from the code that already exists:

- Lucidity's workspace already uses `wgpu 25.0.2`.
- `egui-wgpu 0.32.3` uses `wgpu 25.0.0`, so the GUI can share WezTerm's existing device, queue, surface, and command encoder without introducing a second incompatible `wgpu` generation.
- `egui-wgpu` is explicitly designed to render into an existing `wgpu::RenderPass`.
- The current fork has already separated the terminal viewport from surrounding product space and already owns session switching through the WezTerm mux.
- This preserves the hard work and replaces only the wrong presentation/input layer.

`egui` is the application UI library. WezTerm remains the terminal emulator.

---

# 1. Product charter: what Lucidity actually is

This section is authoritative. Agents may not reinterpret it into a more convenient product.

## 1.1 Core product

Lucidity is a lightweight Windows desktop application for people who use native coding-agent terminal harnesses but do not want to manage those harnesses as anonymous terminal windows.

The native agent TUI remains the conversation interface. Lucidity adds the GUI that the native TUIs are missing:

- a polished session sidebar;
- one-click creation and switching between conversations;
- durable cross-harness conversation history;
- clear runtime status;
- stop, resume, settle, rename, search, and filter controls;
- graphical terminal settings;
- graphical coding-agent settings;
- account-usage and context visibility;
- close-to-tray persistence;
- a modular adapter format for adding harnesses;
- a thin Cursor/VS Code client later;
- phone control later.

It is not another coding harness. It does not replace Claude Code, Codex CLI, Kimi Code, OpenCode, Grok, Cline, or their slash commands.

## 1.2 The user experience in one sentence

> Open one app, see every configured coding-agent conversation in a normal clickable GUI, click one to use that harness's real native TUI inside an embedded WezTerm terminal, and manage the painful parts—threads, settings, usage, and recovery—without terminal bookkeeping.

## 1.3 Non-negotiable behavior

1. **Native TUI preservation**
   - The terminal content is the actual harness.
   - Slash commands, goals, model controls, plugins, permissions, subagent views, and future harness features remain available immediately.

2. **Sidebar conversations are durable identities**
   - A row represents one native harness conversation.
   - A live WezTerm pane is only the current runtime attachment.
   - Closing, stopping, or crashing a pane must not erase the conversation row.

3. **Runtime state and organization state remain independent**
   - `Settle` is organizational only.
   - A settled conversation may still be working.
   - A stopped conversation may remain active in the upper working set.
   - The row X/stop action stops the runtime but does not settle or delete the conversation.

4. **No WSL requirement**
   - Native Windows is the first product.
   - WSL may be an optional execution target later, never a prerequisite.
   - No tmux installation is required; WezTerm's mux remains the underlying session mechanism.

5. **Real GUI interaction**
   - `+ New`, agent picker, settings, usage, row actions, menus, filters, dialogs, text fields, toggles, and sliders must be real mouse- and keyboard-operable GUI controls.
   - Rendering a label or rectangle does not count as implementing a control.
   - A schema does not count as implementing a settings page.

6. **Close to tray**
   - Closing the main window keeps the tray host and owned agent runtimes alive.
   - Tray Exit is the real application quit path.

7. **Lightweight**
   - No browser runtime is required for the desktop app.
   - No continuous polling just to keep the GUI current.
   - Historical conversations allocate no PTY.
   - Off-screen conversation transcripts are not loaded to render the sidebar.

## 1.4 Explicitly not the product

Lucidity is not:

- a terminal color theme;
- a TUI sidebar drawn with terminal-looking text;
- a rebranded stock WezTerm window;
- a custom chat renderer;
- an agent subscription;
- a second copy of each harness's session database;
- an IDE;
- a general terminal replacement trying to compete with every Warp feature;
- a requirement that users understand tmux, mux domains, shell quoting, or session IDs.

---

# 2. Definition of complete

No agent may describe Lucidity as complete until this exact workflow works in the packaged Windows application.

## 2.1 Required end-to-end workflow

1. Launch `Lucidity.exe`.
2. See a conventional desktop GUI with:
   - native Windows title bar;
   - polished resizable sidebar;
   - `+ New` button;
   - search/filter controls;
   - active and history sections;
   - settings button;
   - a welcome/empty content surface if no conversation is selected.
3. Click `+ New`.
4. Receive a GUI agent picker showing configured harness profiles and icons.
5. Choose an agent and a project directory using a native folder picker.
6. Create the session.
7. See a new sidebar row with:
   - agent icon;
   - conversation title;
   - project/agent metadata;
   - runtime status.
8. See the real agent TUI in the embedded WezTerm content area.
9. Create at least one additional session and switch between both by clicking sidebar rows.
10. Hover a row and use:
    - Settle;
    - Stop/X;
    - overflow/context menu.
11. Verify Settle moves the row to History without stopping a running agent.
12. Verify Stop kills the runtime but leaves the row available to resume.
13. Click a stopped row and resume the same native conversation.
14. Open Settings with a mouse.
15. Change a terminal setting using a GUI control and observe it apply.
16. Change an agent profile's executable/arguments using GUI controls.
17. Open Usage and see separate account-limit and conversation-context surfaces, or a clear provider-unavailable state.
18. Close the window, leave the session running in the tray, reopen, and return to the same terminal.
19. Restart Lucidity and find imported/history conversations without manually remembering session IDs or start times.
20. Use a native `/new` or equivalent command in a supported harness and see the newly created native conversation appear/rebind in the sidebar once its identity is authoritatively confirmed.

## 2.2 Required visual standard

The application chrome must read immediately as a desktop application comparable in interaction density to T3 Code, OpenCode Desktop, or Hermes Desktop.

At minimum:

- proportional UI font for chrome;
- separate monospace terminal font;
- real icons;
- hover, pressed, focus, disabled, selected, error, and loading states;
- normal spacing and alignment;
- menus and popovers;
- no duplicate fake title bar;
- no all-monospaced, all-uppercase terminal aesthetic across the GUI;
- no status tokens such as `WORK`, `EXT?`, or `APPR` as the primary human-facing design;
- no decorative action bar whose click behavior is a no-op.

A successful build and passing Rust tests are necessary but not sufficient.

---

# 3. Current repository audit

Baseline: `b215b572dbe778fa634a9ee973a7d0d3ebab18c8`.

## 3.1 Keep

These are useful foundations and should not be rewritten without a specific defect:

### WezTerm terminal and mux integration

Keep:

- WezTerm terminal model and escape parser;
- font shaping and glyph atlas;
- PTY/ConPTY integration;
- GPU renderer;
- scrollback, selection, links, clipboard, alternate screen, mouse modes;
- mux tabs/panes and pane focus;
- product bootstrap through `run_product_with_hooks`;
- current terminal viewport/layout separation, after adapting it to the new UI host.

Relevant current areas:

- `wezterm-gui/src/lib.rs`
- `wezterm-gui/src/product_runtime.rs`
- `wezterm-gui/src/termwindow/app_layout.rs`
- `wezterm-gui/src/termwindow/render/draw.rs`
- `wezterm-gui/src/termwindow/webgpu.rs`

### Host and lifecycle

Keep:

- `ProductApplication`;
- catalog recovery and SQLite migrations;
- durable conversation IDs;
- runtime attachments;
- pending native identity and rebind logic;
- Windows Job Object ownership;
- tray controller;
- named-pipe host;
- extension event stream;
- current process cleanup and quit hardening.

Relevant current areas:

- `agent-terminal/src/catalog/**`
- `agent-terminal/src/host/**`
- `agent-terminal/src/ipc/**`
- `agent-terminal/src/runtime/**`
- `agent-terminal/src/windows/**`
- `agent-terminal/src/chrome/app.rs`

### Protocol foundation

Keep and extend:

- `ConversationId`, `ProfileId`, `AdapterId`;
- `OrganizationState`;
- `RuntimeState`;
- `RuntimeAttachment`;
- structured agent-event envelope;
- adapter manifest parsing;
- IPC versioning;
- existing conversation new/open/settle/stop requests.

Relevant current area:

- `agent-protocol/src/lib.rs`

### Adapter package foundation

Keep and extend:

- executable discovery;
- launch/resume command templates;
- settings-schema fields;
- settings-binding fields;
- usage/context provider hooks;
- fixture and hook concepts.

Relevant current area:

- `agent-backends/**`

### Cursor/VS Code extension plumbing

Keep as a later client:

- current named-pipe client;
- packaging;
- push event handling.

Relevant current area:

- `extensions/vscode/**`

## 3.2 Replace

These are the wrong abstraction and should be retired after the replacement vertical slice passes:

- `wezterm-gui/src/termwindow/render/product_chrome.rs`
- `ProductSidebarProvider`
- `ProductSidebarSnapshot` as the complete UI contract
- coarse `ChromeItem` hit regions for the product UI
- product UI text drawn through `render_screen_line`
- the internal fake 32-pixel title bar
- the current hand-built state rail and status tokens
- the current action bar with no action semantics
- the production behavior that automatically creates/opens a mock session at startup

The old chrome may remain behind a temporary compile-time feature during the migration. It must not remain the default product UI after Gate 2.

## 3.3 Finish or implement

The current code has placeholders or protocol shapes, but not the real feature:

- no normal `+ New` UI;
- no agent picker;
- no project picker;
- no graphical settings route;
- no graphical terminal settings;
- no graphical agent settings;
- no row Settle/Stop buttons;
- no context menu;
- no rename workflow;
- no search/filter UI;
- no agent icons;
- no real history importers in the bundled Claude/Codex manifests;
- no structured event capabilities in those manifests;
- no real usage provider—the current refresh request records `{"status":"refresh_requested"}`;
- no profile editor;
- no account-level usage model distinct from conversation context;
- no production-quality welcome, empty, launch-error, or missing-agent state;
- no product UI accessibility tree;
- no screenshot/interaction acceptance gate.

## 3.4 Performance debt to correct while replacing the UI

Two current behaviors should not survive into the finished shell:

1. `ProductApplication` wakes on a fixed 25 ms loop.
2. The current sidebar snapshot path queries SQLite while producing render state.

Replace both with:

- blocking/event-driven channels;
- cached immutable UI snapshots rebuilt only when underlying state changes;
- explicit `request_product_redraw()` when a new snapshot is published.

---

# 4. Target architecture

## 4.1 One process, one native window, one GPU surface

```text
agent.exe
│
├── ProductApplication
│   ├── CatalogStore / SQLite
│   ├── HostController
│   ├── Adapter registry
│   ├── Runtime/process ownership
│   ├── Tray
│   ├── Named-pipe service
│   ├── Usage/context providers
│   └── ProductUiBridge
│       ├── immutable cached UiSnapshot
│       └── UiCommand channel
│
├── LucidityUi
│   ├── egui application shell
│   ├── sidebar
│   ├── dialogs and popovers
│   ├── settings pages
│   └── usage/context surfaces
│
└── WezTerm TermWindow
    ├── ProductUiHost
    │   ├── egui::Context
    │   ├── egui_wgpu::Renderer
    │   ├── input/platform integration
    │   └── Box<dyn ProductUiController>
    │
    └── existing WezTerm terminal renderer
        └── currently selected mux pane
```

There is no second top-level GUI process and no child browser.

## 4.2 Render order

For product mode on Windows:

1. Obtain the current WezTerm WebGPU surface texture.
2. Render the WezTerm terminal into the terminal rectangle using the existing renderer.
3. Run the Lucidity egui frame.
4. Update egui textures and buffers using the same WebGPU device/queue.
5. Begin a second render pass with `LoadOp::Load`.
6. Render egui panels, controls, popovers, dialogs, and overlays.
7. Submit once and present once.

This allows dialogs and menus to overlay the terminal while leaving terminal rendering untouched.

## 4.3 Why not a separate outer window plus embedded child HWND

That remains a fallback, not the primary architecture.

The current fork already solved:

- single-window terminal clipping;
- pane focus;
- mux switching;
- product-mode lifecycle;
- tray ownership;
- shared process state.

Moving to a parent/child HWND design would reintroduce:

- two focus systems;
- child-window DPI coordination;
- IME/caret placement across HWND boundaries;
- z-order and clipping edge cases;
- extra process/window lifecycle;
- input forwarding;
- more difficult overlays over the terminal.

The same-surface egui path preserves more existing work and introduces fewer moving parts.

## 4.4 Bounded fallback gate

If the same-surface integration cannot satisfy **all** of these in the bounded integration spike, stop and write an ADR before proceeding:

- clickable button;
- text field with copy/paste;
- dropdown/popover;
- scroll area;
- correct DPI;
- terminal keyboard/mouse behavior preserved outside the GUI region;
- no duplicate `wgpu` major versions;
- one present per frame;
- no material idle CPU regression.

The fallback is a native Rust shell with a child WezTerm HWND, not a return to hand-painted chrome.

---

# 5. New crate and module structure

Add one app-specific GUI crate and one generic WezTerm integration module.

```text
lucidity-ui/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── app.rs
    ├── bridge.rs
    ├── model.rs
    ├── route.rs
    ├── theme.rs
    ├── assets.rs
    ├── components/
    │   ├── button.rs
    │   ├── icon.rs
    │   ├── session_row.rs
    │   ├── status_badge.rs
    │   ├── usage_bar.rs
    │   ├── setting_row.rs
    │   ├── key_recorder.rs
    │   └── empty_state.rs
    ├── sidebar/
    │   ├── mod.rs
    │   ├── active.rs
    │   ├── history.rs
    │   └── filters.rs
    ├── screens/
    │   ├── welcome.rs
    │   ├── session.rs
    │   ├── settings.rs
    │   └── launch_error.rs
    ├── dialogs/
    │   ├── new_session.rs
    │   ├── agent_profile.rs
    │   └── confirm_stop.rs
    └── settings/
        ├── general.rs
        ├── agents.rs
        ├── terminal.rs
        ├── appearance.rs
        ├── keybindings.rs
        ├── sessions.rs
        ├── usage.rs
        └── advanced.rs
```

Inside `wezterm-gui`:

```text
wezterm-gui/src/product_ui/
├── mod.rs
├── host.rs
├── input.rs
├── platform.rs
├── renderer.rs
└── tests.rs
```

Inside `agent-terminal`:

```text
agent-terminal/src/ui_bridge.rs
agent-terminal/src/settings/
agent-terminal/src/usage/
agent-terminal/src/importers/
```

---

# 6. Generic WezTerm product-UI contract

The WezTerm fork should know how to host a GUI, but not know Lucidity's business domain.

## 6.1 Add the factory to product hooks, not product identity

Keep `ProductGuiConfig` as plain identity/configuration data.

Extend `ProductGuiHooks` with a factory:

```rust
pub type ProductUiFactory =
    Arc<dyn Fn() -> Box<dyn ProductUiController> + Send + Sync + 'static>;

pub struct ProductGuiHooks {
    pub close_to_tray: bool,
    pub on_ready: Option<Arc<dyn Fn() + Send + Sync>>,
    pub on_window_hidden: Option<Arc<dyn Fn() + Send + Sync>>,
    pub ui_factory: Option<ProductUiFactory>,
}
```

This avoids putting non-comparable trait objects into `ProductGuiConfig`.

## 6.2 Controller contract

```rust
pub trait ProductUiController {
    fn layout_spec(&self) -> ProductLayoutSpec;

    fn show(
        &mut self,
        ctx: &egui::Context,
        frame: ProductUiFrame<'_>,
    ) -> ProductUiResponse;
}

pub struct ProductLayoutSpec {
    pub sidebar_width_points: f32,
    pub header_height_points: f32,
    pub bottom_bar_height_points: f32,
    pub terminal_visibility: TerminalVisibility,
}

pub enum TerminalVisibility {
    Visible,
    CoveredByOpaqueUi,
    HiddenPreserveSize,
}

pub struct ProductUiFrame<'a> {
    pub window_size_points: egui::Vec2,
    pub pixels_per_point: f32,
    pub terminal_rect_points: egui::Rect,
    pub focused: bool,
    pub now: Instant,
    pub window_ops: &'a dyn WindowOps,
}

pub struct ProductUiResponse {
    pub layout_changed: bool,
    pub terminal_focus_requested: bool,
    pub repaint_after: Option<Duration>,
}
```

## 6.3 Per-window `ProductUiHost`

Store one UI host in each product `TermWindow`:

```rust
struct ProductUiHost {
    context: egui::Context,
    renderer: egui_wgpu::Renderer,
    input: ProductUiInputState,
    controller: Box<dyn ProductUiController>,
    pending_textures: egui::TexturesDelta,
    last_platform_output: egui::PlatformOutput,
}
```

The stock WezTerm binary never creates this object.

## 6.4 WebGPU-only product path initially

Lucidity product mode sets:

```text
front_end = WebGpu
```

Stock WezTerm remains unchanged and may continue supporting OpenGL, WebGPU, and software modes.

Do not expose a Lucidity “renderer backend” setting until either:

- an `egui_glow` fallback is implemented for the Glium path; or
- the setting clearly explains that Lucidity currently requires WebGPU.

This is an implementation dependency, not a permanent product ideology.

---

# 7. Input, focus, clipboard, and IME routing

The input boundary is as important as rendering.

## 7.1 Priority order

Every Windows event follows this order:

1. Native window management.
2. Open Lucidity modal/popover.
3. Lucidity control with keyboard focus.
4. Lucidity global shortcut.
5. WezTerm terminal.

## 7.2 Mouse

Before obtaining an active pane, `mouse_event_impl` must offer the event to `ProductUiHost`.

This is required because settings/welcome screens must work even when no terminal pane exists.

Rules:

- If egui consumes the pointer event, do not send it to the terminal.
- If a Lucidity popup is open over the terminal, it owns the click.
- If the pointer is inside `terminal_rect` and no GUI overlay captures it, translate coordinates relative to the terminal rectangle and use existing WezTerm mouse behavior.
- Preserve terminal mouse-grab modes.
- Preserve text selection, links, scroll wheel, and alternate-screen mouse reporting.

## 7.3 Keyboard

Offer raw and composed key events to egui first when:

- a GUI text field is focused;
- a modal is open;
- a dropdown/menu is open;
- the key matches a Lucidity global shortcut.

Otherwise route to WezTerm.

Default shortcuts:

```text
Ctrl+N          New session
Ctrl+,          Settings
Ctrl+K          Focus sidebar search
Ctrl+Tab        Next active conversation
Ctrl+Shift+Tab  Previous active conversation
Ctrl+V          Paste into focused GUI field or terminal
Ctrl+C          Copy GUI text; in terminal copy selection, otherwise send ^C
Ctrl+F          Search sidebar/settings when GUI-focused; terminal search when terminal-focused
Escape          Close GUI modal first; otherwise pass to terminal
```

Provide a raw-terminal shortcut mode for advanced users.

## 7.4 Clipboard

Reuse WezTerm's existing clipboard platform integration.

- GUI copy output writes to the same Windows clipboard.
- GUI paste reads from the same clipboard.
- Terminal paste uses WezTerm's bracketed-paste-aware path.
- Clipboard decisions depend on current focus, not the agent brand.

## 7.5 IME and caret

Map egui's text-cursor rectangle to the existing window IME APIs.

Acceptance must include:

- ordinary Latin typing;
- Unicode composition;
- emoji;
- high-DPI caret placement;
- terminal IME unaffected when terminal has focus.

## 7.6 Cursor and URLs

Apply egui platform output through existing window services:

- cursor icon;
- open URL;
- copied text;
- IME rectangle;
- repaint scheduling.

---

# 8. Layout and routes

## 8.1 Remove the internal fake title bar

Use the normal Windows title bar for the first finished product.

The client area starts with the actual application layout. This removes the current double-title-bar appearance and avoids custom drag/maximize logic.

A custom integrated title bar can be a later polish task.

## 8.2 Main routes

```rust
enum MainRoute {
    Welcome,
    Session(ConversationId),
    Settings(SettingsPage),
}
```

The sidebar remains present on all routes unless the window is too narrow.

## 8.3 Default dimensions

These are initial design tokens, not immutable user constraints:

```text
Sidebar default: 288 logical px
Sidebar minimum: 232
Sidebar maximum: 420
Content header: 52
Sidebar session row: 52
History row: 38
Corner radius: 8
Small gap: 6
Normal gap: 10
Section gap: 16
```

The divider is draggable and persists its width.

## 8.4 Terminal layout

When `MainRoute::Session`:

```text
window client
├── sidebar
└── content
    ├── session header
    └── terminal rect
```

When `MainRoute::Settings`:

```text
window client
├── settings navigation/sidebar
└── settings content
```

Initially, settings may draw an opaque GUI surface over the still-attached terminal and block terminal input. Later optimization may skip terminal painting while preserving its last size.

## 8.5 Responsive behavior

- At narrow widths, sidebar collapses to an icon rail.
- A visible button restores it.
- The session content remains usable.
- Do not silently turn the GUI back into terminal-like labels.

---

# 9. Visual and interaction specification

## 9.1 Typography

Application chrome:

- proportional font;
- use an app-owned open font such as DM Sans/Inter, or resolve Segoe UI through the existing font stack;
- fallback cleanly.

Terminal:

- independent user-configured monospace font.

Use monospace only for paths, command previews, IDs, and technical values.

## 9.2 Sidebar

### Header

- Lucidity logo/name.
- Full-width `+ New` button.
- Optional adjacent quick-launch dropdown.
- Search button/field.
- Agent/project filter.

### Active section

Each row includes:

- adapter icon;
- title;
- secondary agent + project metadata;
- status dot/text;
- selected state;
- hover actions:
  - Settle;
  - Stop/X or Resume;
  - More.

### History section

- compact;
- collapsed count when large;
- virtualized;
- same search and filters;
- shows runtime status if a settled conversation is still running.

### Footer

- Settings;
- usage/account summary;
- tray/host status only when useful.

## 9.3 Session content header

Show:

- agent icon and profile;
- title;
- project;
- runtime status;
- account-usage button;
- context-usage button/bar;
- overflow menu.

Do not crowd the terminal with a large toolbar.

## 9.4 New-session dialog

Required fields:

- agent profile;
- project path;
- optional title;
- optional per-launch arguments.

Behavior:

- recent project list;
- native Windows folder picker;
- clear missing-executable state;
- Configure Agent shortcut;
- create button disabled until requirements are valid.

## 9.5 Row semantics

### Click

- live attachment: focus existing pane;
- no attachment: resume native conversation;
- missing adapter/executable: show repair dialog, not a blank terminal.

### Settle

- update `OrganizationState::Settled`;
- never stop or detach;
- keep visible runtime badge if working.

### Stop/X

- stop the owned runtime/process tree;
- detach pane;
- retain conversation and organization state;
- replace X with Resume/play when stopped.

### More/context menu

- Rename;
- Settle/Unsettle;
- Stop/Resume;
- Open project folder;
- Reveal transcript/session file where available;
- Remove from Lucidity;
- Delete native history only behind a separate explicit destructive confirmation.

## 9.6 Status language

Human-facing values:

```text
Working
Needs input
Approval needed
Ready
Stopped
Failed
Running elsewhere
Unknown
```

Use short tokens only in diagnostics, not as the main design.

## 9.7 Required control states

Every interactive control must implement:

- hover;
- pressed;
- keyboard focus;
- disabled;
- selected;
- loading where applicable;
- error where applicable;
- tooltip for icon-only actions.

---

# 10. Product UI bridge and state flow

## 10.1 Do not query SQLite during every frame

Add a cached UI model:

```rust
pub struct UiSnapshot {
    pub revision: u64,
    pub host: HostUiState,
    pub adapters: Arc<[AdapterView]>,
    pub profiles: Arc<[ProfileView]>,
    pub active: Arc<[ConversationView]>,
    pub history_page: Arc<[ConversationView]>,
    pub selected: Option<ConversationId>,
    pub account_usage: Arc<HashMap<ProfileId, AccountUsageView>>,
    pub context_usage: Arc<HashMap<ConversationId, ContextUsageView>>,
}
```

Store it as:

```rust
RwLock<Arc<UiSnapshot>>
```

Rendering clones the `Arc`, not the database contents.

Rebuild only when:

- catalog revision changes;
- runtime revision changes;
- usage/context data changes;
- adapter/profile settings change;
- history pagination request completes.

## 10.2 Command channel

```rust
pub enum UiCommand {
    NewConversation(NewConversationRequest),
    OpenConversation(ConversationId),
    SetOrganizationState {
        conversation_id: ConversationId,
        state: OrganizationState,
    },
    StopConversation(ConversationId),
    RenameConversation {
        conversation_id: ConversationId,
        title: String,
    },
    RefreshUsage(ProfileId),
    RefreshContext(ConversationId),
    SaveAppSettings(AppSettingsPatch),
    SaveProfile(ProfilePatch),
    RequestHistoryPage(ConversationQuery),
    OpenProject(PathBuf),
    OpenNativeSessionLocation(ConversationId),
}
```

The GUI sends commands to a channel and returns immediately.

## 10.3 Replace the fixed 25 ms pump

Use an event-driven host loop that blocks on:

- UI commands;
- mux/agent events;
- process exits;
- tray actions;
- filesystem importer events;
- usage provider completion.

When state changes:

1. apply change;
2. rebuild relevant cached view;
3. publish new `Arc<UiSnapshot>`;
4. call `request_product_redraw()`.

No activity means no 40 Hz background wake-up.

---

# 11. Settings architecture

## 11.1 App-owned settings

Lucidity owns its own structured settings file, for example:

```text
%APPDATA%\Lucidity\settings.json
```

Use atomic write:

1. serialize to temporary file;
2. flush;
3. replace;
4. retain last-known-good backup.

Do not rewrite the user's separate `.wezterm.lua`.

## 11.2 Terminal settings

Use a curated typed model first:

```text
Font family
Font size
Line height
Color scheme
Cursor style
Cursor blink
Padding
Scrollback lines
Ligatures
Bell
Hyperlink behavior
Copy/paste behavior
Terminal search shortcut
WebGPU power preference
Preferred GPU adapter
```

Map the model into WezTerm config overrides and use existing config-reload machinery.

Advanced later:

- searchable browser generated from WezTerm `ConfigMeta`;
- import selected values from an existing WezTerm config;
- export generated config.

## 11.3 Keybindings

Maintain two scopes:

1. Lucidity application commands.
2. Terminal/WezTerm actions.

The GUI must show conflicts before saving.

Required keybinding recorder behavior:

- click Record;
- press combination;
- display normalized combination;
- show conflict and destination;
- Save/Cancel;
- Reset to default.

## 11.4 Agent profiles

A user may have multiple profiles for one adapter.

Example:

```text
Claude — Personal
Claude — Work
OpenCode — Kimi provider
OpenCode — GLM provider
```

Add persistent profile configuration:

```rust
struct AdapterProfileConfig {
    id: ProfileId,
    adapter_id: AdapterId,
    display_name: String,
    executable_path: Option<PathBuf>,
    launch_args: Vec<String>,
    resume_args: Vec<String>,
    environment: BTreeMap<String, SecretOrPlainValue>,
    config_root: Option<PathBuf>,
    enabled: bool,
}
```

The GUI always exposes:

- executable;
- launch args;
- resume args;
- environment;
- account/profile label;
- test installation;
- version output.

## 11.5 Harness-native settings

Use the existing adapter-manifest concepts:

- `settings_schema`;
- `settings_bindings`.

Binding engine priorities:

1. harness-supported CLI/config command;
2. format-preserving structured editor;
3. atomic structured rewrite with backup and visible warning.

Support order:

- JSON/JSONC;
- TOML using a format-preserving editor;
- YAML later;
- environment/CLI bindings.

For every change:

- show file and field;
- preserve unrelated data;
- back up;
- validate;
- write atomically;
- roll back on failure;
- identify whether the current session must restart.

## 11.6 Settings page layout

Left navigation:

```text
General
Agents
Terminal
Appearance
Keybindings
Sessions
Usage
Integrations
Advanced
```

Right content uses:

- section heading;
- explanatory text;
- normal setting rows;
- controls aligned consistently;
- reset button;
- restart-required indicator;
- inline errors.

---

# 12. Adapter manifest V2

The current manifests are launch/resume stubs. Extend them without discarding the parser.

## 12.1 New fields

```toml
schemaVersion = 2
id = "claude"
displayName = "Claude Code"
icon = "icon.svg"
brandColor = "#D97757"

capabilities = [
  "history",
  "structured_events",
  "account_usage",
  "context_usage",
  "settings"
]
```

Add structured sections for:

- profile defaults;
- history importer;
- event bridge;
- settings schema/bindings;
- account-usage providers;
- context providers;
- command hints such as `/new`;
- version compatibility;
- assets.

## 12.2 Adapter assets

Each adapter package may include:

```text
adapter.toml
icon.svg
settings.schema.json
fixtures/
hooks/
```

Render SVG icons through `egui_extras`' SVG loader.

## 12.3 Community safety

Declarative adapters may:

- discover executables;
- define command templates;
- define known paths;
- parse structured metadata;
- bind settings;
- provide regex extractors;
- provide assets.

Executable hook installation requires:

- explicit approval;
- visible commands/diff;
- backup and rollback;
- known destinations;
- source attribution.

The later AI adapter-builder produces a draft package and fixtures; it does not silently install arbitrary generated scripts.

---

# 13. Conversation history and identity

## 13.1 Unified catalog

When an adapter profile is configured, Lucidity imports that harness's existing native conversations into the catalog.

Imported rows:

- default to Settled;
- do not allocate a PTY;
- preserve native session ID and path;
- load only metadata needed by the visible list.

## 13.2 Import performance

Startup:

- import the newest bounded page synchronously or immediately after first frame;
- continue older history in the background;
- use filesystem watches or provider events for incrementality;
- do not rescan every transcript on every launch.

Sidebar:

- use `egui::ScrollArea::show_rows` or equivalent virtualization;
- keep pagination;
- load preview text only for visible rows;
- index title, adapter, project, and timestamp.

Acceptance dataset:

- at least 10,000 synthetic conversation records;
- no PTYs for imported history;
- responsive scrolling/filtering;
- no full transcript deserialization during ordinary sidebar rendering.

## 13.3 Import order

Implement and verify independently:

1. Codex
2. Claude Code
3. OpenCode
4. Kimi
5. Grok
6. Cline

The exact native paths and formats must be researched from the installed version or primary source. Do not guess them in a generic plan.

## 13.4 `/new`, `/resume`, `/clear`, and fork behavior

Keep the existing pending-identity and structured-event foundation.

Authority order:

1. structured session event;
2. provider API event;
3. session-store watcher correlated to owned runtime;
4. submitted command hint plus confirmation;
5. screen heuristic;
6. user binding prompt.

When pane P changes from conversation A to new native conversation B:

1. upsert B;
2. detach P from A;
3. attach P to B;
4. keep A in catalog;
5. mark B Active and selected;
6. redraw sidebar;
7. do not restart the harness.

Never create a durable row from raw `/new` keystrokes alone.

---

# 14. Usage and context

## 14.1 Split the data model correctly

Account limits belong to a profile/account.

Conversation context belongs to a conversation.

```rust
struct AccountUsageSnapshot {
    profile_id: ProfileId,
    buckets: Vec<UsageBucket>,
    source: UsageSource,
    confidence: Confidence,
    observed_at: DateTime<Utc>,
    last_error: Option<String>,
}

struct ConversationContextSnapshot {
    conversation_id: ConversationId,
    used_tokens: Option<u64>,
    window_tokens: Option<u64>,
    percent_used: Option<f64>,
    compaction_state: Option<String>,
    source: ContextSource,
    confidence: Confidence,
    observed_at: DateTime<Utc>,
    last_error: Option<String>,
}
```

Do not store weekly account quota only as a conversation row.

## 14.2 Provider order

For each adapter/profile:

1. structured provider API or local app-server;
2. structured harness event;
3. native session/transcript metadata;
4. noninteractive CLI command;
5. installed hook;
6. controlled TUI transaction;
7. screen parser fallback.

## 14.3 Refresh behavior

- Opening the usage popover refreshes if stale for more than five minutes.
- Manual refresh always works.
- Structured providers may refresh on turn completion.
- No account API polling while the usage surface is closed.
- No hidden `/usage` command while the harness is busy or awaiting input.
- Keep and label the last good snapshot on failure.

## 14.4 Controlled TUI transaction

For adapters that require a slash-command screen:

1. require authoritative idle state;
2. capture current terminal state;
3. send configured usage command;
4. wait for ready signature;
5. wait for stable output;
6. capture bounded cells;
7. parse declarative adapter schema;
8. send configured close key;
9. restore focus;
10. cache result.

## 14.5 UI

Account popover:

```text
Claude — Personal
Session limit   62%   resets 3:00 PM
Weekly limit    34%   resets Monday
Updated now · /usage
```

Context surface:

```text
Conversation context
122k / 200k
61%
```

Never merge those into one percentage.

---

# 15. Database and protocol migrations

## 15.1 Database migrations

Add migrations rather than modifying existing SQL.

Proposed additions:

### Migration 004 — profile configuration

```text
profile_configuration
adapter_installations
```

### Migration 005 — conversation previews and search

```text
latest_prompt_preview
summary_preview
search index / supporting indexes
```

### Migration 006 — split usage/context

```text
account_usage_snapshots keyed by profile_id
conversation_context_snapshots keyed by conversation_id
```

### Migration 007 — app/session UI metadata

```text
last_selected_conversation
optional per-conversation UI metadata
```

App-global appearance and keybindings may remain in the app settings file rather than SQLite.

## 15.2 IPC protocol V2

Retain V1 negotiation for the existing extension while adding V2.

Add or extend:

```text
conversation.list with filters/search
conversation.rename
conversation.remove
conversation.openNativeLocation

profile.list
profile.create
profile.update
profile.delete
profile.test

settings.get
settings.patch
settings.reset

usage.getAccount
usage.refreshAccount
context.getConversation
context.refreshConversation
```

Push events:

```text
catalog.changed
conversation.changed
runtime.changed
profile.changed
settings.changed
usage.changed
context.changed
```

---

# 16. Implementation phases and hard gates

The phases are sequential where noted. Parallel work is allowed only across non-overlapping paths.

## Phase 0 — Reset execution authority

### Work

- Tag baseline commit.
- Create `lucidity/v2-gui-shell`.
- Add `docs/SUPERSEDED_PLANS.md`.
- Mark prior hackathon plans as historical, not executable.
- Add this plan as `docs/LUCIDITY_V2_PLAN.md`.
- Capture current screenshot and release performance baseline.
- Disable automatic mock runtime in production; retain `--demo` or test mode.

### Gate 0

- Current package still builds and launches.
- Baseline tests pass.
- Old behavior can be reproduced from tag.
- No new backend architecture work begins before Gate 1.

---

## Phase 1 — Prove egui inside the existing WezTerm surface

### Sole writer

One Rust integration agent owns:

```text
Cargo.toml
Cargo.lock
wezterm-gui/Cargo.toml
wezterm-gui/src/product_ui/**
wezterm-gui/src/termwindow/render/draw.rs
wezterm-gui/src/termwindow/webgpu.rs
wezterm-gui/src/termwindow/mod.rs
wezterm-gui/src/termwindow/keyevent.rs
wezterm-gui/src/termwindow/mouseevent.rs
wezterm-gui/src/termwindow/app_layout.rs
```

No other agent edits these paths.

### Work

- Pin `egui`, `egui-wgpu`, and `egui_extras` to `0.32.3`.
- Add custom integration, not `eframe`.
- Force WebGPU only in Lucidity product mode.
- Render:
  - one real button;
  - one text field;
  - one dropdown;
  - one scroll area;
  - one popover over the terminal.
- Route input correctly.
- Apply clipboard/cursor/IME platform output.
- Preserve terminal functionality.

### Gate 1 acceptance

In one packaged test build:

- button clicks;
- text field accepts typing/copy/paste;
- dropdown works;
- scroll works;
- popover overlays terminal;
- terminal still accepts keys;
- terminal mouse selection works;
- terminal Ctrl+C/Ctrl+V behavior is correct;
- resize and 100/125/150/200% DPI work;
- no second top-level window;
- no second present;
- one `wgpu` major version;
- idle CPU is not materially worse than baseline.

If Gate 1 fails, write an ADR and activate the child-HWND fallback. Do not return to the old hand-painted UI.

---

## Phase 2 — Replace the current chrome with the actual shell

### Work

- Add `lucidity-ui` crate.
- Add normal theme and proportional font.
- Remove internal fake title bar.
- Build:
  - sidebar;
  - active/history lists;
  - content header;
  - welcome screen;
  - `+ New` button;
  - agent picker;
  - project picker;
  - row Settle;
  - row Stop/Resume;
  - context menu;
  - settings route shell.
- Replace `ProductSidebarProvider` with full UI bridge.
- Keep old chrome behind a temporary feature until acceptance.

### Gate 2 acceptance

Using the mock harness, entirely by mouse except terminal typing:

```text
Launch
→ + New
→ choose Mock Agent
→ choose project
→ row appears
→ terminal appears
→ create second session
→ switch rows
→ settle one while it is working
→ row moves to History and keeps working
→ stop it
→ row remains
→ resume it
→ open Settings
→ return to same session
→ close to tray
→ reopen same pane
```

A screenshot review is mandatory.

After Gate 2, the old product chrome is no longer the default.

---

## Phase 3 — Real graphical settings

### Work

- App settings persistence.
- Terminal settings overrides.
- Appearance/theme.
- Keybinding recorder.
- Agent profile editor.
- Installation/version test.
- Safe config binding engine for initial formats.
- Settings navigation and rows.

### Gate 3 acceptance

- Change terminal font family and size.
- Change color scheme.
- Record and apply a shortcut.
- Edit an adapter executable and launch args.
- Detect a conflicting shortcut.
- Reset a setting.
- Restart only the affected session when required.
- Corrupt write simulation rolls back safely.

---

## Phase 4 — Unified real history and identity

### Work

- Implement one importer at a time.
- Add search/filter/pagination.
- Fill adapter icons and profile data.
- Install/activate structured event bridge where supported.
- Confirm `/new` pane rebinding.
- Remove mock from normal production startup.

### Gate 4 acceptance

At least Claude and Codex:

- existing native conversations import;
- rows show correct agent/project/title/time;
- clicking history resumes the correct native session;
- `/new` creates/rebinds a new row only after identity confirmation;
- 10,000-row synthetic catalog remains responsive;
- imported history consumes no PTYs.

OpenCode and Kimi follow before beta completion.

---

## Phase 5 — Usage and context

### Work

- split account and context persistence;
- implement provider abstractions;
- implement Codex structured provider;
- implement at least one controlled TUI provider;
- build usage/context UI;
- stale-on-open refresh;
- error/staleness display.

### Gate 5 acceptance

- account and context are visibly separate;
- refresh occurs only on open/manual/meaningful event;
- busy agents are not interrupted for TUI usage;
- last good data survives provider failure;
- reset timestamps and stale age display correctly.

---

## Phase 6 — Adapter completion

### Work

Complete built-in support in this order:

1. Claude Code
2. Codex CLI
3. OpenCode
4. Kimi Code
5. Grok
6. Cline

For each:

- discovery;
- profile;
- icon;
- launch/resume;
- history;
- events/status;
- settings;
- usage/context where available;
- fixtures;
- version compatibility tests.

### Gate 6 acceptance

Each built-in adapter has a visible capability matrix and degrades honestly when a capability is unavailable.

---

## Phase 7 — Performance, accessibility, and lifecycle hardening

### Work

- replace 25 ms action pump;
- eliminate render-time DB reads;
- virtualize long lists;
- accessibility tree through egui/AccessKit integration;
- keyboard-only navigation;
- Windows notifications;
- DPI matrix;
- renderer failure handling;
- optional OpenGL/egui fallback evaluation;
- crash recovery and stale attachment reconciliation;
- window geometry persistence.

### Gate 7 targets

Measure against stock/pinned WezTerm and current Lucidity baseline:

- near-zero idle CPU when no output/animation;
- additional one-session GUI RSS target no more than roughly 30 MiB over equivalent WezTerm, measured rather than asserted;
- live-session switching begins within 50 ms on test machine;
- sidebar render p95 under 3 ms for virtualized 10,000-record fixture;
- no DB query per frame;
- no transcript load for off-screen rows;
- no input regression in terminal conformance matrix.

Targets may be adjusted with measured evidence, not silently ignored.

---

## Phase 8 — Cursor/VS Code client update

Do this after the desktop workflow is stable.

### Work

- update IPC V2 client;
- Tree View filters and statuses;
- New in current workspace;
- Open/focus;
- Settle/Unsettle;
- Stop/Resume;
- usage summary;
- configure agents command.

The extension remains a client. It owns no duplicate session catalog or PTY.

---

## Phase 9 — Phone control

Deferred until after desktop beta.

First mode:

- computer-hosted LAN HTTP/WebSocket server;
- one-time QR pairing;
- no required Tailscale;
- view and control sessions;
- explicit Windows firewall onboarding;
- device revocation.

Remote-across-internet mode requires a relay/tunnel and is a separate product decision.

---

# 17. File-level migration map

## 17.1 Root/workspace

Modify:

```text
Cargo.toml
Cargo.lock
```

Add exact compatible dependencies:

```text
egui = 0.32.3
egui-wgpu = 0.32.3
egui-extras = 0.32.3
egui_kittest = 0.32.3          # dev/test
```

Do not add `eframe` or `egui-winit`.

## 17.2 WezTerm fork

Add:

```text
wezterm-gui/src/product_ui/**
```

Modify:

```text
wezterm-gui/src/lib.rs
wezterm-gui/src/product_runtime.rs
wezterm-gui/src/termwindow/mod.rs
wezterm-gui/src/termwindow/app_layout.rs
wezterm-gui/src/termwindow/keyevent.rs
wezterm-gui/src/termwindow/mouseevent.rs
wezterm-gui/src/termwindow/render/draw.rs
wezterm-gui/src/termwindow/webgpu.rs
```

Retire after Gate 2:

```text
wezterm-gui/src/termwindow/render/product_chrome.rs
wezterm-gui/src/product_sidebar.rs
```

## 17.3 App UI

Add:

```text
lucidity-ui/**
```

Modify:

```text
agent-terminal/Cargo.toml
agent-terminal/src/chrome/mod.rs
agent-terminal/src/chrome/app.rs
agent-terminal/src/lib.rs
```

Add:

```text
agent-terminal/src/ui_bridge.rs
agent-terminal/src/settings/**
agent-terminal/src/usage/**
agent-terminal/src/importers/**
```

## 17.4 Protocol/data

Modify:

```text
agent-protocol/src/lib.rs
agent-backends/**
agent-terminal/src/catalog/**
agent-terminal/src/host/**
agent-terminal/src/ipc/**
```

Add migrations 004+.

## 17.5 Extension

Modify only after desktop Gate 4:

```text
extensions/vscode/**
```

---

# 18. Parallel execution workflow

## 18.1 Roles

### Conductor — Codex / GPT-5.6 Sol Max

Owns:

- task graph;
- contracts;
- worktrees;
- review assignment;
- gate evidence;
- integration sequence.

May not declare completion from backend receipts alone.

### UI integration owner — separate Codex / GPT-5.6 Sol Max

Sole writer to the WezTerm/egui hot zone.

Owns Gates 1 and renderer/input aspects of Gate 2.

### UX specification and screenshot reviewer — Cursor / Grok 4.5 High

Owns:

- visual specification;
- T3/OpenCode/Hermes interaction reference map;
- screenshot comparisons;
- Cursor extension later.

Does not edit renderer hot-zone files.

### Host bridge/performance owner — native Grok 4.5 High

Owns:

- cached UI snapshot;
- event-driven command loop;
- profile/settings host plumbing;
- removal of fixed polling.

### Adapter/history owner — Kimi K3 Max or OpenCode/Kimi

Owns one adapter at a time with fixtures and no GUI hot-zone edits.

### Mechanical tests — OpenCode/GLM-5.2 Max

Owns:

- synthetic catalogs;
- migration tests;
- schema fixtures;
- negative tests;
- CI scripts.

### Adversarial reviewer — Claude/Fable 5 Extra High

Reviews:

- Gate 1 input/render integration;
- Gate 2 visual/interaction acceptance;
- final architecture and performance.

## 18.2 Maximum parallelism

Before Gate 1: two active streams.

1. UI integration.
2. Read-only UX spec / host bridge preparation.

After Gate 1: up to four non-overlapping streams.

No other worker edits the renderer hot zone.

## 18.3 Required completion packet

Every worker returns:

```text
Task ID
Branch
Commit SHA
Files changed
Behavior implemented
Exact commands run
Test output
Screenshots/video paths
Performance measurements where relevant
Known limitations
Contract changes proposed
Reviewer focus
```

## 18.4 Review pairing

Use different model families:

```text
Sol UI integration     → Fable review
Grok host bridge       → Sol review
Kimi adapter/importer  → Grok or Sol review
GLM tests              → Sol review
Cursor extension       → Sol review
```

## 18.5 Integration rule

One accepted branch at a time.

After every merge:

- targeted tests;
- packaged launch;
- current gate interaction script;
- screenshot if UI changed.

No batch merge of several unverified branches.

---

# 19. Verification matrix

## 19.1 GUI component tests

Use `egui_kittest` for:

- button activation;
- keyboard focus;
- text entry;
- menu opening;
- modal dismissal;
- search filter;
- row settle;
- row stop;
- settings navigation;
- keybinding recorder;
- disabled/error states.

## 19.2 Golden screenshots

Render deterministic fixtures at:

```text
1280×720 @ 100%
1920×1080 @ 100%
1920×1080 @ 125%
2560×1440 @ 150%
3840×2160 @ 200%
```

Required scenes:

- welcome;
- active sessions;
- history expanded;
- new-session dialog;
- session with usage popover;
- settings/appearance;
- settings/agents;
- launch error;
- missing executable;
- narrow sidebar collapsed.

## 19.3 Native terminal regression

Test:

- regular shell;
- Claude;
- Codex;
- Kimi;
- OpenCode;
- alternate screen;
- mouse reporting;
- text selection;
- hyperlinks;
- scrollback;
- bracketed paste;
- Ctrl+C;
- Unicode/IME;
- resize;
- fullscreen/maximize;
- tray hide/reopen.

## 19.4 Lifecycle

Test:

- settle while working;
- stop without settle;
- resume same session;
- `/new` rebind;
- `/quit` leaves durable row;
- crash and restart;
- stale attachment reconciliation;
- stop process tree with child;
- tray exit;
- multiple sessions;
- missing adapter after history import.

## 19.5 Performance

Measure release builds only:

- stock pinned WezTerm;
- current `b215` Lucidity;
- new Lucidity.

Record:

- cold startup;
- idle CPU over 60 seconds;
- RSS with zero, one, and five live sessions;
- session switch latency;
- sidebar scroll/render time;
- database queries per frame;
- terminal output throughput.

---

# 20. What is deliberately deferred

Not part of the desktop recovery critical path:

- cross-agent coordination/worktrees;
- automatic conflict avoidance;
- public cloud relay;
- phone client;
- macOS/Linux;
- multi-window layouts;
- terminal attachment inside Cursor;
- every WezTerm option;
- AI-generated adapter installation;
- arbitrary community executable plugins;
- a custom frameless title bar;
- replacing the native harness UI.

These are deferred because the actual requested desktop application must exist first—not because they are prohibited forever.

---

# 21. Source and licensing ledger

Track copied or ported material in `docs/SOURCE_LEDGER.md`.

Permissible sources:

- WezTerm: MIT
- T3 Code: MIT
- egui/egui-wgpu: MIT OR Apache-2.0
- Lucide icons: ISC
- adapter/project icons: verify and record individually
- CCManager detector logic: MIT, if still used

Do not copy Warp application code into the MIT repository; Warp's application code is AGPL. Its architecture may be studied.

---

# 22. First dispatch order

The conductor's first actions are exactly:

1. Commit this plan as the new authority.
2. Mark old implementation plans superseded.
3. Tag `b215b572d`.
4. Create the V2 integration branch.
5. Dispatch Gate 1 to one Sol implementation session.
6. Dispatch the UX specification/screenshot matrix to Cursor/Grok as read-only.
7. Dispatch event-driven UI bridge design to native Grok, without touching hot-zone files.
8. Send the plan and current screenshot to Fable for a pre-code adversarial review focused on whether the plan now describes the user's actual product.
9. Do not dispatch history, usage, or additional adapters until Gate 1 is green.
10. Do not use the word “complete” until the Section 2 workflow passes in the packaged app.

---

# 23. Final product invariant

> **Lucidity owns the desktop application experience. WezTerm owns the terminal surface. Each coding harness owns its native conversation interface.**

Any implementation that collapses those boundaries—especially one that turns the desktop GUI back into terminal-styled painted text—is a regression, even if it is native, GPU-rendered, and well tested.
