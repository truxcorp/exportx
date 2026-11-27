# 0007. Releases on semver tags; updates from GitHub Releases with Tauri's updater
Date: 2026-11-27 · Status: Accepted
## Context
The app needs a release process (one version, one commit, built for Linux and macOS) and
a way for installed copies to update. The repo is public (truxcorp/exportx); builds are
not code-signed for the OS yet, but updates must be verifiable.
## Options
- A: Tauri's updater plugin; a `v*.*.*` tag builds the Linux and Mac bundles, signs the
  update files with an update key (minisign, not an OS certificate) and publishes a GitHub
  Release with `latest.json`. One-click update; needs the key as a secret.
- B: a "new version" notice only, linking to the release page. No key; manual install.
- C: OS stores (Flathub, an apt repo, Homebrew cask, winget). Native, but a channel each
  to maintain, and Flatpak's sandbox fights the card and Trash access.
## Decision
A. The tag sets the version (stamped into Cargo.toml at build time, no bump commits);
`make release V=x.y.z` checks and pushes the tag. The signing key lives in the `release`
environment, usable from `v*` tags only. `latest.json` lists the AppImage
(`linux-x86_64-appimage`) and the Mac app (`darwin-aarch64-app`): the updater can't
replace a `.deb`, whose users get a notice instead (slice 5).
## Consequences
Losing the update key means installed copies can't update any more: the human keeps a
backup. Mac updates of an unsigned app need checking on a Mac. Windows and Intel Macs
come later (new `latest.json` entries).
