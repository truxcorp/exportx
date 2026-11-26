#!/usr/bin/env bash
# Migration check, run by `make lint` (install as .github/scripts/check-migrations.sh).
# Usage: check-migrations.sh <migrations-dir>     base: origin/$BASE_REF (default: project.default_branch, else main)
# Refuses edits to migrations the base branch already has: they may have run on STAGING
# or PROD, so add a new migration instead. Warns on DROP, RENAME and type changes in new
# ones: contract steps ship in a later release than their expand step.
set -euo pipefail
dir=${1:?usage: $0 <migrations-dir>}
trunk=$(awk '/^[^[:space:]#]/ { inb = ($0 ~ /^project:/); next }
  inb && $1 == "default_branch:" { v = $2; sub(/#.*/, "", v); print v; exit }' .workflow.yml 2>/dev/null || true)
base=origin/${BASE_REF:-${trunk:-main}}
if ! git rev-parse -q --verify "$base^{commit}" >/dev/null; then
  echo "SKIP migrations-check: $base not fetched"
  exit 0
fi
bad=0
while IFS= read -r f; do
  if [ -f "$f" ] && ! git diff --quiet "$base" -- "$f"; then
    echo "REFUSED  $f exists on $base and was edited: add a new migration instead"
    bad=1
  fi
done < <(git ls-tree -r --name-only "$base" -- "$dir")
while IFS= read -r f; do
  [ -f "$f" ] || continue
  if grep -Eiq '(^|[^[:alnum:]_])(drop|rename)([^[:alnum:]_]|$)|alter[[:space:]]+column[^;]*[[:space:]]type([^[:alnum:]_]|$)' "$f"; then
    echo "WARNING  $f: DROP, RENAME or type change; ship it in a later release than its expand step"
  fi
done < <({ git diff --name-only --diff-filter=A "$base" -- "$dir"; git ls-files --others --exclude-standard -- "$dir"; } | sort -u)
exit "$bad"
