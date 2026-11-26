#!/usr/bin/env bash
# Replace "uses: owner/repo@PIN # vN" with the commit SHA of vN.
# Usage: pin-actions.sh .github/workflows/*.yml     Requires: gh (authenticated).
set -euo pipefail
[ $# -gt 0 ] || { echo "usage: $0 <workflow.yml>..." >&2; exit 1; }

for file in "$@"; do
  while IFS= read -r line; do
    action=$(sed -E 's/.*uses:[[:space:]]*([^@[:space:]]+)@PIN.*/\1/' <<<"$line")
    version=$(sed -E 's/.*#[[:space:]]*([^[:space:]]+).*/\1/' <<<"$line")
    repo=$(cut -d/ -f1-2 <<<"$action")   # owner/repo (drops sub-path actions)
    sha=$(gh api "repos/$repo/commits/$version" --jq .sha)
    [[ "$sha" =~ ^[0-9a-f]{40}$ ]] || { echo "Could not resolve $repo@$version" >&2; exit 1; }
    sed -i.bak -E "s|(uses:[[:space:]]*$action)@PIN([[:space:]]*#[[:space:]]*$version)|\1@$sha\2|" "$file"
    rm -f "$file.bak"
    echo "$file: $action@$version -> $sha"
  done < <(grep -E 'uses:[[:space:]]*[^@[:space:]]+@PIN[[:space:]]*#' "$file" | sort -u)
done
