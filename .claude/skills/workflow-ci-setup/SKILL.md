---
name: workflow-ci-setup
description: Set up or change PR checks - the pr.yml and security.yml GitHub Actions workflows, Makefile check targets, local git hooks, the migration check, self-hosted runner, action pinning, Dependabot. Use when creating or editing anything in .github/workflows, .githooks, the Makefile, or runner config.
---

# CI setup: PR checks

Templates are in `.claude/skills/workflow-ci-setup/templates/`. Copy them, never
overwrite existing files (merge them), and fill every `EDIT:` marker from
`.workflow.yml`. Ask the human for anything missing.

## What runs
| Where | When | What |
|---|---|---|
| `.githooks/pre-commit` | each commit (soft) | gitleaks on the staged changes |
| `.githooks/pre-push` | each branch push (soft) | `commands.lint`, `typecheck`, `unit` |
| `pr.yml`, the only PR workflow | pull_request, not docs-only | lint, typecheck, unit, security; integration (`ci.integration`) only if relevant paths changed |
| `security.yml` | weekly + `workflow_dispatch` | `make security` (and dependency audits), for new CVEs on unchanged code |

All workflows run on `RUNNER_LABELS`. Copy targets: `pr.yml` and `security.yml` →
`.github/workflows/`, `dependabot.yml` → `.github/`, `githooks/*` → `.githooks/`
(executable), `check-migrations.sh` → `.github/scripts/` (executable). Then
`make hooks`, and tell the human each clone runs it once.

Checks before the release are soft and cheap to re-run: no PR has to contain `main`'s
tip, so there is no up-to-date check.

Rules:
- **YAML stays thin.** Workflows only call `make` targets (`ci.security` for scans,
  `ci.integration` for integration tests). All logic lives in the Makefile and scripts,
  so the CI host stays replaceable.
- Every workflow declares `permissions: contents: read` and a `concurrency` group.
- Every job sets `timeout-minutes`, a little above its usual duration. GitHub's default
  is 6 h, and a stuck job on a single runner blocks everything for that long.
- **No secrets in `pr.yml`.** Only the human sets secrets (`gh secret` is denied to agents);
  never read or echo their values.
- Never interpolate user-controlled input (`${{ github.event.* }}`, `inputs.*`) directly
  in `run:`. Pass it through `env:`.
- Agents can't write commit statuses or checks (the guard hook blocks writes to
  `/statuses` and `/check-runs`; reads are fine), and `gh auth token` is denied.

## Runner selection and fallback
Repo variable `RUNNER_LABELS` holds a JSON value used by
`runs-on: ${{ fromJSON(vars.RUNNER_LABELS || '"ubuntu-latest"') }}`.
- Self-hosted: `gh variable set RUNNER_LABELS --body '<ci.runner_labels>'`
- Fallback when the self-hosted runner is offline:
  `gh variable set RUNNER_LABELS --body '<ci.runner_fallback>'`

## Self-hosted runner hygiene (AI-written code runs on it)
- Run jobs in containers, or use an ephemeral runner (`--ephemeral`) inside a container,
  so each job starts clean. Never run jobs directly on the host.
- Isolate the runner on the network (its own VLAN, or firewall rules so it can't reach
  other LAN machines).
- Private repos only. Never attach a self-hosted runner to a public repo.
- Keep the runner updated (auto-update on), or jobs stop being queued to it.

Speed, on a single self-hosted runner:
- Jobs queue behind one another, so a PR's wall time is the sum of its jobs. More
  runners in parallel is the main speed-up.
- Bake the toolchain and scanners into the runner or job image, including compilers
  some test modes need (e.g. a C toolchain for Go's `-race`).
- On an ephemeral runner, `setup-*` caches are downloaded and re-saved on every job for
  nothing: turn them off, or rely on the baked image.
- Watch memory: a large build can exceed a container's limit.

## Pin actions to commit SHAs
The templates contain `uses: owner/repo@PIN # vN`. Resolve them before the first commit:
```sh
.claude/skills/workflow-ci-setup/templates/pin-actions.sh .github/workflows/*.yml
```
The script replaces each `PIN` with the commit SHA of the version in the comment
(using `gh api`). To update later: change the `# vN` comment back to `PIN`, or let
Dependabot (`github-actions` ecosystem) open the update PRs.

## Makefile contract
Targets the workflows call: `lint typecheck test-unit test-integration security`; plus
`hooks` and the optional `migrations-check`.
- Targets not implemented yet print `SKIP` and exit 0.
- `security` runs gitleaks, semgrep and trivy and fails on high severity or above.
- The runner, or its job container image, must provide `make`, `gitleaks`, `semgrep`,
  `trivy` and the project toolchain. Prefer one CI image with all of these, so jobs
  don't download tools on every run.

## Migration check (only if the project has DB migrations)
`check-migrations.sh` refuses edits to migrations the base branch already has and warns
on DROP, RENAME and type changes. Set `MIGRATIONS` in the Makefile and add the line
`lint: migrations-check`. `pr.yml` fetches the base branch for it.

## Dependabot
Copy `dependabot.yml` with one entry per package ecosystem, plus `github-actions`:
weekly, minor and patch updates grouped into one PR per ecosystem, each major on its
own. Each update is a normal PR: checks, human review.
