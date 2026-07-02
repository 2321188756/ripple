@echo off
cd /d "%~dp0"

echo ====================================
echo  Ripple - AI Chat Assistant
echo ====================================
echo.
echo  API: http://192.168.0.123:3000/v1
echo  Model: deepseek-v4-flash
echo.

:: 检查 Node.js
where node >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Node.js 未安装，请从 https://nodejs.org 安装 18+
    pause
    exit /b 1
)

:: 检查 Rust
where rustc >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Rust 未安装，请从 https://rustup.rs 安装
    pause
    exit /b 1
)

:: 安装 npm 依赖（如需要）
if not exist "node_modules" (
    echo [INFO] 正在安装 npm 依赖...
    call npm install
    if %ERRORLEVEL% neq 0 (
        echo [ERROR] npm install 失败
        pause
        exit /b 1
    )
)

echo [INFO] 启动中... 首次编译需要 1-2 分钟
echo.
echo   Tauri 窗口打开后，在 Settings 里填入 API Key
echo   （已默认填好，首次可直接对话）
echo.
echo   按 Ctrl+C 停止
echo ====================================
echo.

npx tauri dev

if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Ripple 异常退出，错误码: %ERRORLEVEL%
    pause
)
