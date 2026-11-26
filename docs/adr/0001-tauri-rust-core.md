# 0001. Desktop app with Tauri v2, a Rust core and plain HTML/JS screens
Date: 2026-11-26 · Status: Accepted
## Context
`ExportPro.command` only runs on macOS (bash 3.2 + osascript dialogs). The tool must
become a small app for macOS, Windows and Linux, used by a photographer who doesn't
code: one download, double-click, native folder pickers, French and English.
It moves the only copy of someone's photos, so the logic must be well tested.
## Options
- A: Tauri v2 (Rust core + HTML/JS UI) — small bundles (~5–10 MB), native dialogs and
  drag and drop, installers, updater and signing support; two languages, OS webviews
  differ slightly (mostly on Linux).
- B: Go + Wails or Fyne — simpler language; Fyne looks non-native with its own folder
  picker, both need cgo for cross builds, Wails is less mature.
- C: Python + PySide6 — fastest to write; 60–150 MB bundles, antivirus false positives,
  per-OS builds, fiddly notarization.
- D: Rust native UI (egui/Slint/iced), E: Electron, F: keep bash with per-OS dialogs —
  less native / far too heavy / no Windows and no real robustness gain.
## Decision
A. Layout: `crates/core` (pure logic, no UI: scan → plan → apply / erase / undo),
`crates/cli` (same features in a terminal, used by tests), `src-tauri` + a plain
HTML/JS front end with no framework. The core's API is the contract the UI and CLI share.
## Consequences
The CLI makes every behaviour testable without the UI. Linux builds need webkit2gtk
4.1 (provided by the `Containerfile`). The bash script stays in the repo for reference.
