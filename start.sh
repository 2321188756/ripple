#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"

echo "===================================="
echo " Ripple - AI Chat Assistant"
echo "===================================="
echo ""
echo " API: http://192.168.0.123:3000/v1"
echo " Model: deepseek-v4-flash"
echo ""

# Check Node.js
if ! command -v node &>/dev/null; then
    echo "[ERROR] Node.js not found. Install 18+ from https://nodejs.org"
    exit 1
fi

# Check Rust
if ! command -v rustc &>/dev/null; then
    echo "[ERROR] Rust not found. Install from https://rustup.rs"
    exit 1
fi

# Install npm deps if needed
if [ ! -d "node_modules" ]; then
    echo "[INFO] Installing npm dependencies..."
    npm install || { echo "[ERROR] npm install failed"; exit 1; }
fi

echo "[INFO] Starting... first build takes ~1-2 minutes"
echo ""
echo "  Press Ctrl+C to stop"
echo "===================================="
echo ""

npx tauri dev
