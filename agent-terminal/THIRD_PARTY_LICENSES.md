# Direct dependency license record

This package is MIT-licensed. Its new direct registry dependency edges use crates already pinned by the root workspace:

| Crate | SPDX license |
|---|---|
| `anyhow` | MIT OR Apache-2.0 |
| `async-channel` | MIT OR Apache-2.0 |
| `chrono` | MIT OR Apache-2.0 |
| `log` | MIT OR Apache-2.0 |
| `parking_lot` | MIT OR Apache-2.0 |
| `rusqlite` | MIT |
| `serde` | MIT OR Apache-2.0 |
| `serde_json` | MIT OR Apache-2.0 |
| `sha2` | MIT OR Apache-2.0 |
| `smol` | MIT OR Apache-2.0 |
| `tempfile` | MIT OR Apache-2.0 |
| `uuid` | Apache-2.0 OR MIT |
| `winapi` | MIT OR Apache-2.0 |
| `windows` | MIT OR Apache-2.0 |
| `cc` | MIT OR Apache-2.0 |
| `embed-resource` | MIT |

The direct path dependencies (`agent-backends`, `agent-protocol`, `config`, `mux`, `portable-pty`, and `window`) are MIT in-workspace packages. No new registry package or version is introduced by this package.
