@echo off
setlocal
cd /d "%~dp0"

echo ====================================
echo  Ripple - AI Chat Assistant
echo ====================================
echo.

where node >nul 2>nul
if errorlevel 1 (
    echo [ERROR] Node.js is required. Install Node.js 18 or newer.
    pause
    exit /b 1
)

where rustc >nul 2>nul
if errorlevel 1 (
    echo [ERROR] Rust is required. Install it from https://rustup.rs
    pause
    exit /b 1
)

if not exist "node_modules" (
    echo [INFO] Installing npm dependencies...
    call npm install
    if errorlevel 1 (
        echo [ERROR] npm install failed.
        pause
        exit /b 1
    )
)

echo [INFO] Starting Ripple from source...
echo [INFO] The first Rust build may take a few minutes.
echo [INFO] Press Ctrl+C to stop.
echo.

call npm run tauri -- dev
set "RIPPLE_EXIT_CODE=%ERRORLEVEL%"

if not "%RIPPLE_EXIT_CODE%"=="0" (
    echo.
    echo [ERROR] Ripple exited with code %RIPPLE_EXIT_CODE%.
    pause
)

exit /b %RIPPLE_EXIT_CODE%
