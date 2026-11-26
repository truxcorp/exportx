# Workflow entry points (.workflow.yml). CI and humans call these, nothing else.
# Targets run with the tools on the current machine; `make box T="..."` runs them in the
# Containerfile image instead (CI does: make ci). Targets not implemented yet print SKIP.

CONTAINER ?= $(shell command -v podman || command -v docker)
# The image tag follows its inputs, so a changed Containerfile builds a new image.
IMAGE ?= localhost/exportpro-dev:$(shell cat Containerfile rust-toolchain.toml | sha256sum | cut -c1-12)
# The image's user is non-root: run it as the caller's uid so it can write to the checkout
# and leaves files the caller owns (podman: keep-id maps the uid through the user namespace).
AS_ME = --user "$$(id -u):$$(id -g)" $(if $(filter podman,$(notdir $(CONTAINER))),--userns=keep-id)
T ?= lint typecheck test-unit security

.PHONY: lint typecheck test-unit test-integration security hooks image box ci

## lint: rustfmt check and clippy (warnings fail)
lint:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --locked -- -D warnings

## typecheck: cargo check
typecheck:
	cargo check --workspace --all-targets --locked

## test-unit: Rust tests, the bash version's tests and the guard hook's tests
test-unit:
	cargo test --workspace --locked
	bash tests/test_exportpro.sh
	bash tests/test_guard.sh

## test-integration: none yet
test-integration:
	@echo "SKIP test-integration (none yet)"

## security: gitleaks, semgrep, trivy; high or worse fails
security:
	scripts/security.sh

## hooks: once per clone: gitleaks on each commit, lint/typecheck/unit before each push
hooks:
	git config core.hooksPath .githooks

## image: build the dev/CI image (skipped when this tag exists)
image:
	@[ -n "$(CONTAINER)" ] || { echo "needs podman or docker (or CONTAINER=...)" >&2; exit 1; }
	$(CONTAINER) image inspect $(IMAGE) >/dev/null 2>&1 || $(CONTAINER) build -t $(IMAGE) -f Containerfile .

## box: run `make $(T)` in the image, with the checkout mounted
box: image
	$(CONTAINER) run --rm $(AS_ME) -v "$(CURDIR):/src:z" -e CI -e CARGO_TARGET_DIR=/src/target/box \
	  $(IMAGE) make $(T)

## ci: what pr.yml runs
ci: box
