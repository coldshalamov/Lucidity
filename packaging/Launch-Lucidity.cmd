@echo off
setlocal

set "LUCIDITY_START=%~dp0Start-Lucidity.ps1"

if not exist "%LUCIDITY_START%" (
    echo Lucidity startup script is missing.
    echo Expected: "%LUCIDITY_START%"
    pause
    exit /b 1
)

start "Lucidity" powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "%LUCIDITY_START%"

endlocal
exit /b 0
