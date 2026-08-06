param(
    [Parameter(Mandatory = $true)]
    [int] $ProcessId,

    [Parameter(Mandatory = $true)]
    [ValidateRange(320, 7680)]
    [int] $Width,

    [Parameter(Mandatory = $true)]
    [ValidateRange(240, 4320)]
    [int] $Height
)

$ErrorActionPreference = 'Stop'

Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class LucidityResizeNative {
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool SetWindowPos(
        IntPtr hwnd,
        IntPtr insertAfter,
        int x,
        int y,
        int width,
        int height,
        uint flags
    );
}
'@

$process = Get-Process -Id $ProcessId -ErrorAction Stop
$handle = $process.MainWindowHandle
if ($handle -eq [IntPtr]::Zero) {
    throw "Process $ProcessId has no main window"
}

$before = [LucidityResizeNative+Rect]::new()
if (-not [LucidityResizeNative]::GetWindowRect($handle, [ref] $before)) {
    throw "GetWindowRect failed before resize for process $ProcessId"
}

$noMove = 0x0002
$noZOrder = 0x0004
$noActivate = 0x0010
if (-not [LucidityResizeNative]::SetWindowPos(
    $handle,
    [IntPtr]::Zero,
    0,
    0,
    $Width,
    $Height,
    $noMove -bor $noZOrder -bor $noActivate
)) {
    throw "SetWindowPos failed for process $ProcessId"
}

Start-Sleep -Milliseconds 750

$after = [LucidityResizeNative+Rect]::new()
if (-not [LucidityResizeNative]::GetWindowRect($handle, [ref] $after)) {
    throw "GetWindowRect failed after resize for process $ProcessId"
}

[pscustomobject]@{
    ProcessId = $ProcessId
    BeforeWidth = $before.Right - $before.Left
    BeforeHeight = $before.Bottom - $before.Top
    RequestedWidth = $Width
    RequestedHeight = $Height
    AfterWidth = $after.Right - $after.Left
    AfterHeight = $after.Bottom - $after.Top
}
