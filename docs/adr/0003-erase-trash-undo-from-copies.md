# 0003. Copy only; erasing originals goes to the Trash and can be undone from the copies
Date: 2026-11-26 · Status: Accepted
## Context
v0.2.0 moves files. The app copies instead (often SD card → disk), then asks whether to
erase the originals, and that erase must be undoable on macOS, Windows and Linux.
## Options
- A: erase = move the originals to the system Trash (`trash` crate); undo = copy them
  back from the verified copies, using the log. Works the same on all OSes; the Trash
  is an extra, familiar safety net.
- B: erase = Trash, undo = restore from the Trash — programmatic restore isn't
  supported on macOS by the `trash` crate.
- C: the app keeps its own hidden backup of the originals — double the disk space.
## Decision
A. Only originals whose copy was verified (size + hash, logged) are erased. Undo reads
`exportpro.log` in the shoot folder, re-verifies the hash of each copy, and restores
the original name and folder.
## Consequences
The log is part of the contract: JSON lines with a format version and the layout used,
so undo works even after the settings change. Trash on removable volumes behaves per
OS (e.g. `.Trashes` on macOS); covered by integration tests on Linux, manual tests elsewhere.
