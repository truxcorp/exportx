#!/usr/bin/env bash
# Workflow guard: Claude Code PreToolUse hook for Bash.
# Deliberately light: it only covers what permission rules in settings.json can't express,
# and it blocks rather than asks, so unattended sessions never wait on a prompt.
# Own checks cover git; other modules add checks as guard.d/*.sh (each run with bash).
# guard.d contract: called as `check "<segment>"` with env GUARD_BRANCH, GUARD_CWD, GUARD_CMD.
#   exit 2 + stderr message = block; exit 0 + one stdout line = ask reason; exit 0 silent = allow.
# Exit 2 = block (stderr goes to Claude). JSON "ask" = make the human confirm.
set -uo pipefail

block() {
  echo "BLOCKED by workflow guard: $1. See .workflow/rules.md. Do not work around this; skip this step and carry on, or tell the human." >&2
  exit 2
}
ask() {
  jq -n --arg r "$1" '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"ask",permissionDecisionReason:$r}}'
  exit 0
}

command -v jq >/dev/null 2>&1 || block "jq is not installed (required by .claude/hooks/guard.sh)"

input=$(cat)
cmd=$(jq -r '.tool_input.command // ""' <<<"$input")
cwd=$(jq -r '.cwd // ""' <<<"$input")
[ -n "$cmd" ] || exit 0

branch=$(git -C "${cwd:-.}" branch --show-current 2>/dev/null || true)

# Protected branches: main and master always, the trunk (project.default_branch) and
# project.protected_branches (one-line list: [a, b]). Exact names. Both the working-tree
# .workflow.yml and the copy merged on the remote's default branch count, so editing the
# file locally can add protection but never remove it: removal needs a merged PR.
top=$(git -C "${cwd:-.}" rev-parse --show-toplevel 2>/dev/null || true)
cfg_local=""
[ -n "$top" ] && [ -f "$top/.workflow.yml" ] && cfg_local=$(cat "$top/.workflow.yml")
cfg_merged=""
for ref in origin/HEAD origin/main; do
  cfg_merged=$(git -C "${top:-.}" show "$ref:.workflow.yml" 2>/dev/null) && break
  cfg_merged=""
