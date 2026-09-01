#!/usr/bin/env bash
#
# Discrimination matrix for the shared-checkout git hooks.
#
# WHY THIS EXISTS
# ---------------
# `scripts/pre-commit-foreign-index.sh` and `scripts/post-index-change-stage-log.sh`
# guard against one session's commit capturing another's staged work. The guard failed
# in production within two hours of being installed
# (docs/issues/archive/2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md),
# and nothing in the repo would have caught a recurrence: there is no shellcheck, no CI
# step for `scripts/`, and the sibling hook `pre-commit-unreviewed-content.sh` has no
# test at all. Its precedent was a hand-run matrix pasted into a commit message, which
# is evidence about one instant and cannot fail a later build.
#
# Every case below is a DISCRIMINATION: each asserts the hook is silent where it must be
# silent AND loud where it must be loud. A suite checking only the loud direction passes
# against a hook that refuses everything; one checking only silence passes against a hook
# that was deleted.
#
# Usage:
#   tests/hooks-discrimination.sh          # all suites; non-zero exit on any failure
#
# Runs entirely inside throwaway repos under $TMPDIR. It never touches this checkout,
# which matters because several sessions share it and a stray `git reset` here would
# destroy their uncommitted work.

set -uo pipefail

SRC="$(cd "$(dirname "$0")/../scripts" && pwd)"
REAL_REPO="$(cd "$(dirname "$0")/.." && pwd)"
PASS=0
FAIL=0

ok() { echo "  PASS  $1"; PASS=$((PASS + 1)); }
no() {
    echo "  FAIL  $1"
    [ -n "${2:-}" ] && echo "        $2"
    FAIL=$((FAIL + 1))
}
eq() { [ "$2" = "$3" ] && ok "$1" || no "$1" "want '$3' got '$2'"; }
has() { printf '%s' "$2" | grep -qF "$3" && ok "$1" || no "$1" "missing: $3"; }

