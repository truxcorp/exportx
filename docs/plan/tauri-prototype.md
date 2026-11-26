# Plan: ExportPro desktop app (Tauri v2 MVP)

Status: **approved 2026-11-26**, ADRs 0001–0004 accepted. Heavy work: 5 slices, about 5–7 days.
Start with slice 1. Read `CLAUDE.md`, `.workflow/rules.md` and the ADRs in `docs/adr/` first.

## Goal
Replace `ExportPro.command` (macOS-only bash) with a small cross-platform desktop app
(macOS, Windows, Linux) for a photographer who doesn't code: pick a source and a
destination, name the shoot, preview, copy, then optionally erase the originals.
The bash script and `old_for_ref/` stay in the repo untouched; the app reimplements
their behaviour (`tests/test_exportpro.sh` is the reference).

## Decisions taken
- Stack: Tauri v2, Rust core, plain HTML/JS screens: ADR 0001.
- Capture dates: `nom-exif`, file times as fallback: ADR 0002.
- Copy only; erasing originals is a separate, undoable step: ADR 0003.
- Configurable tree layout (`p|v` → `photo|video`, …): ADR 0004: every path part is a variable; numbers zero-padded (`txo_0001`).
- UI and CLI in French and English; the system language picks the default.
- CI: **Linux only, on the truxcorp org's self-hosted Linux runners** (`ci.runner_labels`
  in `.workflow.yml`). If jobs don't get picked up or fail for runner reasons, stop and
  ask the human; don't switch to GitHub-hosted runners on your own.
  GitHub-hosted macOS / Windows runners come after the MVP, so the first Mac test
  happens after the MVP too.
- Dev environment: one `Containerfile` on `ubuntu:26.04` (Rust, Node, Tauri Linux libs,
  gitleaks, semgrep, trivy), used as the CI job image and as a local toolbox/distrobox
  (the dev host is Fedora Silverblue, where Tauri's libs aren't installed).

## Behaviours (each gets a test)

**Scan and plan (core)**
1. Given a source folder, when it is scanned, then top-level files are listed, hidden
   files skipped, files sharing a base name (`IMG_1.CR2` + `IMG_1.JPG`) form one group,
   and a group with any video file is a video group. Port the 42 bash test cases.
2. Given the date field is filled in, then that date is used, whatever the files say.
3. Given no date entered, then each file's date is its stored capture date (EXIF for
   CR2, CR3, NEF, ARW, DNG, HEIC, JPG; QuickTime/MP4 creation date for videos), falling
   back to the earliest of the file's creation and modification times.
4. Given the files span several days, then the user is shown each day with its file
   count and chooses either **one day** (all files go into that day's folder) or
   **each day in its own date folder** (files split by their own day).
5. Given the user chooses "no date", then the date part of the tree is dropped
   (`<destination>/<shoot>/…`).
6. Numbers follow capture time, then name, and continue after the highest
   `<prefix>_N` already in the target shoot folder (per day folder when split).
7. Names invalid on any OS are refused (`\ / : * ? " < > |`, control chars, Windows
   reserved names such as `CON`, trailing dot or space, leading dot), and so are two
   destination names differing only by case.
8. A preview lists every `original → destination`; the plan is refused if any
   destination file already exists.

**Copy, then erase on request (core)**
9. On confirm, each file is copied, fsynced and verified (size + hash), and logged to
   `exportpro.log` as one JSON line per file (format version, layout used, source,
   destination, hash). Originals are untouched. Empty "edited" folders are created;
   on macOS extended attributes are stripped from the copies (v0.1.2 "xattr 164" fix).
10. If a copy fails, it is reported and logged; copies already done stay; originals
    are untouched.
11. After a successful copy, the app asks whether to erase the originals; yes moves
    only originals whose copy was verified to the system Trash.
12. "Undo erase" restores the originals from the verified copies, at their original
    names and places, on all three OSes.

**App (Tauri)**
13. Flow: source (folder picker or drag and drop) → destination → shoot name, prefix
    (default `txo`), optional date → day choice if needed → preview → copy progress →
    result with "Open folder" → erase question.
14. Language follows the system, with an FR/EN switch; CLI messages are translated too.
15. The app remembers the last destination, prefix and layout preset.

## Slices (one PR each)
| # | PR | Size |
|---|---|---|
| 1 | Scaffold + CI: Cargo workspace (`crates/core`, `crates/cli`), Makefile (`lint` = `cargo fmt --check` + clippy, `typecheck` = `cargo check`, `test-unit` = `cargo test` + `bash tests/test_exportpro.sh` + `bash tests/test_guard.sh`), `Containerfile` (ubuntu:26.04), `pr.yml` + `security.yml` on the self-hosted runner (Linux only), Dependabot (cargo, npm, github-actions), git hooks, `RUNNER_LABELS` repo variable. Use the `workflow-ci-setup` skill. | ~1 d |
| 2 | Core: scan, date rules, layout templates, plan; `exportpro plan`. Behaviours 1–8. | ~1.5 d |
| 3 | Core: copy + verify + log, erase to Trash, undo; `exportpro apply / erase / undo`. Behaviours 9–12. | ~1.5 d |
| 4 | Tauri app: screens, FR/EN, settings incl. layout presets. `build.yml` builds unsigned Linux AppImage + `.deb` on merge to `main` and on `workflow_dispatch`. Behaviours 13–15. | ~2 d |
| 5 | Polish: progress, wording, edge cases found while using it. | ~0.5–1 d |

After the MVP: GitHub-hosted macOS (universal `.dmg`) and Windows (`.msi` / NSIS)
builds, Mac test by the human, then signing / notarization (needs the human's certificates).

## Choices to list under "Decisions for you" in the PRs
- EXIF dates have no timezone: read as camera-local time. QuickTime dates are UTC:
  converted to the computer's timezone.
- With "one day" chosen, all files go into that day's folder, whatever their own date.
- Numbers are zero-padded to 4 digits by default (`txo_0001`); v0.2.0 used `txo_1`.
- Packages are unsigned during the MVP.
