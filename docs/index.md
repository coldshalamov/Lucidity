---
hide:
  - toc
---

*WezTerm is a powerful cross-platform terminal emulator and multiplexer written by <a href="https://github.com/wez/">@wez</a> and implemented in <a href="https://www.rust-lang.org/">Rust</a>*

!!! note "Lucidity fork"
    This repository is evolving into **Lucidity**: a WezTerm fork that adds a desktop “host bridge” and a mobile terminal app so your phone can control a live desktop pane (PTY mirroring, not remote desktop).

    - Lucidity overview: `lucidity/index.md`
    - Phase 1 usage: `lucidity/phase1.md`

![Screenshot](screenshots/wezterm-vday-screenshot.png)

[Download :material-tray-arrow-down:](installation.md){ .md-button }

## Features

* Runs on Linux, macOS, Windows 10, FreeBSD and NetBSD
* [Multiplex terminal panes, tabs and windows on local and remote hosts, with native mouse and scrollback](multiplexing.md)
* <a href="https://github.com/tonsky/FiraCode#fira-code-monospaced-font-with-programming-ligatures">Ligatures</a>, Color Emoji and font fallback, with true color and [dynamic color schemes](config/appearance.md).
* [Hyperlinks](hyperlinks.md)
* [A full list of features can be found here](features.md)

Looking for a [configuration reference?](config/files.md)

**These docs are searchable: press `S` or click on the magnifying glass icon
to activate the search function!**

<figure markdown>

![Screenshot](screenshots/two.png)

<figcaption>Screenshot of wezterm on macOS, running vim</figcaption>
</figure>
