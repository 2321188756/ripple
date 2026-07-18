---
name: verify
summary: Drive Ripple's native Tauri surface with isolated WebdriverIO smoke tests.
---

# Ripple runtime verification

## Native desktop surface

1. Build the test-only native binary and frontend bridge:
   `npm run build:e2e:native`
2. Drive the packaged-style Tauri application through WebView2:
   `RIPPLE_E2E_BINARY=D:/AI/Ripple/src-tauri/target/release/ripple-app.exe npm run test:e2e:native`
3. The smoke flow must observe:
   - native window title and `ping` IPC;
   - empty isolated KB database;
   - opening the independent settings window;
   - switching to Logs and seeing `Ripple starting` from the real native log.
4. Verify `.e2e-data/ripple.db`, `.e2e-data/.ripple-install-secret`, and `.e2e-data/logs/` exist.
5. Build production frontend with `npm run build`, then confirm `dist/` contains neither `WDIO Tauri Plugin` nor `wdioTauri`.

## Gotchas

- The Tauri service may print a harmless teardown warning about clearing the mock store after the WebDriver session closes; the spec result remains authoritative.
- `build:e2e:native` sets `VITE_RIPPLE_E2E=true` and enables Rust feature `e2e`; normal builds must not contain the test bridge.
- Use `RIPPLE_DATA_DIR` isolation. Existing root-level `ripple.db`, `.ripple-install-secret`, or `logs/` may predate E2E and should not be used as smoke evidence.
