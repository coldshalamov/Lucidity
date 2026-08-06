param(
    [Parameter(Mandatory = $true)]
    [int] $ProcessId
)

$ErrorActionPreference = 'Stop'

Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class LucidityDpiNative {
    [DllImport("user32.dll")]
    public static extern IntPtr GetWindowDpiAwarenessContext(IntPtr hwnd);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool AreDpiAwarenessContextsEqual(IntPtr first, IntPtr second);
}
'@

$process = Get-Process -Id $ProcessId -ErrorAction Stop
$handle = $process.MainWindowHandle
if ($handle -eq [IntPtr]::Zero) {
    throw "Process $ProcessId has no main window"
}

$context = [LucidityDpiNative]::GetWindowDpiAwarenessContext($handle)
$perMonitorV2 = [IntPtr]::new(-4)
$perMonitor = [IntPtr]::new(-3)
$systemAware = [IntPtr]::new(-2)
$unaware = [IntPtr]::new(-1)

$name = if ([LucidityDpiNative]::AreDpiAwarenessContextsEqual($context, $perMonitorV2)) {
    'PerMonitorV2'
}
elseif ([LucidityDpiNative]::AreDpiAwarenessContextsEqual($context, $perMonitor)) {
    'PerMonitor'
}
elseif ([LucidityDpiNative]::AreDpiAwarenessContextsEqual($context, $systemAware)) {
    'SystemAware'
}
elseif ([LucidityDpiNative]::AreDpiAwarenessContextsEqual($context, $unaware)) {
    'Unaware'
}
else {
    'Unknown'
}

[pscustomobject]@{
    ProcessId = $ProcessId
    WindowHandle = $handle
    Context = $context
    Awareness = $name
    IsPerMonitorV2 = $name -eq 'PerMonitorV2'
}
