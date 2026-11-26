#!/usr/bin/env bash
# make security: gitleaks (whole history), semgrep (p/default, ERROR fails) and trivy
# (dependencies, secrets, misconfigurations; HIGH and CRITICAL fail). The tools come from
# the Containerfile image: run `make box T=security` on a machine without them.
# semgrep and trivy read a copy of the tracked files only, so gitignored local files
# (.env, build output) never reach a scanner or its output.
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
for tool in gitleaks semgrep trivy; do
  command -v "$tool" >/dev/null || { echo "make security: $tool missing; run: make box T=security" >&2; exit 1; }
done

tree="$(mktemp -d)"
trap 'rm -r -- "$tree"' EXIT
git -C "$root" ls-files -z --cached | tar -C "$root" --null -T - --ignore-failed-read -cf - 2>/dev/null | tar -C "$tree" -xf -

echo "== gitleaks (history)"
gitleaks git --no-banner --redact "$root"

echo "== semgrep (p/default, ERROR)"
(cd "$tree" && semgrep scan --config p/default --error --severity ERROR --quiet --metrics=off)

echo "== trivy (HIGH, CRITICAL)"
trivy fs --scanners vuln,secret,misconfig --severity HIGH,CRITICAL --exit-code 1 --quiet "$tree"
