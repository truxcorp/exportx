# 0005. End-to-end UI tests with tauri-driver, WebKitWebDriver and Xvfb
Date: 2026-11-27 · Status: Accepted
## Context
The app's screens (`ui/app.js`) and their Tauri wiring (`crates/app/src/main.rs`) had no
automated test: the flow and the commands below them are tested, the glue between them is
not. Native folder pickers and drag and drop can't be driven by WebDriver.
## Options
- A: `tauri-driver` (Tauri's WebDriver server) + WebKitGTK's `WebKitWebDriver` + Xvfb in the
  dev image; tests in `node:test` with a small WebDriver client over `fetch` (no npm
  dependency). Drives the real app, its Rust commands and WebKitGTK.
- B: the same stack with WebdriverIO: more complete client, large npm dependency tree.
- C: Playwright on `ui/` with a fake `window.__TAURI__`: fast, but tests neither the real
  app nor WebKitGTK.
## Decision
A, approved by the human. Folders are entered through a paste field on the source and
destination screens (also useful to users), since the native pickers can't be automated.
Each test starts its own `tauri-driver` with its own HOME, config folder, Trash and
language, and saves a screenshot per screen in `$CARGO_TARGET_DIR/e2e-screenshots/`
(`target/box/e2e-screenshots/` in the image, which CI uploads).
## Consequences
`make test-e2e` (in the image, CI too) builds the app and runs `e2e/*.test.js` under Xvfb.
Linux/WebKitGTK only: macOS has no WebDriver for WKWebView, Windows (WebView2) would need
msedgedriver, both after the MVP. The pickers and drag and drop stay manual checks.
