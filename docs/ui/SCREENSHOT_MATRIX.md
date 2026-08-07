# Screenshot acceptance matrix

Capture at 1280×800 and 1920×1080, 100% and 150% DPI when possible.

| ID | Scene | Pass criteria |
|---|---|---|
| S01 | Welcome (no sessions) | Native title bar, sidebar, `+ New`, empty welcome copy, no mock auto-row |
| S02 | New session dialog | Agent cards, folder field, Create enabled when valid |
| S03 | One active Claude/Codex/session | Real TUI or honest missing-exe state; row metadata correct |
| S04 | Two active sessions | Switch by click; no duplicate pane for attached |
| S05 | Settled while Working | Row in History; status still Working |
| S06 | Stopped row | Stop left row; Resume available |
| S07 | Settings shell | Navigation + terminal/agent pages with real controls |
| S08 | Usage | Account limit + context panels or provider-unavailable |
| S09 | Narrow window | Sidebar collapses; restore control visible |
| S10 | After tray reopen | Same session surface restored |

Store captures under scratch `ui-review/` or `docs/baselines/screenshots/`.
