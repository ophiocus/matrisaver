@echo off
set SCRIPT=%~dp0run_windows_multi.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%" -EnableOverlay
