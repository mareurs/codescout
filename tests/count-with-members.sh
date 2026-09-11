#!/usr/bin/env bash
#
# Discrimination matrix for scripts/count-with-members.sh.
#
# WHY THIS EXISTS
# ---------------
# The tool's whole claim is that a count and the members it summarises cannot be
# derived separately, because bug-fix-session-log:F-133 records two cases where a
# CORRECT count sat beside a wrong membership list and actively corroborated it.
# A suite that only checked the counts would therefore be the defect under test.
#
# So every case below asserts on MEMBERSHIP, and the two counting assertions exist
# only to check that the count agrees with the members printed next to it.
#
# Fixtures are throwaway git repos under mktemp -d — never this checkout, which
# several sessions share and whose history changes under the suite.
set -u

SRC="$(cd "$(dirname "$0")/../scripts" && pwd)"
TOOL="$SRC/count-with-members.sh"
PASS=0
FAIL=0

has() { # has <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        PASS=$((PASS + 1)); echo "  ok   $1"
    else
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- expected to find: $3"
        printf '       got: %s\n' "$2" | head -8
    fi
}

hasnt() { # hasnt <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- expected NOT to find: $3"
        printf '       got: %s\n' "$2" | head -8
    else
        PASS=$((PASS + 1)); echo "  ok   $1"
    fi
}

eq() { # eq <label> <actual> <expected>
    if [ "$2" = "$3" ]; then
        PASS=$((PASS + 1)); echo "  ok   $1"
    else
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- expected [$3], got [$2]"
    fi
}

SID_A="aaaaaaaa-0000-0000-0000-000000000001"
SID_B="bbbbbbbb-0000-0000-0000-000000000002"

REPO="$(mktemp -d)"
trap 'rm -rf "$REPO"' EXIT
cd "$REPO" || exit 1
git init -q . 2>/dev/null || git init -q .
git config user.email t@example.com
git config user.name Tester

mk() { # mk <subject> <body-or-empty> <trailer-or-empty>
    echo "$RANDOM$RANDOM" > f.txt
    git add f.txt
    msg="$1"
    [ -n "$2" ] && msg="$msg

$2"
    [ -n "$3" ] && msg="$msg

Session-Id: $3"
    git commit -q -m "$msg"
    git rev-parse --short=8 HEAD
}

# A real root commit, so BASE is a sha. `git rev-parse HEAD` on an unborn branch
# prints the literal string "HEAD" AND exits non-zero, so `|| true` captures "HEAD"
# and the range silently becomes HEAD..HEAD -- an empty range that passes as a clean
# run. Found by this suite on its first execution.
git commit -q --allow-empty -m "root"
BASE=$(git rev-parse --short=8 HEAD)
C1=$(mk "first"  ""                                        "$SID_A")
# THE DISCRIMINATOR: authored by B, but its BODY names A's id. A grep over %b
# attributes this commit to A; git's trailer parser does not. Measured on the real
# repo 2026-09-11: that grep returned 21 for a session owning 20 commits in range.
C2=$(mk "second" "Discussed with session $SID_A at length."  "$SID_B")
C3=$(mk "third"  "no trailer at all"                         "")
C4=$(mk "fourth" ""                                          "$SID_A")
RANGE="${BASE}..HEAD"

OUT=$(bash "$TOOL" "$RANGE" 2>&1)
echo "scripts/count-with-members.sh"
echo "--- grouping ---"
A_LINE=$(printf '%s\n' "$OUT" | grep -F "$SID_A" || true)
B_LINE=$(printf '%s\n' "$OUT" | grep -F "$SID_B" || true)
N_LINE=$(printf '%s\n' "$OUT" | grep -F "(none)" || true)

has   "A's line lists A's first commit"            "$A_LINE" "$C1"
has   "A's line lists A's second commit"           "$A_LINE" "$C4"
hasnt "a BODY MENTION does not confer authorship"  "$A_LINE" "$C2"
has   "B owns the commit whose body named A"       "$B_LINE" "$C2"
has   "a trailerless commit is reported, not dropped" "$N_LINE" "$C3"
eq    "A's count matches the members beside it"    "$(echo "$A_LINE" | awk '{print $1}')" "2"
eq    "B's count matches the members beside it"    "$(echo "$B_LINE" | awk '{print $1}')" "1"

echo "--- completeness ---"
MEMBERS=$(printf '%s\n' "$OUT" | grep -v '^total:' | awk '{for(i=3;i<=NF;i++) print $i}' | sort)
eq "every commit in the range appears exactly once" "$(printf '%s\n' "$MEMBERS" | wc -l | tr -d ' ')" "4"
eq "no member is printed twice"                     "$(printf '%s\n' "$MEMBERS" | sort -u | wc -l | tr -d ' ')" "4"
has "the total is stated"                           "$OUT" "total: 4 commit(s)"

echo "--- the invariant: no count without members ---"
CO=$(bash "$TOOL" --count-only "$RANGE" 2>&1; echo "rc=$?")
has "--count-only is refused"                  "$CO" "no count-only mode"
has "the refusal states WHY, not just 'unknown option'" "$CO" "F-133"
has "--count-only exits non-zero"              "$CO" "rc=2"

echo "--- the self-guard (mutation on a copy of the production path) ---"
# Sabotage the member-matching line so the printed members cannot account for the
# range. The guard must refuse rather than print a plausible partial table.
MUT="$REPO/mutated.sh"
sed 's/if (v == want) printf/if (0) printf/' "$TOOL" > "$MUT"
MOUT=$(bash "$MUT" "$RANGE" 2>&1; echo "rc=$?")
has "a breakdown that does not account for the range is refused" "$MOUT" "must not be used"
has "the refusal exits non-zero"                                 "$MOUT" "rc=1"
# Control: the unmutated tool on the same range does NOT emit that refusal, so the
# assertion above discriminates rather than matching a string the tool always prints.
hasnt "the control run does not emit the refusal"                "$OUT"  "must not be used"

echo "--- other trailers, and edges ---"
BY=$(bash "$TOOL" --by Co-Authored-By "$RANGE" 2>&1)
has "an unknown trailer groups everything under (none)" "$BY" "(none)"
EMPTY=$(bash "$TOOL" "HEAD..HEAD" 2>&1)
has "an empty range says so"                            "$EMPTY" "0 commits"
BAD=$(bash "$TOOL" "not-a-real-ref..HEAD" 2>&1; echo "rc=$?")
has "an invalid range is refused"                       "$BAD" "rc=2"

echo
echo "passed: $PASS   failed: $FAIL"
[ "$FAIL" -eq 0 ]
