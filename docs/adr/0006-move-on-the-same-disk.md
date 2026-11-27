# 0006. On the same disk, the files can be moved instead of copied
Date: 2026-11-27 · Status: Accepted (partly supersedes 0003)
## Context
ADR 0003 makes the app copy only. When the destination is on the source's disk (reordering
a folder), a copy takes twice the space and erasing the originals is refused (same device),
so the user ends up with both. The app and the CLI must offer the same choice.
## Options
- A: rename each file into the tree (never over an existing file: hard link then unlink,
  check-then-rename where there are no hard links), offered when a test rename from the
  source folder to the destination works; logged as `moved` (size, no hash: the bytes
  never change); undo renames back. Instant, no extra space.
- B: keep copy-then-Trash: double the space until the Trash is emptied, slow.
- C: hard links only: fails on FAT/exFAT and keeps two names until the original is removed.
## Decision
A. The preview offers Move (preselected) or Copy when the test rename works; the CLI has
`apply --move`. A symbolic link is copied and verified instead (moving it would move the
link). A file that can't be renamed after all (e.g. another volume: EXDEV) is reported as
failed and stays where it was. Source folders are left in place. When a copy wouldn't fit,
the preview still offers the move.
## Consequences
Each copy or move has a run id in its log lines; undoing a move takes back that run only
(`exportpro undo --run <id>`), never erased originals or other moves. On file systems
without hard links (FAT, exFAT) the no-replace check and the rename are two steps, as for
copies: a file created at that very name in between would be replaced.
The log gains `moved` and `moved-back` events (format version unchanged: older versions
report such lines as unreadable). Undo reads them from the same log. After a move, or a
copy on the same disk, the app doesn't ask to erase the originals.
