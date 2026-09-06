#!/usr/bin/env bash
#
# Discrimination matrix for `scripts/pre-push-foreign-session-guard.sh`.
#
# WHY THIS EXISTS
# ---------------
# The guard refuses a push that would publish another session's commits. It is the
# pusher-side complement to "a session that cannot publish must not COMMIT to a shared
# branch" (docs/trackers/observer-blindness.md OB-20), and it is the kind of guard that
# fails silently in the wrong direction: one that refuses everything looks exactly as
# safe as one that works, right up until someone disables it, and one that refuses
# nothing looks identical to a quiet week.
#
# So every case below is a DISCRIMINATION — each asserts silence where silence is
# required AND refusal where refusal is required. Same posture as
# `tests/hooks-discrimination.sh`, whose idiom this borrows.
#
# HERMETIC ON PURPOSE. Every case builds its own throwaway repo under $TMPDIR with
# commits whose `Session-Id` trailers this file controls. It never reads this checkout's
# history: the first version of these cases cited live SHAs from the unpushed pile, which
# made the suite a measurement of one instant — `experiments` is rebased after every ship,
# so those SHAs die and the test would have started failing for a reason having nothing to
# do with the guard.
#
# Usage:
#   tests/pre-push-foreign-session-guard.sh     # non-zero exit on any failure

set -uo pipefail

GUARD="$(cd "$(dirname "$0")/../scripts" && pwd)/pre-push-foreign-session-guard.sh"
ZERO="0000000000000000000000000000000000000000"

ALICE="aaaaaaaa-1111-2222-3333-444444444444"
BOB="bbbbbbbb-5555-6666-7777-888888888888"
CAROL="cccccccc-9999-0000-1111-222222222222"

PASS=0
FAIL=0
ok() { echo "  PASS  $1"; PASS=$((PASS + 1)); }
no() {
    echo "  FAIL  $1"
    [ -n "${2:-}" ] && echo "        $2"
    FAIL=$((FAIL + 1))
}
eq() { [ "$2" = "$3" ] && ok "$1" || no "$1" "want exit $3, got $2"; }
has() { printf '%s' "$2" | grep -qF -- "$3" && ok "$1" || no "$1" "expected to find: $3"; }
hasnt() { printf '%s' "$2" | grep -qF -- "$3" && no "$1" "should NOT contain: $3" || ok "$1"; }

# Just the per-commit rows of a refusal (`    <sha8>  <sid>  <subject>`). Asserting
# "your sid is absent from the OUTPUT" would be wrong: the message prints `Your session
# id: <you>` on purpose, so the scope of that claim is the report rows, not the page.
report_rows() { printf '%s' "$1" | grep -E '^    [0-9a-f]{8}  ' || true; }

[ -x "$GUARD" ] || { echo "FATAL: $GUARD is not executable"; exit 1; }

# ---------------------------------------------------------------- throwaway repo builder
# commit <sid|-> <subject>   — `-` means NO trailer, the plain-terminal case.
REPO=""
new_repo() {
    REPO="$(mktemp -d "${TMPDIR:-/tmp}/prepush-guard-XXXXXX")"
    git -C "$REPO" init -q -b main
    git -C "$REPO" config user.email t@example.invalid
    git -C "$REPO" config user.name  Test
    # No hooks: this suite tests the guard directly, not the stamping hook.
    git -C "$REPO" config core.hooksPath /dev/null
}
commit() {
    local sid="$1" subject="$2" msg
    echo "$RANDOM$RANDOM" >> "$REPO/f.txt"
    git -C "$REPO" add f.txt
    if [ "$sid" = "-" ]; then
        msg="$subject"
    else
        printf -v msg '%s\n\nCo-Authored-By: T <t@example.invalid>\nSession-Id: %s' "$subject" "$sid"
    fi
    git -C "$REPO" commit -q -m "$msg"
}
sha() { git -C "$REPO" rev-parse "${1:-HEAD}"; }

# run <pusher-sid|-> <ack|-> <stdin-line>  -> sets OUT, EC
OUT=""; EC=0
run() {
    local pusher="$1" ack="$2" line="$3"
    local -a env=()
    [ "$pusher" != "-" ] && env+=("CLAUDE_CODE_SESSION_ID=$pusher")
    [ "$ack" != "-" ] && env+=("CODESCOUT_PUSH_ACK=$ack")
    OUT="$(printf '%s\n' "$line" | (cd "$REPO" && env -u CLAUDE_CODE_SESSION_ID -u CODESCOUT_PUSH_ACK "${env[@]}" "$GUARD" origin git@example.invalid:x) 2>&1)"
    EC=$?
}

echo
echo "== a push carrying only your own commits =="
new_repo
commit "$ALICE" "alice one"; BASE=$(sha)
commit "$ALICE" "alice two"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "allowed"                        "$EC" 0
eq  "and says nothing at all"        "$(printf '%s' "$OUT" | wc -c)" 0

