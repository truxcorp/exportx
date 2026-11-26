---
name: workflow-feature
description: Build a feat or fix with strict TDD (RED → GREEN → REFACTOR), mostly integration tests. Load before writing any code for a new feature, a behaviour change, or a bug fix.
---

# Feature or fix: TDD

The rules in `.workflow/rules.md` still apply. Plan and ADRs first: see `workflow-git`
(its Plan section). Plan approval gives you the behaviour list; each behaviour becomes
one test, in that order, with a test level (integration by default, E2E for a
user-visible flow, unit for pure logic with many cases).

## 0. Is TDD on?
Read `tdd.enabled` in `.workflow.yml`.
- **On** (the default): follow everything below.
- **Off**, because `enabled: false` or because the human said "no TDD" for this task:
  - Tests are still required, but they may be written after the code, in the same
    commits.
  - `verify-tdd.sh` reports SKIP when `enabled: false`. For a chat override it will
    fail; explain that in the PR.
  - Start the PR body with `TDD off: <reason>`.
- Never edit the `tdd` block yourself unless the human asks in this session.

Plumbing (paths in `tdd.exempt`: CI, infra, deploy, release, hooks, build files) is out
of scope: no TDD loop and no new tests. It goes in `ci:`, `build:` or `chore:` commits,
on its own branch when it can, and is proven by running it.

## 1. TDD loop, once per behaviour, in plan order
**A. RED**
- Write **one** test for the next behaviour, in its final form: real assertions, no
  "TODO" or "expected to fail" comments.
- Run it. It must fail **for the expected reason**: an assertion on the missing
  behaviour, not an import error, typo, fixture or setup problem. If it fails for
  another reason, fix the test setup and run it again. If it passes immediately, the
  behaviour already exists or the test is wrong: fix the test, or drop the behaviour
  and list it under `## Decisions for you`.
- Commit only the test files: `test(red): <behaviour>`. In the body, paste the
  one-line failure message as proof.

**B. GREEN**
- Write the **minimum** production code to make that test pass. No extra features and
  no speculative code: behaviour without a test doesn't get written.
- **Never edit test files in this step.** If a test you wrote on this branch looks
  wrong, fix it in a new `test(red)` commit that says why, and list it under
  `## Decisions for you`. A test that was on the trunk (`project.default_branch`) before your branch: leave the PR as
  a draft with the question (`workflow-git`, Plan). Never weaken an assertion to get
  green.
- Run the new test and the related suite; all must pass. Commit: `feat: …` or `fix: …`.

**C. REFACTOR**
- Improve the design (names, duplication, structure) with every test green after each
  change. No behaviour change.
- Commit `refactor: …` for production code. Test-code cleanup goes in a separate
  `refactor(test): …` commit, which may not change any expected value or assertion.
- Skip this step if there is nothing worth improving. Don't invent work.

Forbidden at every step:
- skip, xfail or `.only` markers,
- assertions that compute the expected value with the code under test,
- mocking the unit under test,
- deleting a failing test.

## 2. Test strategy: integration first, unit tests sparingly
- **Integration (default):** run the real app layer against real dependencies (DB,
  object storage, queues) in containers (`tests.containers`, default **podman**).
  Pulling images is allowed. With Testcontainers, set `DOCKER_HOST` to the podman
  socket. Each test owns its data (unique IDs, or a fresh schema or transaction) so
  tests can run in any order.
- **Mocks** only at third-party boundaries you don't own (payment, email, external
  APIs). Use a local fake or a recorded contract, never a mock of your own DB.
- **Unit tests** only for pure logic with many cases (parsing, pricing, rules).
- **E2E:** for user-visible flows, as the plan calls for.
- **Bug fix:** the RED test reproduces the bug, through the same path users take.
- **Tools:** use what's installed. **Ask before installing** anything on the machine.
  Container images are fine to pull.

## 3. Before opening the PR
1. Run all local checks (lint, typecheck, unit, integration).
2. Verify the TDD commit discipline (the regex is `tests.regex` in `.workflow.yml`; the
   script reads it itself):
   ```sh
   .claude/skills/workflow-feature/verify-tdd.sh            # base: origin/<trunk> from .workflow.yml
   ```
   The script fails if a commit other than `test(red)` or `refactor(test)` touches test
   files, or if a `feat`/`fix` branch has no `test(red)` commit. It warns when a
   `ci`/`build`/`chore` commit touches a path outside `tdd.exempt`: if that path is
   plumbing, suggest it under `## Decisions for you`. Fix the history
   (interactive rebase on your own branch, then `--force-with-lease`) or explain the
   exception in the PR.
3. Push, open the PR (concise), tick "RED seen for each new test", and link the ADR if
   there is one. Then report the checks and merge as `workflow-git` says.
