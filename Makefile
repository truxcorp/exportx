# Workflow entry points (.workflow.yml). CI and humans call these, nothing else.
# Targets run with the tools on the current machine; `make box T="..."` runs them in the
# Containerfile image instead (CI does: make ci). Targets not implemented yet print SKIP.

CONTAINER ?= $(shell command -v podman || command -v docker)
# The image tag follows its inputs, so a changed Containerfile builds a new image.
# (shasum where sha256sum is missing, as on stock macOS)
IMAGE ?= localhost/exportpro-dev:$(shell cat Containerfile rust-toolchain.toml | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-12)
# The image's user is non-root: run it as the caller's uid so it can write to the checkout
# and leaves files the caller owns. Rootless podman maps that uid through its user
# namespace (keep-id, which rootful podman refuses).
ROOTLESS = $(if $(filter podman,$(notdir $(CONTAINER))),$(shell $(CONTAINER) info --format '{{.Host.Security.Rootless}}' 2>/dev/null))
AS_ME = --user "$$(id -u):$$(id -g)" $(if $(filter true,$(ROOTLESS)),--userns=keep-id)
# Build output of `make box`; the pre-push hook points it at the main checkout's.
TARGET_DIR ?= $(CURDIR)/target
T ?= lint typecheck test-unit security

.PHONY: lint typecheck test-unit test-integration security hooks image box run ci bundle

## lint: rustfmt check and clippy (warnings fail)
lint:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --locked -- -D warnings

## typecheck: cargo check
typecheck:
	cargo check --workspace --all-targets --locked

## test-unit: Rust tests, the app's JS tests, the bash version's tests and the guard hook's tests
test-unit:
	cargo test --workspace --locked
	node --test ui/*.test.js
	bash tests/test_exportpro.sh
	bash tests/test_guard.sh

## test-integration: none yet
test-integration:
	@echo "SKIP test-integration (none yet)"

## security: gitleaks, semgrep, trivy; high or worse fails
security:
	scripts/security.sh

## bundle: the Linux app packages (AppImage, .deb); in the image (make box T=bundle) they land in target/box/release/bundle
bundle:
	npm ci --no-audit --no-fund
	APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=true npx tauri build --bundles appimage,deb

## hooks: once per clone: gitleaks on each commit, lint/typecheck/unit before each push
hooks:
	git config core.hooksPath .githooks

## image: build the dev/CI image (skipped when this tag exists)
image:
	@[ -n "$(CONTAINER)" ] || { echo "needs podman or docker (or CONTAINER=...)" >&2; exit 1; }
	$(CONTAINER) image inspect $(IMAGE) >/dev/null 2>&1 || $(CONTAINER) build -t $(IMAGE) -f Containerfile . \
	  || { scripts/podman-diagnose.sh $(CONTAINER); exit 1; }

# Crates downloaded in the image are kept on the host between runs.
CARGO_REGISTRY ?= $(HOME)/.cargo/registry
BOX_RUN = mkdir -p "$(CARGO_REGISTRY)" "$(TARGET_DIR)" && $(CONTAINER) run --rm $(AS_ME) -v "$(CURDIR):/src:z" \
	  -v "$(CARGO_REGISTRY):/usr/local/cargo/registry:z" -v "$(TARGET_DIR):/target:z" \
	  -e CI -e CARGO_TARGET_DIR=/target/box $(IMAGE)

## box: run `make $(T)` in the image, with the checkout mounted
box: image
	$(BOX_RUN) make $(T)

## run: run any command in the image, e.g. make run C="cargo test -p exportpro-core"
run: image
	$(BOX_RUN) $(C)

## ci: what pr.yml runs
ci: box