# REFUSE TO RUN ANYWHERE BUT A THROWAWAY. Defence in depth, and not theoretical.
#
# This suite runs `git add -A` and `git commit`. On 2026-09-01 its first version defined
# `new_repo() { cd "$(mktemp -d)"; ...; }` and called it as `T="$(new_repo)"` — command
# substitution runs a SUBSHELL, so the `cd` never reached the parent and every one of
# those commands executed against the real shared checkout. A junk commit landed on
# `experiments`, and a `git reset --hard` in the same suite ran in a tree four other
# sessions were working in.
#
# The `cd` bug is fixed below. This check exists because fixing a bug is not the same as
# making its class impossible: any future edit that reintroduces a subshell gets a
# refusal instead of a live repo. `git reset --hard` is also gone from the suite outright
# — a plain `git reset` empties the index without touching the working tree, which is all
# any case here ever needed, so the destructive form has no reason to appear in a test.
assert_throwaway() {
    _at="$(pwd -P)"
    case "$_at" in
        "$REAL_REPO" | "$REAL_REPO"/*)
            echo "REFUSING: this suite is running inside the real checkout:" >&2
            echo "    $_at" >&2
            echo "It runs destructive git commands and must only run in a throwaway." >&2
            exit 1
            ;;
    esac
    [ -d "$_at/.git" ] || {
        echo "REFUSING: no .git at $_at — not running git commands here." >&2
        exit 1
    }
}

# Sets $T and cds into it. Call as `new_repo`, NEVER as `T="$(new_repo)"` — that is the
# subshell that caused the incident described above.
new_repo() {
    T="$(mktemp -d)"
    cd "$T" || exit 1
    git init -q .
    assert_throwaway
    git config user.email t@t
    git config user.name t
    mkdir -p .git/hooks
    cat > .git/hooks/post-index-change <<SHIM
#!/usr/bin/env bash
exec "$SRC/post-index-change-stage-log.sh" "\$@"
SHIM
    chmod +x .git/hooks/post-index-change
}

# The guard as a bare (index) commit sees it. GIT_INDEX_FILE must be UNSET rather than
# empty: an empty value makes git read a nonexistent index and report every dst blob as
# zeros, so nothing matches the log and the guard reads everything as ours — silence for
# entirely the wrong reason.
guard() {
    env -u GIT_INDEX_FILE CLAUDE_CODE_SESSION_ID="$1" \
        bash "$SRC/pre-commit-foreign-index.sh" 2>&1
    echo "EXIT=$?"
}
owner_of() { awk -F'\t' -v p="$1" '$3 == p { print $1; exit }' .git/session-stage-log; }

A="aaaaaaaa-0000-0000-0000-aaaaaaaaaaaa"
B="bbbbbbbb-1111-1111-1111-bbbbbbbbbbbb"

# ---------------------------------------------------------------- 1. the index guard
echo "== index guard"
new_repo
echo base > a.txt
echo base > b.txt
git add -A > /dev/null 2>&1
git commit -qm base

echo mine > a.txt
CLAUDE_CODE_SESSION_ID="$A" git add a.txt
eq "records the staging session" "$(owner_of a.txt)" "$A"
has "own paths only -> silent" "$(guard "$A")" "EXIT=0"

echo theirs > b.txt
CLAUDE_CODE_SESSION_ID="$B" git add b.txt
out="$(guard "$A")"
has "foreign path -> refuse" "$out" "EXIT=1"
has "names the foreign path" "$out" "b.txt"
has "offers the pathspec remedy for mine" "$out" "git commit -- a.txt"

# A pathspec commit gets a temp index named next-index-<pid>.lock and ignores the shared
# index entirely, so it cannot capture and must not be refused.
has "pathspec commit -> silent" \
    "$(CLAUDE_CODE_SESSION_ID="$A" GIT_INDEX_FILE=".git/next-index-1.lock" \
        bash "$SRC/pre-commit-foreign-index.sh" 2>&1; echo "EXIT=$?")" "EXIT=0"

git reset -q
has "nothing staged -> silent" "$(guard "$A")" "EXIT=0"

echo x > a.txt
CLAUDE_CODE_SESSION_ID="$B" git add a.txt
has "no session id -> silent" \
    "$(env -u GIT_INDEX_FILE -u CLAUDE_CODE_SESSION_ID \
        bash "$SRC/pre-commit-foreign-index.sh" 2>&1; echo "EXIT=$?")" "EXIT=0"
rm -rf "$T"

# ------------------------------------------------ 2. the stager wins, not the observer
# The production failure. `post-index-change` fires on EVERY index write including
# `git status`, so "first observer wins" let a passer-by claim a peer's staged batch.
echo "== stager wins"
new_repo
echo base > s1.txt
echo gone > del.txt
git add -A > /dev/null 2>&1
git commit -qm base

echo mine > s1.txt
CLAUDE_CODE_SESSION_ID="$A" git add s1.txt
CLAUDE_CODE_SESSION_ID="$A" git rm -q --cached del.txt
eq "a staged DELETION is recorded" "$(owner_of del.txt)" "$A"

CLAUDE_CODE_SESSION_ID="$B" git status --short > /dev/null
eq "peer's git status does not steal an edit" "$(owner_of s1.txt)" "$A"
eq "peer's git status does not steal a deletion" "$(owner_of del.txt)" "$A"

rm -f .git/session-stage-log
CLAUDE_CODE_SESSION_ID="$B" git status --short > /dev/null
eq "cold log + peer status -> unknown, not the passer-by" "$(owner_of s1.txt)" "-"
out="$(guard "$B")"
has "unknown reads as foreign -> refuse" "$out" "EXIT=1"
has "refusal names the staged deletion" "$out" "del.txt"

echo more > s1.txt
CLAUDE_CODE_SESSION_ID="$A" git add s1.txt
eq "a real add still claims normally" "$(owner_of s1.txt)" "$A"

# Invocation form must not change attribution. `staging_op` classifies the tokens after
# `git` in /proc/$PPID/cmdline, and git's global flags come in two shapes: `--git-dir=X`
# is ONE token, but `-C <path>` and `--git-dir <path>` put the value in its own argv slot.
# A parser that skips the flag and then classifies the next token reads a PATH as the
# subcommand. Every add below is the same operation by the same session as the one above.
# `git -C` matters most: codescout-companion's worktree guard mandates that exact form.
echo formC > formC.txt
CLAUDE_CODE_SESSION_ID="$A" git -C "$PWD" add formC.txt
eq "-C <path> add is a staging op" "$(owner_of formC.txt)" "$A"

echo formJ > formJ.txt
CLAUDE_CODE_SESSION_ID="$A" git --git-dir="$PWD/.git" --work-tree="$PWD" add formJ.txt
eq "--git-dir=X joined add is a staging op" "$(owner_of formJ.txt)" "$A"

echo formS > formS.txt
CLAUDE_CODE_SESSION_ID="$A" git --git-dir "$PWD/.git" --work-tree "$PWD" add formS.txt
eq "--git-dir X separate add is a staging op" "$(owner_of formS.txt)" "$A"

echo formK > formK.txt
CLAUDE_CODE_SESSION_ID="$A" git -c user.name=t -C "$PWD" add formK.txt
eq "-c k=v then -C <path> add is a staging op" "$(owner_of formK.txt)" "$A"

# The other direction, and it is what stops "skip the value too" being over-applied: a
# NON-staging verb wearing the same flags must still fail to claim. A fix that swallows
# one token after every flag, or that returns 0 whenever it cannot classify, passes the
# four cases above and fails this one.
rm -f .git/session-stage-log
CLAUDE_CODE_SESSION_ID="$B" git -C "$PWD" status --short > /dev/null
eq "-C <path> status is NOT a staging op" "$(owner_of formC.txt)" "-"

rm -rf "$T"

# -------------------------------------------------------------- 3. owner resolution
echo "== owner resolution"
new_repo
LIVE="${CLAUDE_CODE_SESSION_ID:-}"
if [ -n "$LIVE" ]; then
    echo base > live.txt
    echo base > dead.txt
    git add -A > /dev/null 2>&1
    git commit -qm base
    echo x > live.txt
    echo x > dead.txt
    git add -A > /dev/null 2>&1
    git diff --cached --raw > raw.tmp
    : > .git/session-stage-log
    while IFS=$'\t' read -r blob path; do
        case "$path" in
            live.txt) o="$LIVE" ;;
            *) o="99999999-dead-dead-dead-999999999999" ;;
        esac
        printf '%s\t%s\t%s\n' "$o" "$blob" "$path" >> .git/session-stage-log
    done < <(awk -F'\t' '{ split($1, a, " "); print a[4] "\t" $2 }' raw.tmp)
    rm -f raw.tmp
    out="$(guard "00000000-1111-2222-3333-444444444444")"
    has "a live owner resolves" "$out" "LIVE — "
    has "a live owner gets an address" "$out" "cc-socks/"
    has "an absent owner is NOT LIVE" "$out" "NOT LIVE"
    has "and points at its transcript" "$out" ".jsonl"
else
    echo "  SKIP  owner resolution (no CLAUDE_CODE_SESSION_ID to resolve against)"
fi
rm -rf "$T"

# ---------------------------------- 4. the neighbouring hook still covers its own axis
# foreign-index covers CROSS-path capture; unreviewed-content covers INTRA-path, the
# working tree moving under a pathspec commit after you staged. Neither covers the
# other, so a change to one must never be read as covering both.
echo "== intra-path axis (pre-commit-unreviewed-content.sh)"
new_repo
echo base > f.txt
git add f.txt > /dev/null 2>&1
git commit -qm base
echo mine > f.txt
git add f.txt
echo "mine + THEIR LINE" > f.txt
cp .git/index .git/next-index-9.lock
GIT_INDEX_FILE=".git/next-index-9.lock" git add f.txt
out="$(GIT_INDEX_FILE=".git/next-index-9.lock" \
    bash "$SRC/pre-commit-unreviewed-content.sh" 2>&1; echo "EXIT=$?")"
has "working tree moved after staging -> refuse" "$out" "EXIT=1"
has "names the file" "$out" "f.txt"
rm -rf "$T"

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" = "0" ]
