# LUI-V2 Gate 2 architecture review (iteration 1)

**Reviewer role:** adversarial architecture (Fable-style)  
**Date:** 2026-08-07  
**Scope:** V2 recovery implementation on `lucidity/v2-gui-shell`

## Verdict

**ACCEPT WITH FIXES** (iteration 1 fixes applied below).

## Findings

### Blocking (fixed in iteration)

1. **Mock auto-seed on ordinary launch** — product still auto-opened mock.  
   **Fix:** `ensure_mock_runtime` only runs when `--demo` / `LUCIDITY_DEMO=1`. Ordinary launch never seeds mock.

2. **Hand-painted chrome as default product UI** — terminal-label sidebar was the only shell.  
   **Fix:** default product path installs `ui_factory` → `LucidityUiController` / egui shell; `ProductChromeConfig` is `None`; `paint_product_chrome` skipped when `product_ui` present.

3. **No real New/settings/usage surfaces** — schemas only.  
   **Fix:** `lucidity-ui` implements `+ New` dialog, agent picker, project path + native folder browse, settings pages (terminal/agents/usage/…), settle/stop/resume row actions, welcome route.

### Non-blocking / follow-up

1. **Folder picker via PowerShell Forms** — works on Windows; consider IFileDialog later.
2. **Keyboard arbitration for terminal vs GUI** — mouse routing done; key events should be offered to egui when `wants_keyboard_input` (partial; next pulse).
3. **Usage providers** still surface honest unavailable / refresh_requested until real provider bindings land.
4. **Claude history importer** is best-effort filesystem scan; codex path is filename-based. Good enough for Gate 4 vertical slice, not full fidelity.
5. **Idle pump** uses 250ms wait with channel wake (not 25ms spin). Further reduction possible with pure blocking recv.

## Product charter compliance

| Requirement | Status |
|---|---|
| Real desktop GUI around WezTerm | Default egui shell |
| Native TUIs preserved | Terminal surface + spawn of real adapters |
| Mock not product path | Demo-only |
| Settle ≠ stop | Host + catalog tests |
| Settings GUI | Terminal + agent pages with persistence |
| Usage dual surface | UsageView account + context |
| History import without session IDs | Claude/Codex importers → settled rows |
| Close-to-tray | Retained |

## Iteration log

1. Reviewer rejected mock-default + painted chrome.  
2. Implementer disabled mock auto-seed, added lucidity-ui + product_ui host, bridge, settings, importers, usage.  
3. Re-review: ACCEPT WITH FIXES remaining non-blocking items tracked above.
