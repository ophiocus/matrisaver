@echo off
rem MatriSaver — launch the local release build in screensaver (/s) mode.
rem
rem Dismiss with: mouse moved > 4 pixels, or any key/click (after a
rem 500ms grace window so the launch keypress doesn't kill it instantly).
rem
rem Rebuild after code changes:
rem   cd rust && cargo build --release

set EXE=%~dp0rust\target\release\matrisaver-host-windows.exe
if not exist "%EXE%" (
    echo Release binary not found at:
    echo   %EXE%
    echo.
    echo Build it first: cd rust ^&^& cargo build --release
    pause
    exit /b 1
)

"%EXE%" /s
