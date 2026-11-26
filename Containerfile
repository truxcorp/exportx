# ExportPro dev and CI toolbox (docs/plan/tauri-prototype.md): Rust, Node, Tauri's Linux
# libraries and the security scanners. CI runs every check in it (make ci); on a host
# without these tools (Fedora Silverblue), `make box T=<targets>` does the same, or use it
# as a toolbox/distrobox image. The base image is pinned by digest, rustup-init, gitleaks
# and trivy by version and checksum; semgrep (PyPI) and apt packages by version only.
FROM docker.io/library/ubuntu:26.04@sha256:0d5255be560e2d2ddcfbfda88549e9e815ef29e7cd7fc62183d5709e6e2963c6

ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential ca-certificates curl file git jq make pkg-config xz-utils \
      libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libxdo-dev \
      nodejs npm pipx \
    && rm -rf /var/lib/apt/lists/*

# Rust: the toolchain pinned in rust-toolchain.toml
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:$PATH
ARG RUSTUP_VERSION=1.29.3
ARG RUSTUP_SHA256=b1c8cb14e30889e7e2999e6b69e62d9e9b9ffc0f4df8180564d306d9680e6c55
COPY rust-toolchain.toml /tmp/rust-toolchain.toml
RUN channel=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' /tmp/rust-toolchain.toml) \
    && curl -sSfL -o /tmp/rustup-init \
      "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-gnu/rustup-init" \
    && echo "$RUSTUP_SHA256  /tmp/rustup-init" | sha256sum -c - \
    && chmod +x /tmp/rustup-init \
    && /tmp/rustup-init -y --no-modify-path --profile minimal \
         --default-toolchain "$channel" --component rustfmt,clippy \
    && chmod -R a+rwX "$RUSTUP_HOME" "$CARGO_HOME" && rm /tmp/rust-toolchain.toml /tmp/rustup-init

# Scanners for make security
ARG GITLEAKS_VERSION=8.30.1
ARG GITLEAKS_SHA256=551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb
ARG TRIVY_VERSION=0.76.2
ARG TRIVY_SHA256=bc0fc726e017b8927a62b2d3531d7a84ee01769daf5c99ed866dced54089fdbf
ARG SEMGREP_VERSION=1.187.0
RUN curl -sSfL -o /tmp/gitleaks.tgz \
      "https://github.com/gitleaks/gitleaks/releases/download/v${GITLEAKS_VERSION}/gitleaks_${GITLEAKS_VERSION}_linux_x64.tar.gz" \
    && echo "$GITLEAKS_SHA256  /tmp/gitleaks.tgz" | sha256sum -c - \
    && tar -xzf /tmp/gitleaks.tgz -C /usr/local/bin gitleaks \
    && curl -sSfL -o /tmp/trivy.tgz \
      "https://github.com/aquasecurity/trivy/releases/download/v${TRIVY_VERSION}/trivy_${TRIVY_VERSION}_Linux-64bit.tar.gz" \
    && echo "$TRIVY_SHA256  /tmp/trivy.tgz" | sha256sum -c - \
    && tar -xzf /tmp/trivy.tgz -C /usr/local/bin trivy \
    && rm /tmp/gitleaks.tgz /tmp/trivy.tgz \
    && PIPX_HOME=/opt/pipx PIPX_BIN_DIR=/usr/local/bin pipx install "semgrep==${SEMGREP_VERSION}"

# The checkout is mounted here by make box; git trusts it whatever its owner. Jobs run as
# a non-root user (make box maps it to the caller's uid, so files it writes are theirs).
RUN git config --system safe.directory '*'
USER 1000:1000
ENV HOME=/tmp
WORKDIR /src
