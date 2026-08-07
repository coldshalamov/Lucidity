# Lucidity Windows package

This directory documents the reproducible Windows distribution assembled at
`dist\Lucidity`. The package keeps the native GUI, deterministic mock harness,
adapter contracts, editor extension, and required notices together.

## Build and assemble

From a Visual Studio developer PowerShell at the repository root:

```powershell
.\scripts\package-windows.ps1
```

The default path uses `cargo build --locked --release`, `npm ci`, and the
extension's checked-in package script. It requires the Rust MSVC toolchain,
Visual Studio C++ build tools, Node.js, and npm.

For a rapid package from existing debug binaries and an existing VSIX:

```powershell
.\scripts\package-windows.ps1 -SkipBuild
```

`-SkipBuild` runs no Cargo or npm commands and defaults to `target\debug`. If
the artifacts live elsewhere, use `-TargetDirectory`; if the VSIX is elsewhere,
use `-VsixPath`:

```powershell
.\scripts\package-windows.ps1 -SkipBuild `
  -TargetDirectory C:\build\lucidity-target `
  -VsixPath C:\build\lucidity-vscode-0.1.0.vsix
```

The assembler only replaces the exact `dist\Lucidity` directory. It writes a
sorted `SHA256SUMS.txt` covering the resulting package.

## Run Lucidity

From the repository root after assembly:

```powershell
powershell -ExecutionPolicy Bypass -File .\dist\Lucidity\Start-Lucidity.ps1
```

Or, from inside `dist\Lucidity`:

```powershell
powershell -ExecutionPolicy Bypass -File .\Start-Lucidity.ps1
```

The launcher prepends the packaged `bin` directory to `PATH`, publishes the
packaged adapter directory through `LUCIDITY_ADAPTER_PACKAGES`, and starts the
native `agent.exe`. Pass `-Wait` to keep the launcher attached until Lucidity
exits.

## Install the editor extension

VS Code:

```powershell
code --install-extension .\dist\Lucidity\extensions\lucidity-vscode-0.1.0.vsix
```

Cursor:

```powershell
cursor --install-extension .\dist\Lucidity\extensions\lucidity-vscode-0.1.0.vsix
```

Start Lucidity, open the Lucidity Activity Bar view, and run **Lucidity: New in
Current Workspace**. The packaged `mock-agent` adapter is the deterministic
first-run harness; Claude, Codex, and Kimi adapter packages are also included.

## Package layout

```text
dist/Lucidity/
  Start-Lucidity.ps1
  README.md
  BUILD-INFO.txt
  SHA256SUMS.txt
  bin/
    agent.exe
    lucidity-mock-agent.exe
    *.dll
  adapters/
    *.agent-adapter/
  extensions/
    lucidity-vscode-0.1.0.vsix
  licenses/
```
