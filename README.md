# Lucidity

*Lucidity is a fork of [WezTerm](https://github.com/wez/wezterm): a GPU-accelerated terminal emulator + multiplexer written in Rust.*

**Product goal:** a desktop terminal that runs a real PTY/ConPTY session, plus a mobile app that renders the terminal output locally (terminal emulator) and sends keystrokes so your phone behaves like you’re typing into the desktop terminal in real time.

## Status (as of 2026-01-17)

- Phase 1 (local mirroring proof): **implemented**.
  - Desktop host bridge (`lucidity-host`) can list panes, attach to one pane, stream raw PTY output bytes, and inject input bytes.
  - A minimal CLI client (`lucidity-client`) can connect over TCP and mirror a pane.
- Phase 3 (pairing splash UX): **implemented (local-only)**.
  - Desktop shows a QR/code overlay on first window open (close with Enter).
  - Desktop host exposes a local pairing API (`pairing_payload` / `pairing_submit`).
  - When the GUI is running, pairing requests require an approve/reject prompt.
  - Approved devices are stored in a local SQLite trust store.
- Mobile apps, cloud relay, Google OAuth, subscriptions/quotas: **not implemented yet** (tracked in `docs/lucidity/`).


## Phase 1 quick start (local proof)

1) Run the GUI (`wezterm-gui`). It auto-starts the host bridge on localhost.
2) In another terminal, connect the test client:

Windows build note:
- Building `wezterm-gui`/`wezterm` on Windows requires a full Perl toolchain for vendored OpenSSL.
- If you hit OpenSSL/perl errors, install Strawberry Perl (recommended) and ensure it is earlier on PATH than Git/MSYS perl.


```sh
cargo run -p lucidity-client -- --addr 127.0.0.1:9797
```

To allow LAN connections (will likely trigger firewall prompts):

```sh
set LUCIDITY_LISTEN=0.0.0.0:9797
```

To disable the embedded host server:

```sh
set LUCIDITY_DISABLE_HOST=1
```

To disable the pairing splash overlay:

```sh
set LUCIDITY_DISABLE_SPLASH=1
```

## Docs

- Lucidity overview + roadmap: `docs/lucidity/index.md`
- Phase 1 protocol + usage: `docs/lucidity/phase1.md`

## Upstream

Lucidity is based on WezTerm. See WezTerm’s original project and docs for the baseline terminal behavior and features.

## Supporting the Project

If you use and like WezTerm, please consider sponsoring it: your support helps
to cover the fees required to maintain the project and to validate the time
spent working on it!

[Read more about sponsoring](https://wezterm.org/sponsor.html).

* [![Sponsor WezTerm](https://img.shields.io/github/sponsors/wez?label=Sponsor%20WezTerm&logo=github&style=for-the-badge)](https://github.com/sponsors/wez)
* [Patreon](https://patreon.com/WezFurlong)
* [Ko-Fi](https://ko-fi.com/wezfurlong)
* [Liberapay](https://liberapay.com/wez)
