# Lucidity scope (do not scan WezTerm)

This repo is a fork of WezTerm; most of the code is unrelated to Lucidity tasks.

Work primarily in:
- `lucidity-host/`: TCP server inside WezTerm; frames + JSON `{ "op": ... }`
- `lucidity-client/`: shared client library (Rust)
- `lucidity-pairing/`: pairing protocol + QR format + crypto handshake
- `lucidity-proto/`: shared framing (`encode_frame`, `FrameDecoder`)

Only touch WezTerm GUI glue when needed:
- `wezterm-gui/src/main.rs` (autostarts host)
- `wezterm-gui/src/termwindow/mod.rs` (pairing UX hooks)
- `wezterm-gui/src/pairing_handler.rs`
- `wezterm-gui/src/overlay/*` (QR splash + approval dialog)

Avoid diving into:
- `wezterm-*` (except `wezterm-gui`)
- `termwiz/`, `term/`, `mux/`, `window/`, `deps/`, `test-data/`

