# Source Context and Corrected Decisions

The planning source is the ChatGPT conversation **Harness Control Plane Options**:

`https://chatgpt.com/c/6a6a3030-e360-83ea-874f-9856a4a92029`

The conductor read all five visible turns directly on 2026-08-06. This file records accepted product decisions without copying the full conversation into the repository.

## Accepted user decisions

- The product is for people who value native terminal-agent efficiency but find thread discovery, resumption, settings, and status opaque.
- Windows native is the first platform. WSL is neither required nor part of the protected MVP.
- Closing the window should leave a lightweight tray host and live work running. Closing from the tray is the true exit.
- Native harness TUIs and slash commands remain the chat surface.
- All configured harness histories should appear in one paginated, filterable sidebar with agent identity and project context.
- `/new` should result in a new sidebar conversation, but only after native identity is confirmed.
- Settling is purely organizational and must never stop a process.
- Account quota and per-conversation context both matter and must remain separate concepts.
- Usage should refresh on demand or stale-on-open, not through indiscriminate background calls or hidden TUI injection.
- Harness support should be modular and shareable, with declarative manifests, settings bindings, fixtures, and reviewed optional hooks.
- A Cursor/VS Code extension should mirror the host rather than become a second session manager.
- Mobile control is later and must not compete with the Windows vertical slice.
- The project should be MIT licensed.
- The implementation should be sleek and performant Rust.

## Planning assumptions explicitly reopened

- Folder trees in the planning documents were conceptual, not instructions to move the existing WezTerm checkout under `upstream/wezterm`.
- Crate count and boundaries must follow the live repository and actual dependency seams, not one crate per noun.
- Cursor indexing is useful for repository scouting but is not authoritative orchestration state.
- Multiple Kimi providers add throughput but not independent model-family review.
- The settings UI may begin on WezTerm's renderer, but accessibility, form controls, and renderer coupling must be attacked before that becomes irreversible.

## Requested model use

- Codex GPT-5.6 Sol Max: authoritative conductor plus a separate critical Rust/integration session.
- Claude Fable 5 Extra High: architecture attack, renderer/contract review, and integrated-system review.
- Kimi K3: use across multiple harnesses/providers for bounded implementation and long-context audits; do not use Kimi to review Kimi.
- Claude Opus 5 Extra High: at least two deliberate native frontend-design iterations, one generative and one adversarial.
- Grok 4.5 High and GLM-5.2 Max were planning-thread candidates, not requirements. Current local preflight did not produce a supported Grok model turn, so no critical path depends on either route; supported Fable, Kimi, Opus, and Codex lanes own the work.
