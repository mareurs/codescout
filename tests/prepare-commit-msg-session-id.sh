#!/usr/bin/env bash
#
# Discrimination matrix for scripts/prepare-commit-msg-session-id.sh.
#
# WHY THIS EXISTS
# ---------------
# That hook had NO test of any kind until 2026-09-16 — `ls tests/` matched nothing and the
# only reference to it anywhere in tests/ was an incidental mention in the pre-push suite.
# It writes the one field that makes authorship answerable on a checkout where every
# session commits as the same git author, so a silent regression in it is a permanent,
# unrecoverable loss in every commit made while it was broken.
#
# Every case is a DISCRIMINATION: each asserts the hook writes what it must AND stays
# silent where it must. A suite checking only that a trailer appears passes against a hook
# that stamps unconditionally, which would invent authors.
#
# Runs entirely inside throwaway repos under $TMPDIR.

set -uo pipefail

SRC="$(cd "$(dirname "$0")/../scripts" && pwd)"
REAL_REPO="$(cd "$(dirname "$0")/.." && pwd)"
HOOK="$SRC/prepare-commit-msg-session-id.sh"
PASS=0
FAIL=0

ok() { echo "  PASS  $1"; PASS=$((PASS + 1)); }
no() {
    echo "  FAIL  $1"
    [ -n "${2:-}" ] && echo "        $2"
    FAIL=$((FAIL + 1))
}
eq() { [ "$2" = "$3" ] && ok "$1" || no "$1" "want '$3' got '$2'"; }

assert_throwaway() {
    _at="$(pwd -P)"
    case "$_at" in
        "$REAL_REPO" | "$REAL_REPO"/*)
            echo "REFUSING: running inside the real checkout: $_at" >&2
            exit 1
            ;;
    esac
}

new_repo() {
    T="$(mktemp -d)"
    cd "$T" || exit 1
    git init -q .
    assert_throwaway
    git config user.email t@t
    git config user.name t
    mkdir -p .git/hooks
    cat > .git/hooks/prepare-commit-msg <<SHIM
#!/usr/bin/env bash
exec "$HOOK" "\$@"
SHIM
    chmod +x .git/hooks/prepare-commit-msg
}

# A realistic message: a body AND an existing final-paragraph trailer block. The empty-body
# case would pass against a hook that appends blindly, because with nothing to append after
# there is no second block to open — which is the whole defect this hook's own comments are
# about.
msg() {
    printf 'subject line\n\nbody paragraph\n\nCo-Authored-By: Someone <x@y>\n' > "$T/m.txt"
}

# READ THE PARSER, NEVER THE BYTES. `grep` on the message file would pass against a trailer
# written into an earlier paragraph, which is exactly the failure being guarded — it is
# present in the text and absent to every query. `%(trailers:key=…)` is the consumer whose
# answer matters.
parsed() { git log -1 --format="%(trailers:key=$1,valueonly)" | sed '/^$/d' | tr '\n' ' '; }
n_parsed() { git log -1 --format="%(trailers:key=$1,valueonly)" | sed '/^$/d' | wc -l; }

A="aaaaaaaa-0000-0000-0000-aaaaaaaaaaaa"
B="bbbbbbbb-1111-1111-1111-bbbbbbbbbbbb"
C="cccccccc-2222-2222-2222-cccccccccccc"

echo "== Session-Id stamping"
new_repo
echo x > a.txt
git add a.txt
msg
CLAUDE_CODE_SESSION_ID="$A" git commit -q -F "$T/m.txt"
eq "stamps the committing session id"          "$(parsed Session-Id)" "$A "
eq "and leaves the existing trailer parseable" "$(parsed Co-Authored-By)" "Someone <x@y> "

# Absence is honest; a default would be a fabricated owner. A terminal commit gets nothing.
echo y > b.txt
git add b.txt
msg
env -u CLAUDE_CODE_SESSION_ID git commit -q -F "$T/m.txt"
eq "no session id -> no Session-Id trailer" "$(n_parsed Session-Id)" "0"
rm -rf "$T"

echo "== the acked co-authors, written rather than pasted"
new_repo
echo x > a.txt
git add a.txt
msg
CLAUDE_CODE_SESSION_ID="$A" CODESCOUT_INDEX_ACK="$B" git commit -q -F "$T/m.txt"
eq "a single acked sid is written"        "$(parsed Co-Authored-Session-Id)" "$B "
eq "and the committer's own id survives"  "$(parsed Session-Id)" "$A "

# THE MULTI-VALUE CASE, and it is the one the obvious implementation gets wrong. `--if-exists
# doNothing` — the flag the Session-Id call above correctly uses — writes only the FIRST
# trailer of a repeated key, so every co-author after the first vanishes with no error.
# Measured before this test existed: two distinct sids under doNothing yielded ONE trailer.
echo y > b.txt
git add b.txt
msg
CLAUDE_CODE_SESSION_ID="$A" CODESCOUT_INDEX_ACK="$B,$C" git commit -q -F "$T/m.txt"
eq "TWO acked sids write TWO trailers" "$(n_parsed Co-Authored-Session-Id)" "2"

# Idempotence across the rewriting operations. `--amend` re-runs the hook against a message
# that already carries the trailers; duplicates here would accumulate silently through every
# rebase and squash this branch sees.
CLAUDE_CODE_SESSION_ID="$A" CODESCOUT_INDEX_ACK="$B,$C" git commit -q --amend --no-edit
eq "an --amend re-run does not duplicate them" "$(n_parsed Co-Authored-Session-Id)" "2"

# THE SILENCE CONTROL. Without it every case above passes against a hook that stamps
# unconditionally — which would put co-authors on commits that have none.
echo z > c.txt
git add c.txt
msg
CLAUDE_CODE_SESSION_ID="$A" git commit -q -F "$T/m.txt"
eq "no ack -> no co-author trailer at all" "$(n_parsed Co-Authored-Session-Id)" "0"

# An empty ack is not an empty author. `CODESCOUT_INDEX_ACK=` reaching the hook as the empty
# string must behave as absent, not write a blank trailer.
echo w > d.txt
git add d.txt
msg
CLAUDE_CODE_SESSION_ID="$A" CODESCOUT_INDEX_ACK="" git commit -q -F "$T/m.txt"
eq "an EMPTY ack writes nothing" "$(n_parsed Co-Authored-Session-Id)" "0"

# `-` is the stage log's UNATTRIBUTED sentinel, never a session. The index guard refuses an
# ack naming it, so reaching here with one means the variable was set by hand — and
# recording a co-author that does not exist is worse than recording none.
echo v > e.txt
git add e.txt
msg
CLAUDE_CODE_SESSION_ID="$A" CODESCOUT_INDEX_ACK="-" git commit -q -F "$T/m.txt"
eq "the '-' sentinel is never written as a co-author" "$(n_parsed Co-Authored-Session-Id)" "0"

# And it must be dropped from a MIXED list rather than poisoning the whole ack — the real
# sid still deserves its trailer.
echo u > f.txt
git add f.txt
msg
CLAUDE_CODE_SESSION_ID="$A" CODESCOUT_INDEX_ACK="$B,-" git commit -q -F "$T/m.txt"
eq "a mixed list keeps the real sid" "$(parsed Co-Authored-Session-Id)" "$B "
rm -rf "$T"

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" = "0" ]
