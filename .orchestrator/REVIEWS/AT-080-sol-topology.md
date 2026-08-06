# AT-080 — Separate Sol Live-Topology Audit

Verdict: **ACCEPT AS WAVE 0 EVIDENCE**. Read-only audit; no product or control-plane edits by the worker.

**Supersession note (controller, 2026-08-06).** Finding 6 correctly identifies the overlapping hot zone but its proposed ordering is historical. ADR-004 and the current Build Map serialize AT-105's generic close/hide foundation **before** AT-101's viewport gate; AT-108 alone performs the later tray/product-intent binding.

Base: `4b1c3c151eb530e569f867e1461693c56fe89695`

## Accepted findings

1. The plan's wrapper layout is wrong for the live checkout. Keep WezTerm at repository root.
2. Use additive root-sibling packages. Begin with `agent-protocol`, `agent-backends`, and `agent-terminal`; split only when a dependency or ownership boundary proves necessary.
3. A separate agent binary is feasible by adding a `wezterm-gui` library seam and retaining a thin stock binary.
4. Workspace/lock/package manifests are integration-only mutex files.
5. The GUI hot zone is one-writer and includes `wezterm-gui/src/{main.rs,lib.rs,frontend.rs,tabbar.rs,resize_increment_calculator.rs,termwindow/**}`.
6. Native close/tray work overlaps `window/src/os/windows/**`, `frontend.rs`, and `termwindow/mod.rs` and must be serialized after the viewport gate.
7. Current high-level close kills the mux window; current Windows `hide()` path minimizes rather than implementing tray hiding.
8. Padding inflation is not a safe sidebar. Render backgrounds, scrollbar coordinates, mouse clamping, and the lack of one shared scissor primitive make false clipping/input proof likely.
9. One explicit layout must drive geometry and input. Opaque overdraw is not sufficient clipping evidence.
10. Gate 1 should extract the library seam, prove stock parity, add the agent binary, test pure layout with Agent mode disabled, thread the geometry through all consumers, then render/route chrome and finally suppress the stock tab bar.

## Exact live owners identified

- OS window: `wezterm-gui/src/frontend.rs` and `window/src/os/windows/window.rs`.
- `TermWindow`/initial dimensions: `wezterm-gui/src/termwindow/mod.rs`.
- Resize/DPI: `termwindow/resize.rs` and `resize_increment_calculator.rs`.
- Render origin/extent: `termwindow/render/{mod.rs,paint.rs,pane.rs,split.rs,screen_line.rs}`.
- Mouse/cell/selection: `termwindow/mouseevent.rs`, `selection.rs`, and `update_text_cursor` in `termwindow/mod.rs`.
- Tab bar/hits: `wezterm-gui/src/tabbar.rs` and `termwindow/render/{tab_bar.rs,fancy_tab_bar.rs}`.
- Mux activation: `TermWindow::activate_tab` into `mux/src/window.rs`.
- Close: Win32 `WM_CLOSE` to `WindowEvent::CloseRequested` to `TermWindow::close_requested` and `Mux::kill_window`.

## Unproven claims retained

- No stock build or native GUI behavior was accepted from this worker.
- No claim of real GPU clipping is made.
- Tray implementation details remain open.
- The three-package boundary still requires Fable/Kimi attack before freeze.
