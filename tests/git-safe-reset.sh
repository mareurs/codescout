#!/usr/bin/env bash
# tests/git-safe-reset.sh — cases for scripts/git-safe-reset.sh
#
# WHAT THIS GUARDS, AND WHY THE MESSAGE ASSERTIONS ARE NOT DECORATION
#
# docs/issues/archive/2026-09-15-git-reset-mixed-silently-unstages-every-peer-on-a-shared-checkout.md:
# `git reset --mixed` (git's own default) rewrites `.git/index` -- ONE file per
# checkout, not per session -- discarding whatever ANY OTHER session had staged,
# with no error and nothing for the victim to attribute it to. A suite that only
# asserts "the refusal happened" and "the bytes did not change" is green against a
# refusal that sends its reader nowhere useful -- CLAUDE.md § *Testing Discipline*:
# a suite tests a guard's PREDICATE and never its REMEDY TEXT, and the remedy is
# where the reader's next action comes from. So the cases below assert on the exit
# code, on the staged-file bytes, AND on the refusal naming the correct staged
# count and calling out to the provenance scan for "whose" -- deleting either half
# keeps the coarse assertions green and reds only these.
#
# FIXTURE: throwaway git repos under mktemp, never this checkout. `git reset` and
# `git add` mutate the tree they run in, and a suite that touched this repo's own
# index to test a script whose whole purpose is protecting a shared index would be
# the exact failure mode this exists to prevent.
set -u

SRC="$(cd "$(dirname "$0")/../scripts" && pwd)"
TOOL="$SRC/git-safe-reset.sh"
PASS=0
FAIL=0

has() { # has <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        PASS=$((PASS + 1)); echo "  ok   $1"
    else
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- expected to find: $3"
        printf '       got: %s\n' "$2" | head -12
    fi
}

hasnt() { # hasnt <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- expected NOT to find: $3"
    else
        PASS=$((PASS + 1)); echo "  ok   $1"
    fi
}

eq() { # eq <label> <actual> <expected>
    if [ "$2" = "$3" ]; then
        PASS=$((PASS + 1)); echo "  ok   $1"
    else
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- expected '$3', got '$2'"
    fi
}

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Stub for scripts/file-provenance.py. Attribution is stubbed rather than driven
# by real transcripts because the thing under test is the REFUSAL and its COUNT,
# not the scan -- and a suite that needed this machine's live profiles would run
# nowhere else. Marker-writing mirrors tests/fmt-mine.sh's mkstub: the marker is
# written by the STUB when invoked, so "was provenance ever consulted" is
# observable without instrumenting git-safe-reset.sh itself.
mkstub() {
    local f="$WORK/prov.sh"
    cat > "$f" <<'STUB'
#!/usr/bin/env bash
[ -n "${GIT_SAFE_RESET_TEST_MARKER:-}" ] && : > "$GIT_SAFE_RESET_TEST_MARKER"
for f in "$@"; do
    printf 'written by THIS session and 5399543d-22d6-4ed9-9ebb-876be459989f  [LIVE] -- %s\n' "$f"
done
exit 0
STUB
    chmod +x "$f"
    echo "$f"
}
PROV_STUB=$(mkstub)

# A repo per case: reset mutates HEAD and the index in place.
newrepo() {
    local d="$WORK/r$RANDOM$RANDOM"
    mkdir -p "$d"
    git -C "$d" init -q
    git -C "$d" config user.email t@t.com
    git -C "$d" config user.name t
    echo one > "$d/a.txt"
    git -C "$d" add a.txt
    git -C "$d" commit -qm c1
    echo two > "$d/b.txt"
    git -C "$d" add b.txt
    git -C "$d" commit -qm c2
    echo "$d"
}

run() { # run <repodir> [args...] -> sets OUT and RC; always injects sid + stub
    local d="$1"; shift
    rm -f "$WORK/consulted"
    OUT=$(cd "$d" && GIT_SAFE_RESET_PROVENANCE="$PROV_STUB" CLAUDE_CODE_SESSION_ID=test-sid \
        GIT_SAFE_RESET_TEST_MARKER="$WORK/consulted" "$TOOL" "$@" 2>&1)
    RC=$?
}

consulted() { [ -f "$WORK/consulted" ] && echo yes || echo no; }

echo "== 1. no mode flag defaults to --soft: HEAD moves, index is untouched =="
R=$(newrepo)
echo staged > "$R/c.txt"
git -C "$R" add c.txt
BEFORE_HEAD=$(git -C "$R" rev-parse HEAD)
run "$R" HEAD~1
eq "bare call exits 0" "$RC" "0"
AFTER_HEAD=$(git -C "$R" rev-parse HEAD)
eq "bare call actually moved HEAD" "$([ "$BEFORE_HEAD" != "$AFTER_HEAD" ] && echo moved)" "moved"
has "bare call kept the pre-existing staged file" "$(git -C "$R" diff --cached --name-only)" "b.txt"
has "bare call kept the newly staged file too" "$(git -C "$R" diff --cached --name-only)" "c.txt"
hasnt "--soft never needs the provenance scan" "$(consulted)" "yes"

