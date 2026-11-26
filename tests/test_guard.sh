#!/usr/bin/env bash
# Tests for the project's exceptions in .claude/hooks/guard.sh:
# - `git -C <dir> push` is judged on <dir>'s repo (it used to skip every push rule);
# - repos whose push URL is listed in project.direct_push_remotes (merged .workflow.yml
#   only, never the session's own remote) may be pushed to main with `git -C <dir> push`;
# - anything the guard can't resolve exactly (cd away, several -C, -c, detached HEAD,
#   URL rewrites) gets no exception.
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
mkrepo "$tmp/vault-case" main https://github.com/simontrux/obsione.git
mkrepo "$tmp/vault-port" main ssh://git@github.com:22/SimonTrux/ObsiOne.git
mkrepo "$tmp/vault-pushurl" main https://github.com/SimonTrux/ObsiOne.git
git -C "$tmp/vault-pushurl" remote set-url --push origin https://github.com/truxcorp/exportPro.git
mkrepo "$tmp/vault-pushdefault" main https://github.com/SimonTrux/ObsiOne.git
git -C "$tmp/vault-pushdefault" remote add up https://github.com/truxcorp/exportPro.git
git -C "$tmp/vault-pushdefault" config remote.pushDefault up
mkrepo "$tmp/vault-insteadof" main https://github.com/SimonTrux/ObsiOne.git
git -C "$tmp/vault-insteadof" config url.https://github.com/truxcorp/exportPro.git.pushInsteadOf https://github.com/SimonTrux/ObsiOne.git
mkrepo "$tmp/vault-detached" main https://github.com/SimonTrux/ObsiOne.git
git -C "$tmp/vault-detached" checkout -q --detach

# A session whose merged config (spoofed) lists its own remote.
own="$tmp/own"
mkrepo "$own" main https://github.com/truxcorp/exportPro.git
printf 'project:\n  name: demo\n  direct_push_remotes: [github.com/truxcorp/exportPro]\n' >"$own/.workflow.yml"
git -C "$own" add .workflow.yml
git -C "$own" -c user.name=t -c user.email=t@t commit -q -m cfg
git -C "$own" update-ref refs/remotes/origin/main HEAD

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
check "$session" "cd $tmp/vault && git pull --rebase && git push" block
check "$session" "git -C $tmp/vault push" allow
check "$session" "git -C $tmp/vault-ssh push origin main" allow
check "$session" "git -C $tmp/vault push --force" block
check "$session" "git -C $tmp/vault push origin --delete old" block
check "$session" "git -C $tmp/other push" block
check "$session" "cd $tmp/other && git push" block
check "$session" "cd $tmp/vault && cd $tmp/other && git push" block
check "$session" "git -C $tmp/feature push" allow
check "$local_only" "git -C $tmp/vault push" block
check "$session" "git -C $tmp/vault-case push" allow
check "$session" "git -C $tmp/vault-port push" allow
check "$session" "git -C $tmp/vault-pushurl push origin main" block
check "$session" "git -C $tmp/vault -c remote.origin.pushurl=https://github.com/truxcorp/exportPro.git push origin main" block
check "$session" "git -C $tmp/vault push --repo=https://github.com/truxcorp/exportPro.git" block
check "$session" "git -C $tmp/vault-pushdefault push" block
check "$session" "git -C $tmp/vault-insteadof push" block
check "$session" "git -C $tmp/vault-detached push" block
check "$session" "git -C $tmp/vault -C ../session push" block
check "$session" "cd /nonexistent || git push" block
check "$session" 'git -C "$PWD" push' block
check "$session" "false && cd $tmp/vault; git push" block
check "$session" "cd $tmp/vault | cat; git push" block
check "$own" "git push origin main" block
check "$own" "git -C $own push origin main" block

printf '%d ok, %d failed\n' "$ok_n" "$fail_n"
[ "$fail_n" -eq 0 ]
