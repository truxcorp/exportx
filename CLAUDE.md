# Project instructions

@.workflow/rules.md

## Project rules

- Merging: squash by default. Rebase-merge (`gh pr merge <n> --rebase --delete-branch`) is
  allowed when a PR's commits are each meaningful and worth keeping in the trunk's history.
- Keep `ExportPro.command`, `old_for_ref/` and `tests/test_exportpro.sh` (the bash version):
  they may still get work. The desktop app is built next to them, never in their place.
- Notes vault pushes from this repo's sessions: `git -C ~/src/ObsiOne-1 push`, exactly that
  (no `cd` before it, no redirection such as `2>&1`): the guard only lets a single
  `git -C <dir> push` reach the vault's `main` (`project.direct_push_remotes`).

## Current work

Desktop app ExportX (Tauri v2, Rust core): the MVP plan `docs/plan/tauri-prototype.md` is
done; decisions in `docs/adr/`. Next steps: its "After the MVP" section.
