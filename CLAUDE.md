# Project instructions

@.workflow/rules.md

## Project rules

- Merging: squash by default. Rebase-merge (`gh pr merge <n> --rebase --delete-branch`) is
  allowed when a PR's commits are each meaningful and worth keeping in the trunk's history.
- Keep `ExportPro.command`, `old_for_ref/` and `tests/test_exportpro.sh` (the bash version):
  they may still get work. The desktop app is built next to them, never in their place.

## Current work

Desktop app (Tauri v2, Rust core): the approved plan is `docs/plan/tauri-prototype.md`,
decisions in `docs/adr/`. Start from the first unmerged slice (`gh pr list --state all`).
