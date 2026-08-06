# Verification Contract

Status: commands are exact where the live repository already supports them; new-package commands become executable after AT-099.

## Evidence grades

- **Direct:** controller ran the command or observed the native behavior at the exact commit.
- **Candidate:** worker supplied a receipt; controller has not reproduced it.
- **Unproven:** code or narration suggests the claim, but no acceptance evidence exists.

Only direct evidence closes a gate.

## Gate 0 — stock Windows baseline

Environment:

```powershell
& .orchestrator\scripts\run-gate0-stock-build.ps1
```

The script prepends the verified portable Strawberry paths, initializes Visual Studio through `vcvars64.bat`, and executes:

```powershell
cargo build --release -p wezterm-gui
```

After build:

```powershell
git status --short
Get-FileHash target\release\wezterm-gui.exe -Algorithm SHA256
```

Native stock smoke, with no existing mux process reused:

```powershell
target\release\wezterm-gui.exe start --always-new-process
```

Record startup, text input, selection/copy, paste, resize, and clean close. Capture process working set/private bytes and idle CPU after a documented quiet interval. Do not claim deterministic runtime acceptance from build success.

Current direct receipts at `4b1c3c151eb530e569f867e1461693c56fe89695`:

- release `wezterm-gui` and user-facing `wezterm` builds succeeded;
- packaged native window and one 80x25 pane started at 120 DPI;
- native mux `list` and `get-text` confirmed the pane and `LUCIDITY_GATE0_RENDER_OK`;
- native mux `send-text --no-paste` plus `get-text` confirmed `LUCIDITY_GATE0_INPUT_OK`;
- `PrintWindow` image inspection confirmed both markers and a visible cursor;
- the stock GUI window directly reported `PerMonitorV2` through the Win32 DPI-awareness-context API;
- direct Win32 resize changed the mux pane from 80x25 / 880x550 physical pixels to 132x42 / 1452x924 at 120 DPI and the resized capture retained the marker/cursor;
- five owned launch trees closed cleanly through the main window with zero remaining descendants;
- a 15.017s quiet sample recorded 0.03125 owned CPU seconds, 303,394,816 bytes aggregate working set, and 236,437,504 bytes aggregate private memory.

These receipts do not prove mouse selection or system-clipboard copy/paste. Keep those checks open as deferred manual acceptance, but do not delay implementation or substitute automation for the eventual witness.

## Focused deterministic matrix after AT-099

```powershell
cargo fmt --all -- --check
cargo check -p agent-protocol -p agent-backends -p agent-terminal -p wezterm-gui
cargo test -p agent-protocol
cargo test -p agent-backends
cargo test -p agent-terminal
cargo test -p mux
cargo test -p wezterm-gui
cargo check -p portable-pty
cargo build --release -p wezterm-gui -p agent-terminal
```

AT-099 must not emit Cargo's `missing a lib target`/ignored-dependency warning: ADR-004 defers the `agent-terminal -> wezterm-gui` path dependency until AT-101 has created the real library target. AT-101's manifest commit then proves the warning is absent and records the exact mechanical lockfile delta.

`cargo nextest` is optional until it is verified installed; it is not a hidden prerequisite.

## Contract tests

Required negative and invariant cases:

- settle/unsettle leaves `RuntimeState`, attachments, pane, PID, and process tree unchanged;
- stop leaves `OrganizationState` unchanged;
- imported history allocates no runtime resource;
- opening a live conversation focuses rather than duplicates it;
- duplicate, stale-generation, malformed, oversized, invalid-UTF-8, and future-version events fail safely;
- ADR-002's 32,768-byte decoded boundary and reserved user-var name are tested at max/max+1 through the real parser→alert→mux→host path;
- saturating the 256-event host queue never blocks terminal output or GUI dispatch and deterministically produces `event_resync_required` plus bounded reconciliation;
- event-before-file, file-before-event, and duplicate-delivery identity transitions converge to one result;
- hint→crash, hint B→hint C→confirm C, confirm-without-hint, expiry, and cross-source stable-evidence-ID delivery converge deterministically;
- after host crash/restart, a stale `(old_host_instance_id, generation)` detach/rebind cannot affect the new instance;
- every prior-host attachment is non-live until current evidence explicitly reattaches it;
- after `/new` rebinds pane P from native ID A to B, a late `session_end(A)` cannot detach B;
- A becomes `NotRunning` with `NativeIdentitySuperseded` while B inherits the exact pane/process owner at generation N+1;
- pagination order and cursor stability remain deterministic under insert/update; a changed snapshot version forces resync;
- catalog corruption/migration failure creates a unique preserved quarantine backup before deterministic rebuild;
- typed process exit is observed exactly once before pane removal for clean, non-zero, explicit-kill, and waiter-error paths;
- corrupt/missing histories quarantine individually;
- direct command templates do not invoke a shell or interpolate unsafe strings;
- last-good usage data survives refresh failure with age/source/error labels.

## Gate 1 geometry and renderer proof

Pure tests:

