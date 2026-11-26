#!/usr/bin/env bash
# Tests for the project's exceptions in .claude/hooks/guard.sh:
# - a push is judged on the repo it targets (cd <dir> / git -C <dir>), not the session's;
# - repos listed in project.direct_push_remotes (merged .workflow.yml only) may be pushed
#   to their main branch directly (e.g. the notes vault).
# Usage: bash tests/test_guard.sh
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
guard="$here/../.claude/hooks/guard.sh"

ok_n=0 fail_n=0
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

mkrepo() { # mkrepo <dir> <branch> [origin url]
  git init -q -b "$2" "$1"
  git -C "$1" -c user.name=t -c user.email=t@t commit -q --allow-empty -m init
  [ -n "${3:-}" ] && git -C "$1" remote add origin "$3"
  return 0
}

# Session repo on main; the merged config lists the vault.
session="$tmp/session"
mkrepo "$session" main https://github.com/truxcorp/exportPro.git
printf 'project:\n  name: demo\n  direct_push_remotes: [github.com/SimonTrux/ObsiOne]\n' >"$session/.workflow.yml"
git -C "$session" add .workflow.yml
git -C "$session" -c user.name=t -c user.email=t@t commit -q -m cfg
git -C "$session" update-ref refs/remotes/origin/main HEAD

mkrepo "$tmp/vault" main https://github.com/SimonTrux/ObsiOne.git
mkrepo "$tmp/vault-ssh" main git@github.com:SimonTrux/ObsiOne.git
mkrepo "$tmp/other" main https://github.com/x/other.git
mkrepo "$tmp/feature" feat/x https://github.com/x/feature.git

# A session whose vault entry exists only in the working tree, not on origin/main.
local_only="$tmp/local-only"
mkrepo "$local_only" main https://github.com/truxcorp/exportPro.git
printf 'project:\n  name: demo\n' >"$local_only/.workflow.yml"
git -C "$local_only" add .workflow.yml
git -C "$local_only" -c user.name=t -c user.email=t@t commit -q -m cfg
git -C "$local_only" update-ref refs/remotes/origin/main HEAD
printf 'project:\n  name: demo\n  direct_push_remotes: [github.com/SimonTrux/ObsiOne]\n' >"$local_only/.workflow.yml"

check() { # check <session dir> <command> <allow|block>
  local input rc got
  input=$(jq -n --arg c "$2" --arg d "$1" '{tool_input:{command:$c},cwd:$d}')
  bash "$guard" <<<"$input" >/dev/null 2>&1
  rc=$?
  got=allow
  [ $rc -eq 2 ] && got=block
  [ $rc -ne 0 ] && [ $rc -ne 2 ] && got="rc=$rc"
  if [ "$got" = "$3" ]; then
    ok_n=$((ok_n + 1)); printf 'ok   %s -> %s\n' "$2" "$3"
  else
    fail_n=$((fail_n + 1)); printf 'FAIL %s -> want %s, got %s\n' "$2" "$3" "$got"
  fi
}

check "$session" "git push" block
check "$session" "git push origin main" block
check "$session" "cd $tmp/vault && git pull --rebase && git push" allow
check "$session" "git -C $tmp/vault push" allow
check "$session" "git -C $tmp/vault-ssh push origin main" allow
check "$session" "git -C $tmp/vault push --force" block
check "$session" "git -C $tmp/vault push origin --delete old" block
check "$session" "git -C $tmp/other push" block
check "$session" "cd $tmp/other && git push" block
check "$session" "cd $tmp/vault && cd $tmp/other && git push" block
check "$session" "git -C $tmp/feature push" allow
check "$local_only" "git -C $tmp/vault push" block

printf '%d ok, %d failed\n' "$ok_n" "$fail_n"
[ "$fail_n" -eq 0 ]