done
project_val() { # project_val <key> <yaml text>: rest of the line under the top-level project: block
  awk -v k="$1" '/^[^[:space:]#]/ { inb = ($0 ~ /^project:/); next }
    inb && $1 == k ":" { sub(/^[^:]*:[[:space:]]*/, ""); sub(/[[:space:]]*#.*/, ""); print; exit }' <<<"$2"
}
protected=" main master "
for c in "$cfg_local" "$cfg_merged"; do
  [ -n "$c" ] || continue
  protected+=" $(project_val default_branch "$c") $(project_val protected_branches "$c" | tr -d '[]"'"'"'' | tr ',' ' ') "
done
protected=$(tr -s ' ' <<<"$protected")
# Project exception: repos whose remote is listed in project.direct_push_remotes (e.g. a
# notes vault) may be pushed to their protected branch directly. Read from the merged
# config only, so a local edit can't add an exception.
direct_ok=" $(project_val direct_push_remotes "$cfg_merged" | tr -d '[]"'"'"'' | tr ',' ' ') "
norm_remote() { # https://host/o/r.git, git@host:o/r.git -> host/o/r
  sed -E 's#^[a-z+]+://##; s#^[^@/]+@##; s#^([^/:]+):#\1/#; s#\.git/?$##; s#/$##' <<<"$1"
}
resolve_dir() { # resolve_dir <base> <path>: absolute path, ~ expanded, quotes removed
  local d=$2
  d=${d#[\"\']}; d=${d%[\"\']}
  # shellcheck disable=SC2088 # literal ~ in the command text, expanded by hand
  case "$d" in "~") d=$HOME ;; "~/"*) d="$HOME/${d#\~/}" ;; /*) ;; *) d="$1/$d" ;; esac
  printf '%s' "$d"
}
cur_dir=${cwd:-$PWD}
is_protected() { [[ "$protected" == *" $1 "* ]]; }
export GUARD_BRANCH="$branch" GUARD_CWD="$cwd" GUARD_CMD="$cmd"
need_ask=""
add_ask() {
  case " $need_ask " in *" $1 "*) return ;; esac
  need_ask="${need_ask:+$need_ask }$1"
}
checks_dir="$(dirname "$0")/guard.d"

# Check each simple command separately, so text in other parts (commit messages,
# 'tail -f', 'gh pr create --base main') doesn't trigger false blocks.
while IFS= read -r seg; do
  seg=$(sed -E 's/^[[:space:]]+//' <<<"$seg")
  [ -n "$seg" ] || continue

  # 0. cd changes the repo later segments act on (cd <vault> && git push).
  if [[ "$seg" =~ ^cd([[:space:]]+([^[:space:]]+))?[[:space:]]*$ ]]; then
    cur_dir=$(resolve_dir "$cur_dir" "${BASH_REMATCH[2]:-~}")
    branch=$(git -C "$cur_dir" branch --show-current 2>/dev/null || true)
    continue
  fi

  # 1. git push rules, judged on the repo the push targets (cd, git -C).
  if [[ "$seg" =~ ^(sudo[[:space:]]+)?git(([[:space:]]+-[Cc][[:space:]]+[^[:space:]]+|[[:space:]]+-[^[:space:]]+)*)[[:space:]]+push([[:space:]].*)?$ ]]; then
    gopts=${BASH_REMATCH[2]}
    args=" ${BASH_REMATCH[4]} "
    tdir=$cur_dir tbranch=$branch
    if [[ "$gopts" =~ -C[[:space:]]+([^[:space:]]+) ]]; then
      tdir=$(resolve_dir "$cur_dir" "${BASH_REMATCH[1]}")
      tbranch=$(git -C "$tdir" branch --show-current 2>/dev/null || true)
    fi
    is_tag=0
    grep -Eq '[[:space:]](--tags|refs/tags/[^[:space:]]+|v[0-9]+\.[0-9]+\.[0-9]+)[[:space:]]' <<<"$args" && is_tag=1
    # 1a. Force push: only --force-with-lease, never to a protected branch or tags.
    if grep -Eq '[[:space:]](--force|-f|-[a-zA-Z]*f[a-zA-Z]*)[[:space:]]|[[:space:]:]\+[[:alnum:]]' <<<"$args"; then
      block "force-push: only --force-with-lease on your own feature branch"
    fi
    if grep -Eq '[[:space:]]--force-with-lease' <<<"$args" && [ $is_tag -eq 1 ]; then
      block "tags are never force-pushed (force-push)"
    fi
    # 1b. The refs the push targets, not the branch checked out: explicit refspecs
    #     (origin main, HEAD:main, refs/heads/main), else the current branch. Tag pushes
    #     are allowed (tag creation is gated by the 'git tag' ask rule).
    if grep -Eq '[[:space:]](--all|--mirror|--branches)[[:space:]]' <<<"$args"; then
      block "no push to protected branches: this pushes every branch, protected ones included"
    fi
    read -ra pw <<<"$args"
    pos=() skip=0
    for w in "${pw[@]}"; do
      if [ $skip -eq 1 ]; then skip=0; continue; fi
      case "$w" in
        -o|--push-option|--repo|--receive-pack|--exec) skip=1 ;;
        -*) ;;
        *) pos+=("$w") ;;
      esac
    done
    if [ ${#pos[@]} -gt 1 ]; then targets=("${pos[@]:1}")
    elif [ $is_tag -eq 0 ]; then targets=(HEAD)
    else targets=()
    fi
    remote=${pos[0]:-origin}
    url=$(git -C "$tdir" remote get-url "$remote" 2>/dev/null || printf '%s' "$remote")
    [[ "$direct_ok" == *" $(norm_remote "$url") "* ]] && targets=()
    for t in "${targets[@]}"; do
      t=${t#+}
      [[ "$t" == *:* ]] && t=${t#*:}
      [ "$t" = HEAD ] && t=$tbranch
      t=${t#refs/heads/}
      if is_protected "$t"; then
        if [ ${#pos[@]} -gt 1 ]; then block "no push to protected branches: $t is protected; open a PR"; fi
        block "no push to protected branches: you are on $t; create a branch first"
      fi
    done
    # 1c. Deleting remote refs is for humans (merged PR branches go with gh pr merge --delete-branch).
    if grep -Eq '[[:space:]](--delete|-d)[[:space:]]|[[:space:]]:[^[:space:]]+' <<<"$args"; then
      block "no remote deletes: deleting a remote branch or tag is for the human"
    fi
  fi

  # 1d. Branch switches earlier in the same command change what a later implicit push
  #     targets (git switch -c x && git push).
  if new=$(sed -nE 's/^git([[:space:]]+-[^[:space:]]+)*[[:space:]]+(switch([[:space:]]+(-c|-C|--create|--force-create))?|checkout[[:space:]]+(-b|-B))[[:space:]]+([^-][^[:space:]]*).*/\6/p' <<<"$seg") \
     && [ -n "$new" ]; then
    branch=$new
  fi

  # 2. Commit statuses and check runs are written by CI only, whatever the tool.
  #    Reads are fine: only a write method or request fields (gh -f/-F, curl -d) block.
  if grep -Eq '/statuses(/|[[:space:]"'"'"']|$)|/check-runs' <<<"$seg" \
     && ! grep -Eiq '(^|[[:space:]])(-X|--method|--request)[[:space:]=]*GET([[:space:]]|$)' <<<"$seg" \
     && grep -Eiq '(^|[[:space:]])((-X|--method|--request)[[:space:]=]*(POST|PUT|PATCH|DELETE)|-[fFd]|--(field|raw-field|input|data[a-z-]*|form))([[:space:]=]|$)' <<<"$seg"; then
    block "statuses by CI only: commit statuses and checks are set by CI"
  fi

  # Per-module checks (run with bash whatever the exec bit; fail closed on anything unexpected).
  if [ -d "$checks_dir" ]; then
    for check in "$checks_dir"/*.sh; do
      [ -f "$check" ] || continue
      # stdout of the check flows through fd 3 into $raw; its stderr is captured in $err.
      raw=$( { err=$(bash "$check" "$seg" 2>&1 1>&3 3>&-); rc=$?; printf '\n\036%s\n%s' "$rc" "$err"; } 3>&1 )
      out=${raw%%$'\n\036'*}
      rest=${raw#*$'\n\036'}
      rc=${rest%%$'\n'*}
      err=""
      [[ "$rest" == *$'\n'* ]] && err=${rest#*$'\n'}
      case "$rc" in
        0) [ -n "$out" ] && add_ask "$(head -n 1 <<<"$out")" ;;
        2)
          if [ -n "$err" ]; then
            echo "$err" >&2
            exit 2
          fi
          block "guard check $(basename "$check") blocked without a message"
          ;;
        *) block "guard check $(basename "$check") failed (exit $rc)" ;;
      esac
    done
  fi
done < <(sed -E 's/(\|\||&&|;|\|&|\||&)/\n/g' <<<"$cmd")

[ -n "$need_ask" ] && ask "$need_ask"
exit 0
