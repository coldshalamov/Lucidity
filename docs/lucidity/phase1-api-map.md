# Phase 1 API Map (WezTerm internals we’re reusing)

This note captures the minimal internal hooks needed for the Phase 1 “local mirroring” proof.

## Pane enumeration

- `mux::Mux::get().iter_panes()` returns `Vec<Arc<dyn Pane>>`.
- `mux::Mux::get().get_pane(pane_id)` returns `Option<Arc<dyn Pane>>`.

## Sending input to a live pane (shared PTY)

- `mux::pane::Pane::writer()` returns a `Write` handle to the pane’s PTY master.
  - Writing bytes here injects input into the same PTY used by the desktop pane.

## Where PTY output bytes are read

- `mux/src/lib.rs` spawns a per-pane thread `read_from_pane_pty(...)` when a pane is added.
- That thread does blocking `reader.read(&mut buf)` and feeds the parser via a socketpair.

Phase 1 needs a “tap” at this point to broadcast raw PTY bytes to remote subscribers without
interfering with the existing parser/UI path.

## Existing mux notifications (render path)

- `mux::MuxNotification::PaneOutput(pane_id)` exists and is used to schedule screen updates for mux clients.
- This is *not* the raw PTY byte stream; it’s the post-parse “pane changed” signal.

