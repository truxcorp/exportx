---
name: workflow-git
description: Daily git flow with exact commands (branch, commit, push, PR, checks, merge), PR conventions, major-PR review, and the plan and ADR format. Use when creating a branch, committing, opening or merging a PR, or planning a feat or fix.
---

# Daily flow: branch, PR, merge

The rules in `.workflow/rules.md` still apply (approvals, Never). Values such as
commands come from `.workflow.yml`. `<trunk>` below is `project.default_branch` from
`.workflow.yml` (`main` when unset): read it before step 1.

## Daily flow (exact commands)
1. `git fetch --prune && git switch -c <type>/<slug> origin/<trunk>`. Types: `feat fix chore
   refactor docs test perf`. Uncommitted changes on `<trunk>` come along to the new branch
   (`git switch -c <type>/<slug>`). If an open PR already covers the task
   (`gh pr list --state open`), continue it instead.
2. **Parallel agents:** one agent per branch, each in its own worktree. Use
   `git worktree add ../<repo>-<slug> -b <type>/<slug> origin/<trunk>`, or Claude Code's
   worktree option. Never have two agents in one checkout.
3. Commit with Conventional Commits (`feat: …`, `fix: …`; a breaking change uses `!`).
4. **Before every push**, run the local checks from `.workflow.yml`
   (`commands.lint`, `commands.typecheck`, `commands.unit`). If any fails, fix it
   before pushing.
5. `git push -u origin HEAD`
6. `gh pr create --base <trunk> --title "<type>: <summary>" --body-file <tmpfile>`, following the PR
   template (`.github/pull_request_template.md`). Add `--draft` if you are blocked.
7. `gh pr checks <n> --watch`, then report every check's result honestly. Never claim
   green without having seen it. A failed check: read its log
   (`gh run view <id> --log-failed`); if it looks flaky, re-run it once
   (`gh run rerun <id> --failed`). A second failure is real: fix it.
8. Rebase only if the PR has a conflict with `<trunk>` (a PR needn't contain its tip):
   `git fetch && git rebase origin/<trunk> && git push --force-with-lease`.
9. Merge once green: with `approvals.standing_merge: true` right away, otherwise after
   "GO" (always after "GO" when the PR changes `project.default_branch` or
   `project.protected_branches`). `gh pr merge <n> --squash --delete-branch`.
10. Next task or slice: `git switch --detach origin/<trunk> && git branch -D <type>/<slug>`,
    then step 1 again from a fresh `origin/<trunk>`.

## PRs
- Small: one concern, mergeable within 1–2 days. Split larger work into slices.
- The title becomes the squash commit, so it follows Conventional Commits.
- The body is **concise**: "What / why" in 1–3 lines; other sections one line or N/A.
- `## Decisions for you`: each choice the plan didn't settle, one line with the
  alternative you didn't take, so the human can revisit it after the merge.

## Code review before merging
With `review.auto: true` (the default), review a PR once its checks are green:
- **Sensitive**, whatever its size: it touches a DB migration, auth or security,
  infrastructure code, or a public API contract, or implements an ADR for the first
  time. Run `/code-review high <n>`.
- Always name the level: without one, `/code-review` reuses the last level used.
- **Substantial**: at least `review.min_lines` changed lines, not counting docs and
  lockfiles. Run `/code-review medium <n>`.
  ```sh
  git diff --shortstat origin/<trunk>...HEAD -- . ':(exclude)*.md' ':(exclude)*.lock' ':(exclude)*-lock.*'
  ```

Then:
- Fix the bug findings you agree with in the same PR, each starting with a test that
  reproduces the bug. List the ones you don't fix, with why, under
  `## Decisions for you`.
- Push, wait for green again, then merge. Don't review the same PR twice unless the
  fixes changed more than `review.min_lines` lines.
- For very risky changes, also mention `/code-review ultra` to the human (deeper, costs
  more); don't wait for it.

With `review.auto: false`, say "Substantial PR (<reason>): run `/code-review <n>`?"
instead, and run it only if the human agrees.

## Plan (before writing code for a feat or fix; scale it to the work)
1. Read the relevant code first. **Ask** about anything unclear now: this is the one
   moment you stop for the human.
2. Write the plan in the conversation:
   - **Behaviours:** one line each, in "given / when / then" form. Each gets a test.
   - **Light work** (1 PR, a few behaviours): the list is the plan.
   - **Heavy work:** add slices, each one PR of 1–2 days. **Say clearly that it is
     heavy**: number of slices, rough size.
3. **Architecture decisions** belong in the plan: a new dependency or service, a data
   model or migration, a public API contract, infra or cost, security or auth, or a
   pattern that will spread across the codebase. Give 2–3 options, with trade-offs and
   your recommendation, as a draft ADR.
4. **Plan approval** accepts the behaviours, the slices and the ADRs in it: commit each
   ADR as `Status: Accepted` in `project.adr_path`, in the first PR that depends on it.
   Then work without stopping until the last slice is merged (or green and waiting for
   "GO").
5. **Decisions found mid-work:**
   - A small one (naming, a missing edge case, a test you wrote on this branch that
     turns out wrong): take the simplest reversible choice and list it under
     `## Decisions for you`. A wrong test of yours gets fixed in a new `test(red)`
     commit, explained there.
   - An architecture decision: pick the most reversible option, commit its ADR as
     `Status: Proposed`, and open an issue
     `gh issue create --title "Review ADR NNNN: <title>" --body "<options, choice, PR link>"`.
     The human accepts it by closing the issue, or rejects it, and you then write a new
     ADR that supersedes it.
   - A test that was on `<trunk>` before your branch looks wrong, a tool to install, a
     secret, or the plan contradicting the code: leave the PR as a draft with the
     question, and move on to the next slice or task.
6. Every `feat` and `fix` ships with tests. A fix includes a test that reproduces the
   bug.

## Long unattended sessions
- Open PRs are the progress record: after a break or a context summary, start from
  `gh pr list --author @me --state open` and their `## Decisions for you`.
- Work through slices in order; one PR at a time unless the plan says they are
  independent.
- Never leave `<trunk>` broken: if a merge turns `main.yml` or `pr.yml` red, the fix PR
  comes before anything else.

## ADR template (`<adr_path>/NNNN-<slug>.md`, numbered in sequence)
```md
# NNNN. <Decision title>
Date: YYYY-MM-DD · Status: Accepted | Proposed (issue #N) | Superseded by NNNN
## Context
<2–4 lines: the problem and the constraints>
## Options
- A: <option> — <trade-offs>
- B: <option> — <trade-offs>
## Decision
<chosen option, approved with the plan or proposed by the agent, and why in 1–2 lines>
## Consequences
<what becomes easier or harder; follow-ups>
```
