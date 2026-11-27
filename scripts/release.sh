#!/usr/bin/env bash
# Releases (ADR 0007). The tag v<version> is the one source of the version.
#   release.sh tag <version>            on an up-to-date, clean main: create and push the tag
#   release.sh check <version>          the tag's commit is on origin/main (CI, first)
#   release.sh build-linux <version>    signed AppImage (+ .sig) and .deb, in dist/
#   release.sh build-macos <version>    signed .app.tar.gz (+ .sig) and .dmg, in dist/
#   release.sh manifest <version> <dir> latest.json for the updater, from the files in <dir>
#   release.sh publish <version> <dir>  manifest, then the GitHub Release with every file
# Builds need TAURI_SIGNING_PRIVATE_KEY (and _PASSWORD) in the environment.
set -euo pipefail
cmd=${1:-}; version=${2:-}
repo=${RELEASE_REPO:-truxcorp/exportx}
root="$(git rev-parse --show-toplevel)"
cd "$root"

die() { echo "release: $*" >&2; exit 1; }
semver='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$'
[[ "$version" =~ $semver ]] || die "usage: release.sh <command> <version>, version like 1.2.3 (got '$version')"
tag="v$version"

# The workspace version (Cargo.toml [workspace.package]), which the app and the CLI take.
stamp() {
  awk -v v="$version" '
    /^\[/ { section = $0 }
    section == "[workspace.package]" && /^version = / { print "version = \"" v "\""; next }
    { print }' Cargo.toml >Cargo.toml.new && mv Cargo.toml.new Cargo.toml
  grep -q "^version = \"$version\"" Cargo.toml || die "could not stamp $version into Cargo.toml"
}

# Copy one built file into dist/, under `name` (default: its own), failing if it's missing.
take() {
  [ -f "$1" ] || die "missing build output: $1"
  cp "$1" "dist/${2:-$(basename "$1")}"
}

need_key() {
  [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ] || die "TAURI_SIGNING_PRIVATE_KEY is not set (the update files must be signed)"
  # The app checks updates with the public half: without it, a release could never update.
  jq -e '.plugins.updater.pubkey | test("^[A-Za-z0-9+/=]{40,}$")' crates/app/tauri.conf.json >/dev/null \
    || die "crates/app/tauri.conf.json has no updater public key (plugins.updater.pubkey)"
}

case "$cmd" in
  tag)
    git fetch --quiet origin main --tags
    [ "$(git rev-parse --abbrev-ref HEAD)" = main ] || die "switch to main first"
    [ -z "$(git status --porcelain)" ] || die "the working tree isn't clean"
    [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || die "main isn't origin/main: pull first"
    ! git rev-parse -q --verify "refs/tags/$tag" >/dev/null || die "$tag already exists"
    git tag -a "$tag" -m "ExportX $version"
    git push origin "$tag"
    echo "release: pushed $tag; the release workflow builds and publishes it"
    ;;
  check)
    git fetch --quiet origin main
    git merge-base --is-ancestor HEAD origin/main || die "$tag isn't on main"
    ;;
  build-linux)
    need_key; stamp
    npm ci --no-audit --no-fund
    APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=true npx tauri build --ci --bundles appimage,deb \
      --config crates/app/tauri.release.conf.json
    out="${CARGO_TARGET_DIR:-target}/release/bundle"
    rm -rf dist && mkdir dist
    take "$out/appimage/ExportX_${version}_amd64.AppImage"
    take "$out/appimage/ExportX_${version}_amd64.AppImage.sig"
    take "$out/deb/ExportX_${version}_amd64.deb"
    ls -l dist
    ;;
  build-macos)
    need_key; stamp
    npm ci --no-audit --no-fund
    npx tauri build --ci --bundles app,dmg --config crates/app/tauri.release.conf.json
    out="${CARGO_TARGET_DIR:-target}/release/bundle"
    rm -rf dist && mkdir dist
    take "$out/dmg/ExportX_${version}_aarch64.dmg"
    # The update archive has no version in its name: given one, so releases never clash.
    take "$out/macos/ExportX.app.tar.gz" "ExportX_${version}_aarch64.app.tar.gz"
    take "$out/macos/ExportX.app.tar.gz.sig" "ExportX_${version}_aarch64.app.tar.gz.sig"
    ls -l dist
    ;;
  manifest|publish)
    dir=${3:-}; [ -d "$dir" ] || die "usage: release.sh $cmd <version> <dir with the built files>"
    url="https://github.com/$repo/releases/download/$tag"
    entry() { # platform key, file name
      [ -f "$dir/$2" ] && [ -f "$dir/$2.sig" ] || die "missing $2 or its .sig in $dir"
      jq -n --arg k "$1" --arg s "$(cat "$dir/$2.sig")" --arg u "$url/$2" '{($k): {signature: $s, url: $u}}'
    }
    {
      entry linux-x86_64-appimage "ExportX_${version}_amd64.AppImage"
      entry darwin-aarch64-app "ExportX_${version}_aarch64.app.tar.gz"
    } | jq -s --arg v "$version" --arg d "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
      --arg n "See https://github.com/$repo/releases/tag/$tag" \
      '{version: $v, notes: $n, pub_date: $d, platforms: add}' >"$dir/latest.json"
    echo "release: wrote $dir/latest.json"
    if [ "$cmd" = publish ]; then
      prerelease=(); [[ "$version" == *-* ]] && prerelease=(--prerelease)
      gh release create "$tag" --repo "$repo" --verify-tag --title "ExportX $version" \
        --generate-notes "${prerelease[@]}" "$dir"/*
    fi
    ;;
  *) die "unknown command '$cmd' (tag, check, build-linux, build-macos, manifest, publish)" ;;
esac
