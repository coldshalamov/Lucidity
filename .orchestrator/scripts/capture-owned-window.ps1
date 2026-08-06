param(
    [Parameter(Mandatory = $true)]
    [int] $ProcessId,

    [Parameter(Mandatory = $true)]
    [string] $OutputPath
)

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class LucidityWindowCaptureNative {
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool GetWindowRect(IntPtr hWnd, out Rect rect);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, uint flags);
}
'@

$process = Get-Process -Id $ProcessId -ErrorAction Stop
$handle = $process.MainWindowHandle
if ($handle -eq [IntPtr]::Zero) {
    throw "Process $ProcessId has no main window"
}

$rect = [LucidityWindowCaptureNative+Rect]::new()
if (-not [LucidityWindowCaptureNative]::GetWindowRect($handle, [ref] $rect)) {
    throw "GetWindowRect failed for process $ProcessId"
}

$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
if ($width -le 0 -or $height -le 0) {
    throw "Invalid window bounds ${width}x${height} for process $ProcessId"
}

$bitmap = [System.Drawing.Bitmap]::new(
    $width,
    $height,
    [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$deviceContext = $graphics.GetHdc()

try {
    $renderFullContent = 2
    if (-not [LucidityWindowCaptureNative]::PrintWindow($handle, $deviceContext, $renderFullContent)) {
        throw "PrintWindow failed for process $ProcessId"
    }
}
finally {
    $graphics.ReleaseHdc($deviceContext)
    $graphics.Dispose()
}

try {
    $bitmap.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Png)
}
finally {
    $bitmap.Dispose()
}

Get-Item -LiteralPath $OutputPath | Select-Object FullName, Length, LastWriteTimeUtc