echo "== 2. explicit --soft behaves identically to the bare form =="
R=$(newrepo)
run "$R" --soft HEAD~1
eq "--soft exits 0" "$RC" "0"
has "--soft kept the index staged" "$(git -C "$R" diff --cached --name-only)" "b.txt"

echo "== 3. --mixed refuses, names the exact staged count, and never touches HEAD or the index =="
R=$(newrepo)
echo staged > "$R/c.txt"
echo staged > "$R/d.txt"
git -C "$R" add c.txt d.txt
BEFORE_HEAD=$(git -C "$R" rev-parse HEAD)
run "$R" --mixed HEAD~1
eq "--mixed exits 1" "$RC" "1"
eq "--mixed left HEAD alone" "$(git -C "$R" rev-parse HEAD)" "$BEFORE_HEAD"
has "--mixed left both files staged" "$(git -C "$R" diff --cached --name-only)" "c.txt"
has "--mixed left both files staged (2)" "$(git -C "$R" diff --cached --name-only)" "d.txt"
has "--mixed says REFUSED" "$OUT" "REFUSED"
has "--mixed names the correct count" "$OUT" "2 file(s) are staged"
has "--mixed lists the first staged file" "$OUT" "c.txt"
has "--mixed lists the second staged file" "$OUT" "d.txt"
has "--mixed reached the provenance scan" "$(consulted)" "yes"
has "--mixed's refusal carries the scan's answer" "$OUT" "5399543d-22d6-4ed9-9ebb-876be459989f"
has "--mixed says why there is no --force" "$OUT" "no --force here"
has "--mixed offers the raw git command as the deliberate bypass" "$OUT" "git reset --mixed HEAD~1"
has "--mixed points back to --soft as the likely answer" "$OUT" "--soft"

echo "== 4. --hard refuses like --mixed AND names the extra working-tree cost =="
R=$(newrepo)
echo staged > "$R/c.txt"
git -C "$R" add c.txt
run "$R" --hard HEAD~1
eq "--hard exits 1" "$RC" "1"
has "--hard says REFUSED" "$OUT" "REFUSED"
has "--hard names the extra working-tree warning" "$OUT" "discards every UNCOMMITTED working-tree edit"
has "--hard still offers the raw command" "$OUT" "git reset --hard HEAD~1"

echo "== 5. --mixed's working-tree warning is --hard-specific, not printed for --mixed =="
R=$(newrepo)
echo staged > "$R/c.txt"
git -C "$R" add c.txt
run "$R" --mixed HEAD~1
hasnt "--mixed does not claim to discard working-tree edits" "$OUT" "discards every UNCOMMITTED working-tree edit"

echo "== 6. nothing staged: --mixed still refuses -- the check and the act are two commands =="
R=$(newrepo)
run "$R" --mixed HEAD~1
eq "zero-staged --mixed still exits 1" "$RC" "1"
has "zero-staged refusal explains the race, not a false zero count" "$OUT" "two separate commands"
hasnt "zero-staged refusal does not claim a specific stale count" "$OUT" "0 file(s) are staged"

echo "== 7. no session id: fails CLOSED, offers the direct commands, never touches the repo =="
R=$(newrepo)
echo staged > "$R/c.txt"
git -C "$R" add c.txt
BEFORE_HEAD=$(git -C "$R" rev-parse HEAD)
rm -f "$WORK/consulted"
OUT=$(cd "$R" && env -u CLAUDE_CODE_SESSION_ID GIT_SAFE_RESET_PROVENANCE="$PROV_STUB" \
    GIT_SAFE_RESET_TEST_MARKER="$WORK/consulted" "$TOOL" --mixed HEAD~1 2>&1); RC=$?
eq "no sid exits 2, not 1" "$RC" "2"
eq "no sid left HEAD alone" "$(git -C "$R" rev-parse HEAD)" "$BEFORE_HEAD"
has "no sid names the missing referent" "$OUT" "there is no session identity"
has "no sid names the solo-checkout fallback" "$OUT" "git reset --soft <commit>"
hasnt "no sid ever reached the provenance scan" "$(consulted)" "yes"

echo "== 8. outside a git repo: exits 2 rather than crashing =="
OUT=$(cd "$WORK" && CLAUDE_CODE_SESSION_ID=test-sid "$TOOL" --mixed HEAD 2>&1); RC=$?
eq "non-repo exits 2" "$RC" "2"
has "non-repo names the reason" "$OUT" "not inside a git repository"

echo
echo "git-safe-reset: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
