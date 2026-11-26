#!/usr/bin/env bash
# Verify TDD commit discipline on the current branch.
# Rule: only "test(red):" and "refactor(test):" commits may touch test files,
# and a branch with feat/fix commits needs at least one "test(red):" commit.
# ci/build/chore commits outside tdd.exempt paths (plumbing: no tests needed) get a warning.
# Usage: verify-tdd.sh [base] [test-path-regex]   (regex defaults to tests.regex in .workflow.yml)
set -uo pipefail

# Default base: origin/<project.default_branch> from .workflow.yml, else origin/main.
trunk=$(awk '/^[^[:space:]#]/ { inb = ($0 ~ /^project:/); next }
  inb && $1 == "default_branch:" { v = $2; sub(/#.*/, "", v); print v; exit }' \
  "$(git rev-parse --show-toplevel 2>/dev/null)/.workflow.yml" 2>/dev/null || true)
base=${1:-origin/${trunk:-main}}
cfg=$(git rev-parse --show-toplevel 2>/dev/null)/.workflow.yml

# .workflow.yml with CRs stripped (CRLF files).
cfgtxt() { [ -f "$cfg" ] && tr -d '\r' <"$cfg"; }
# Human-only TDD switch in .workflow.yml (tdd.enabled: false turns the check off).
tdd_val() {
  cfgtxt | awk -v k="$1" '/^tdd:/{f=1;next} f&&/^[^[:space:]#]/{f=0} f&&$1==k":"{sub(/^[^:]*:[[:space:]]*/,"");sub(/[[:space:]]+#.*$/,"");gsub(/^["'"'"']|["'"'"']$/,"");print;exit}'
}
# Raw value (quotes kept) of a key inside the top-level block $1.
blk_raw() {
  cfgtxt | awk -v b="$1" -v k="$2" 'index($0,b":")==1{f=1;next} f&&/^[^[:space:]#]/{f=0} f&&$1==k":"{sub(/^[^:]*:[[:space:]]*/,"");sub(/[[:space:]]+#.*$/,"");print;exit}'
}
if [ -n "${2:-}" ]; then
  re=$2
else
  raw=$(blk_raw tests regex)
  case "$raw" in
    '"'*|'>'*|'|'*)
      echo "FAIL  tests.regex in .workflow.yml must be a single-quoted string ('...'); double-quoted and block scalars are misread. Ask the human to fix it."
      exit 1 ;;
  esac
  raw=${raw#\'}; raw=${raw%\'}
  re=${raw:-${TDD_TEST_REGEX:-'(^|/)(tests?|__tests__|e2e|spec)/|\.(test|spec)\.[[:alnum:]]+$|_test\.[[:alnum:]]+$'}}
fi
grep -E -- "$re" </dev/null >/dev/null 2>&1
if [ $? -eq 2 ]; then
  echo "FAIL  test-file regex is invalid: $re"
  exit 1
fi
# Paths that need no tests (tdd.exempt); empty turns the ci/build/chore check off.
ex=$(blk_raw tdd exempt)
case "$ex" in
  '"'*|'>'*|'|'*)
    echo "FAIL  tdd.exempt in .workflow.yml must be a single-quoted string ('...'). Ask the human to fix it."
    exit 1 ;;
esac
ex=${ex#\'}; ex=${ex%\'}
if [ -n "$ex" ]; then
  grep -E -- "$ex" </dev/null >/dev/null 2>&1
  [ $? -eq 2 ] && { echo "FAIL  tdd.exempt regex is invalid: $ex"; exit 1; }
fi
# Lines of stdin not matching tdd.exempt.
not_exempt() { grep -Ev -- "$ex"; }

if [ "$(tdd_val enabled)" = "false" ]; then
  echo "SKIP  TDD is off (tdd.enabled: false in .workflow.yml)"
  exit 0
fi

fail=0; red=0; code=0
while IFS= read -r sha; do
  subject=$(git log -1 --format=%s "$sha")
  files=$(git diff-tree --no-commit-id --name-only -r "$sha")
  tests=$(grep -E -- "$re" <<<"$files"); rc=$?
  [ "$rc" -le 1 ] || { echo "FAIL  regex error (grep exit $rc)"; exit 1; }
  others=$(grep -Ev -- "$re" <<<"$files" | grep -Ev '^$'); rc=${PIPESTATUS[0]}
  [ "$rc" -le 1 ] || { echo "FAIL  regex error (grep exit $rc)"; exit 1; }
  case "$subject" in
    "test(red):"*)
      red=$((red+1))
      [ -n "$others" ] && echo "WARN  $sha '$subject' also changes non-test files: $(tr '\n' ' ' <<<"$others")"
      ;;
    "refactor(test):"*)
      [ -n "$others" ] && { echo "FAIL  $sha '$subject' changes non-test files"; fail=1; }
      ;;
    ci:*|ci\(*|build:*|build\(*|chore:*|chore\(*)
      # Test files in exempt paths are plumbing tests; any other is still red/refactor(test) only.
      ptests=$tests; [ -n "$ex" ] && ptests=$(not_exempt <<<"$tests")
      [ -n "$ptests" ] && { echo "FAIL  $sha '$subject' changes test files: $(tr '\n' ' ' <<<"$ptests")"; fail=1; }
      [ -n "$ex" ] || continue
      prod=$(not_exempt <<<"$others")
      [ -n "$prod" ] && echo "WARN  $sha '$subject' changes paths outside tdd.exempt: $(tr '\n' ' ' <<<"$prod")(plumbing? suggest adding them to tdd.exempt)"
      ;;
    *)
      if [[ "$subject" =~ ^(feat|fix)(\(.+\))?!?: ]]; then
        code=1
        [ -n "$ex" ] && [ -n "$others" ] && [ -z "$tests" ] && [ -z "$(not_exempt <<<"$others")" ] &&
          echo "WARN  $sha '$subject' changes only tdd.exempt paths: commit it as ci/build/chore"
      fi
      [ -n "$tests" ] && { echo "FAIL  $sha '$subject' changes test files: $(tr '\n' ' ' <<<"$tests")"; fail=1; }
      ;;
  esac
done < <(git rev-list --reverse "$base"..HEAD)

if [ $code -eq 1 ] && [ $red -eq 0 ]; then
  echo "FAIL  feat/fix commits but no 'test(red):' commit on this branch"; fail=1
fi
[ $fail -eq 0 ] && echo "OK    TDD commit discipline ($red red commit(s))"
exit $fail