echo
echo "== a push carrying another session's commit =="
new_repo
commit "$ALICE" "alice base"; BASE=$(sha)
commit "$BOB"   "bob's withheld work"
commit "$ALICE" "alice on top"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "refused"                        "$EC" 1
has "names the foreign sid"          "$OUT" "$BOB"
has "names the offending subject"    "$OUT" "bob's withheld work"
hasnt "your own commits are not in the report" "$(report_rows "$OUT")" "$ALICE"
hasnt "nor is your own subject"       "$(report_rows "$OUT")" "alice on top"
has "offers the ack, prefilled"      "$OUT" "CODESCOUT_PUSH_ACK=\"$BOB\""
has "points at the class"            "$OUT" "OB-20"

echo
echo "== the ack is per-session, not a switch =="
run "$ALICE" "$BOB"   "refs/heads/main $TIP refs/heads/main $BASE"
eq  "acking the right sid allows"    "$EC" 0
run "$ALICE" "$CAROL" "refs/heads/main $TIP refs/heads/main $BASE"
eq  "acking a DIFFERENT sid still refuses" "$EC" 1
run "$ALICE" "all"    "refs/heads/main $TIP refs/heads/main $BASE"
eq  "ack=all allows"                 "$EC" 0

echo
echo "== two foreign sessions: the ack must name both =="
new_repo
commit "$ALICE" "base"; BASE=$(sha)
commit "$BOB"   "bob work"
commit "$CAROL" "carol work"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "refused"                        "$EC" 1
# Order is git-log order (newest first), not commit order, so assert MEMBERSHIP rather
# than a concatenation — pinning the string would test the traversal, not the guard.
ACKLINE="$(printf '%s' "$OUT" | grep -F 'CODESCOUT_PUSH_ACK=' || true)"
has "prefilled ack names bob"        "$ACKLINE" "$BOB"
has "prefilled ack names carol"      "$ACKLINE" "$CAROL"
run "$ALICE" "$BOB" "refs/heads/main $TIP refs/heads/main $BASE"
eq  "acking one of two still refuses" "$EC" 1
run "$ALICE" "$BOB,$CAROL" "refs/heads/main $TIP refs/heads/main $BASE"
eq  "acking both allows"             "$EC" 0

echo
echo "== a commit with NO trailer is reported, never attributed =="
# REGRESSION, and the reason this file exists at its current shape. With a TAB
# delimiter, `sha<TAB><TAB>subject` collapses (tab is IFS whitespace) and the SUBJECT
# lands in the sid field: every untrailered commit was refused as foreign, "authored by"
# its own subject line. Observed red before the fix. The guard now uses 0x1F.
new_repo
commit "$ALICE" "alice base"; BASE=$(sha)
commit "-"      "a plain terminal commit"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "allowed, not refused"           "$EC" 0
has "but reported"                   "$OUT" "no Session-Id trailer"
has "and named"                      "$OUT" "a plain terminal commit"
hasnt "subject never read as a sid"  "$OUT" "CODESCOUT_PUSH_ACK=\"a plain terminal commit\""
hasnt "and no refusal banner"        "$OUT" "REFUSING"

echo
echo "== untrailered AND foreign: report the one, refuse for the other =="
new_repo
commit "$ALICE" "alice base"; BASE=$(sha)
commit "-"      "terminal commit"
commit "$BOB"   "bob work"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "refused"                        "$EC" 1
has "still reports the untrailered"  "$OUT" "no Session-Id trailer"
has "refuses for the foreign one"    "$OUT" "$BOB"

echo
echo "== a partial refspec publishes a PREFIX, and the guard follows it =="
# The escape that matters: the author of a bottom commit can always publish it alone,
# and nobody else can publish it by pushing their own work on top.
new_repo
commit "$ALICE" "alice base"; BASE=$(sha)
commit "$BOB"   "bob bottom"; BOB_C=$(sha)
commit "$ALICE" "alice above bob"; TIP=$(sha)
run "$BOB"   - "refs/heads/main $BOB_C refs/heads/main $BASE"
eq  "bob may publish bob's own commit"        "$EC" 0
run "$ALICE" - "refs/heads/main $BOB_C refs/heads/main $BASE"
eq  "alice may not publish it for him"        "$EC" 1
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "nor by pushing her own work above it"    "$EC" 1

echo
echo "== cases the guard must stay out of =="
new_repo
commit "$ALICE" "base"; BASE=$(sha)
commit "$BOB"   "bob work"; TIP=$(sha)
run "-"      - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "no session id (a human): allowed, silent" "$EC" 0
eq  "  and truly silent"             "$(printf '%s' "$OUT" | wc -c)" 0
run "$ALICE" - "refs/heads/main $ZERO refs/heads/main $BASE"
eq  "branch deletion: allowed"       "$EC" 0
run "$ALICE" - "refs/tags/v1 $TIP refs/tags/v1 $ZERO"
eq  "tag push: allowed"              "$EC" 0
run "$ALICE" - ""
eq  "empty stdin: allowed"           "$EC" 0

echo
echo "== a brand-new remote branch does not enumerate all of history =="
# remote_sha is all-zeros for a branch the remote has never seen. Without `--not
# --remotes` the range is the whole history and the guard refuses on the first ancestor
# it meets, which would make every new branch unpushable.
new_repo
commit "$BOB"   "ancient, already published"
git -C "$REPO" update-ref refs/remotes/origin/main HEAD
commit "$ALICE" "new work"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $ZERO"
eq  "allowed: bob's published commit is not re-litigated" "$EC" 0

echo
echo "-------------------------------------------"
echo "  $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