- stock layout reproduces pinned-base golden integer dimensions captured before the refactor, not values generated by the refactored formula;
- header/sidebar reduce terminal pixels, rows, and columns correctly;
- terminal origin maps to cell `(0,0)`;
- every chrome-side boundary point is rejected by terminal mapping;
- tiny windows saturate without negative/wrapped dimensions;
- DPI transitions recompute from logical inputs without stale values;
- selection, hyperlink, split, and scrollbar coordinates share the layout result.

Native matrix at fixed receipts:

- inspect the running process DPI awareness and require PerMonitorV2 before accepting any DPI screenshot;
- 100%, 125%, 150%, and 200% DPI where the environment supports them;
- narrow/default/wide sidebar;
- resize, maximize, restore, and minimum-size window;
- mouse selection, wheel, links, clipboard paste, cursor, and IME position;
- real OS-input sidebar activation and chrome click swallowing; mux/CLI activation is not acceptance evidence for hit routing;
- alternate screen, splits, wide glyphs, underlines, Kitty/sixel images where available;
- two sidebar rows repeatedly activate two mux tabs;
- Win11 maximize-button hover exposes the expected snap-layout behavior after agent chrome relocates/suppresses stock tab chrome;
- cross-monitor DPI drag at the available scale pairs shows no oscillation/double resize; cursor and both IME preedit modes share the mouse/layout border origin;
- stock chrome-disabled mode remains visually and behaviorally stock.

If clipping is implemented by bounded geometry plus paint ordering, explicitly test escape content on both renderers. Use transparent-chrome test mode or direct quad-extent assertions so an opaque sidebar cannot hide bleed and create a false pass. Do not label opaque overdraw as renderer clipping.

## Gate 2 Windows lifecycle smoke

The deterministic mock harness must expose its native ID, pane ID, PID, child PID, and monotonic output sequence to the test controller.

```text
start host and UI
launch mock with optional child
assert catalog + one attachment + live parent/child/grandchild Job tree
assert structured state traversed the SetUserVar alert/mux/host path
close UI
assert host, pane, parent, child, and output sequence continue
observe zero Lucidity GUI windows through a documented debounce interval; fail on reconciliation respawn
reopen
assert same pane/PIDs and terminal sequence
settle
assert exact runtime identity is unchanged
stop
assert selected owned process tree exits and organization remains
tray Exit
assert no owned process remains
```

Also test window/render-context loss with host-process survival, hard host-process crash, agent crash, host restart, stale attachment reconciliation, missing session file, exact Quit behavior, nested-job/assignment failure, attempted breakaway, and stock direct-child behavior. Do not call a whole-process crash a surviving-host UI crash.

## Local IPC security and flow control

- reject remote named-pipe clients;
- reject a local client whose SID is not the explicit owner;
- obtain the connected client PID/token and verify its user SID rather than trusting the pipe name or inherited token;
- reject zero/oversized/truncated/invalid-UTF-8 frames without allocation spikes or parser panic;
- hold a subscriber slow until its bounded queue overflows, then require `resync_required` or disconnect and a complete snapshot refetch;
- restart the host, observe a changed `host_instance_id`, and prove the client discards incremental assumptions;
- verify the Node extension-host named-pipe client on Windows before removing the loopback fallback from consideration.

## Performance verification

Measure stock and candidate at matched commit/toolchain/release configuration:

- startup to first usable window;
- idle CPU over the same quiet interval;
- working set and private bytes for host/UI separately where possible;
- sidebar query and view-model latency at 10,000 records;
- live session switch latency;
- redraw/invalidation count while idle;
- redraw/invalidation count with Lucidity chrome visible versus stock, including frames per idle minute and proof that badges do not schedule animation frames;
- provider/network call count while usage UI is closed;
- all network attempts while Agent UI is idle, including proof that the upstream update checker is disabled;
- transcript bytes deserialized for first catalog page and off-screen rows.

Report raw numbers and collection method. Relative claims without a matched baseline remain unproven.

## Extension verification

- activates only when its view or command is used;
- fixture-host mode compiles and tests before Rust host binding;
- version handshake and reconnect are deterministic;
- incremental tree updates preserve selection and ordering;
- current-workspace launch payload is exact;
- missing host provides actionable guidance;
- Windows named-pipe connection, version handshake, queue-overflow resync, and host-instance-change snapshot recovery are direct platform receipts;
- no WebView is required for the MVP.

## Windows product-binary resources

- inspect PE manifest/resource data for PerMonitorV2, UTF-8 active code page, product icon/resource, and version strings;
- inspect the binary subsystem, distinct AUMID/window class/title, and once-only bootstrap ordering;
- verify the live process reports PerMonitorV2 rather than relying on manifest text alone;
- verify required ConPTY/OpenConsole and selected renderer fallback files are provisioned beside the product binary and match the intended source hashes/freshness rather than merely existing;
- verify tray/toast/application identity is distinct from `org.wezfurlong.wezterm` when stock WezTerm is also installed;
- verify `--version` is assigned before argument parsing and never returns the fallback string.

## Acceptance records

Each accepted task gets `.orchestrator/REVIEWS/<task>-<reviewer>.md` containing exact commit, diff scope, commands and raw outcomes, reproduced findings, verdict, controller disposition, and remaining unproven claims.
