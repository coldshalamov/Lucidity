param(
  [Parameter(Mandatory = $true)]
  [ValidateSet('root', 'child', 'grandchild')]
  [string] $Role,

  [Parameter(Mandatory = $true)]
  [string] $ReceiptDirectory
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

trap {
  $errorPath = Join-Path $ReceiptDirectory "$Role.error"
  [System.IO.File]::WriteAllText($errorPath, ($_ | Out-String))
  exit 1
}

[System.IO.Directory]::CreateDirectory($ReceiptDirectory) | Out-Null
$pidPath = Join-Path $ReceiptDirectory "$Role.pid"
[System.IO.File]::WriteAllText($pidPath, [string] $PID)

if ($Role -ne 'grandchild') {
  $nextRole = if ($Role -eq 'root') { 'child' } else { 'grandchild' }
  $pwsh = [Environment]::ProcessPath
  if ([string]::IsNullOrWhiteSpace($pwsh)) {
    $pwsh = (Get-Process -Id $PID).Path
  }

  $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
  $startInfo.FileName = $pwsh
  $startInfo.UseShellExecute = $false
  foreach ($argument in @(
      '-NoLogo',
      '-NoProfile',
      '-NonInteractive',
      '-ExecutionPolicy',
      'Bypass',
      '-File',
      $PSCommandPath,
      '-Role',
      $nextRole,
      '-ReceiptDirectory',
      $ReceiptDirectory
    )) {
    [void] $startInfo.ArgumentList.Add($argument)
  }

  $child = [System.Diagnostics.Process]::Start($startInfo)
  if ($null -eq $child) {
    throw "Failed to start $nextRole helper"
  }
  $child.Dispose()
}

$stopPath = Join-Path $ReceiptDirectory 'stop'
$deadline = [DateTime]::UtcNow.AddMinutes(1)
while (-not [System.IO.File]::Exists($stopPath)) {
  if ([DateTime]::UtcNow -ge $deadline) {
    exit 2
  }
  Start-Sleep -Milliseconds 25
}
