# Lucidity UI specification (V2)

**Status:** implementation authority for shell chrome  
**Source:** V2 recovery plan §8–9 + user product charter

## Product reading

Lucidity is a normal Windows desktop app with:

- native Windows title bar (no internal fake title bar);
- polished resizable sidebar;
- large embedded WezTerm terminal workspace;
- real GUI controls (buttons, text fields, menus, dialogs).

It is **not** a terminal theme and not a custom chat renderer.

## Main window regions

```text
┌──────────────────────────────────────────────────────────┐
│ Windows title bar (native)                               │
├──────────────┬───────────────────────────────────────────┤
│ Sidebar      │ Content header                            │
│  288px def   ├───────────────────────────────────────────┤
│              │                                           │
│  [+ New]     │  Welcome  |  Session terminal  | Settings │
│  Search      │                                           │
│  ACTIVE      │                                           │
│   rows…      │                                           │
│  HISTORY     │                                           │
│   rows…      │                                           │
│  Settings    │                                           │
└──────────────┴───────────────────────────────────────────┘
```

### Dimensions (logical px @ 96 DPI)

| Token | Value |
|---|---:|
| Sidebar default | 288 |
| Sidebar min / max | 232 / 420 |
| Content header | 52 |
| Active session row | 52 |
| History row | 38 |
| Gap small / normal / section | 6 / 10 / 16 |
| Corner radius | 8 |

## Sidebar

- Header with product name and `+ New` primary button
- Search/filter field
- **Active** section (current work)
- **History / Settled** section (compact)
- Footer: Settings entry, optional usage badge
- Row shows: agent icon, title, project/agent metadata, runtime status badge
- Hover reveals Settle / Stop (X) / overflow
- Click open/focus/resume; never duplicates an attached runtime
- Settle never stops; Stop never settles/deletes

## New session dialog

1. Choose agent/profile (icon cards: Claude, Codex, Kimi, custom, mock only if demo)
2. Choose project folder (native Windows folder picker)
3. Optional title / extra launch args
4. Create → durable Active row + launch native TUI when executable present
5. Missing executable → honest configure / error state (no silent empty shell as success)

## Routes

- `Welcome` — empty state when no selection
- `Session(id)` — header + terminal rect
- `Settings(page)` — opaque GUI over terminal; terminal input blocked

## Typography

- Chrome: proportional (Segoe UI / Inter / DM Sans)
- Terminal: independent monospace
- No all-caps monospaced product chrome
- No status tokens like `WORK` / `EXT?` as primary labels; use plain language (“Working”, “Needs input”)

## Accessibility

- Keyboard focus order, tooltips, minimum 28×28 hit targets
- High-contrast-friendly surfaces; saturated color for runtime state only
