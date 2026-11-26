# Workflow entry points. CI and humans call these, nothing else.
# Targets not implemented yet print SKIP and succeed.
# EDIT: replace each body with the project's real commands.

.PHONY: lint typecheck test-unit test-integration security migrations-check hooks

lint:
	@echo "SKIP lint (EDIT: e.g. npm run lint)"

typecheck:
	@echo "SKIP typecheck (EDIT: e.g. npm run typecheck)"

test-unit:
	@echo "SKIP test-unit (EDIT: e.g. npm test)"

test-integration:
	@echo "SKIP test-integration (EDIT: real dependencies in podman containers, e.g. Testcontainers with DOCKER_HOST=unix://$$XDG_RUNTIME_DIR/podman/podman.sock)"

security:
	gitleaks detect --no-banner --redact
	semgrep scan --config p/default --error --severity ERROR --quiet
	trivy fs --exit-code 1 --severity HIGH,CRITICAL --scanners vuln,secret,misconfig --quiet .

# Optional, if the project has DB migrations: refuse edits to migrations main already has,
# warn on DROP/RENAME/type changes. Enable it with a line `lint: migrations-check`.
# EDIT: the migrations folder.
MIGRATIONS ?= db/migrations
migrations-check:
	bash .github/scripts/check-migrations.sh $(MIGRATIONS)

# Local git hooks (soft: --no-verify skips them): gitleaks on each commit, lint, typecheck
# and unit tests on each branch push.
hooks:
	git config core.hooksPath .githooks
