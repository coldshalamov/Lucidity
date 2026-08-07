[CmdletBinding()]
param(
    [switch] $Wait
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$distributionRoot = Split-Path -Parent $PSCommandPath
$binDirectory = Join-Path $distributionRoot "bin"
$adapterDirectory = Join-Path $distributionRoot "adapters"
$agentExecutable = Join-Path $binDirectory "agent.exe"

if (-not (Test-Path -LiteralPath $agentExecutable -PathType Leaf)) {
    throw "Lucidity executable is missing: $agentExecutable"
}

$env:LUCIDITY_DISTRIBUTION_ROOT = $distributionRoot
$env:LUCIDITY_ADAPTER_PACKAGES = $adapterDirectory
$env:PATH = "$binDirectory;$env:PATH"

$process = Start-Process -FilePath $agentExecutable -WorkingDirectory $distributionRoot -PassThru
Write-Host "Lucidity started (PID $($process.Id))."

if ($Wait) {
    $process.WaitForExit()
    exit $process.ExitCode
}
