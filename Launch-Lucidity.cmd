@echo off
setlocal

set "LUCIDITY_START=%~dp0dist\Lucidity\Start-Lucidity.ps1"

if not exist "%LUCIDITY_START%" (
    echo Lucidity is not packaged yet.
    echo Expected launcher script: "%LUCIDITY_START%"
    echo.
    echo Run scripts\package-windows.ps1 first.
    pause
    exit /b 1
)

start "Lucidity" powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "%LUCIDITY_START%"

endlocal
exit /b 0
