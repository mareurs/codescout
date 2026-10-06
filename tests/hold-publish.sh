#!/usr/bin/env bash
#
# Suite for `scripts/hold-publish.sh` (publish hold, OB-20).
#
# Hermetic: every case builds a throwaway repo under $TMPDIR and points HOME at a
# throwaway directory. It never reads this checkout's history or the real session
# registry. Every absence assertion ("no ref created") is paired with a positive one
# (exit code, message) so a dead script cannot pass it.
#
# Usage:
#   tests/hold-publish.sh     # non-zero exit on any failure

set -uo pipefail

SCRIPT="$(cd "$(dirname "$0")/../scripts" && pwd)/hold-publish.sh"
SID="aaaaaaaa-1111-2222-3333-444444444444"
OTHER="bbbbbbbb-5555-6666-7777-888888888888"

PASS=0
FAIL=0
ok() { echo "  PASS  $1"; PASS=$((PASS + 1)); }
no() {
    echo "  FAIL  $1"
    [ -n "${2:-}" ] && echo "        $2"
    FAIL=$((FAIL + 1))
}
eq() { [ "$2" = "$3" ] && ok "$1" || no "$1" "want '$3', got '$2'"; }
has() { printf '%s' "$2" | grep -qF -- "$3" && ok "$1" || no "$1" "expected to find: $3 (in: $2)"; }
hasnt() { printf '%s' "$2" | grep -qF -- "$3" && no "$1" "should NOT contain: $3" || ok "$1"; }

[ -x "$SCRIPT" ] || { echo "FATAL: $SCRIPT is not executable"; exit 1; }

SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/hold-publish-XXXXXX")"
trap 'rm -r "$SCRATCH"' EXIT
FAKEHOME="$SCRATCH/home"
mkdir -p "$FAKEHOME"

REPO=""
new_repo() {
    REPO="$(mktemp -d "$SCRATCH/repo-XXXXXX")"
    git -C "$REPO" init -q -b main
    git -C "$REPO" config user.email t@example.invalid
    git -C "$REPO" config user.name  Test
    git -C "$REPO" config core.hooksPath /dev/null
    echo x > "$REPO/f.txt"
    git -C "$REPO" add f.txt
    git -C "$REPO" commit -q -m init
}

# run <sid|-> <args...>  -> sets OUT (stdout+stderr), EC. `-` means CLAUDE_CODE_SESSION_ID empty.
OUT=""; EC=0
run() {
    local sid="$1"; shift
    [ "$sid" = "-" ] && sid=""
    OUT="$(cd "$REPO" && HOME="$FAKEHOME" CLAUDE_CODE_SESSION_ID="$sid" bash "$SCRIPT" "$@" 2>&1)"
    EC=$?
}
holds() { git -C "$REPO" for-each-ref --format='%(refname)' refs/holds/; }

echo "== set: validation"
new_repo
run - set "why"
eq "set without a session id exits 2" "$EC" 2
eq "set without a session id creates no ref" "$(holds)" ""
has "set without a session id explains itself" "$OUT" "CLAUDE_CODE_SESSION_ID"

for bad in '../x' 'a b' 'a/b'; do
    new_repo
    run "$bad" set "why"
    eq "set with invalid session id '$bad' exits 2" "$EC" 2
    eq "set with invalid session id '$bad' creates no ref" "$(holds)" ""
    has "set with invalid session id '$bad' prints a message" "$OUT" "CLAUDE_CODE_SESSION_ID"
done

echo "== set: reason shapes"
new_repo
run "$SID" set
eq "set with no reason exits 0" "$EC" 0
eq "empty reason writes exactly 'reason: '" "$(git -C "$REPO" cat-file -p "refs/holds/$SID" | sed -n 1p)" "reason: "

new_repo
run "$SID" set waiting for the operator
eq "multi-argument reason is joined with spaces" "$(git -C "$REPO" cat-file -p "refs/holds/$SID" | sed -n 1p)" "reason: waiting for the operator"

new_repo
run "$SID" set "$(printf 'line one\nline two\tTabbed\r')"
eq "newline reason exits 0" "$EC" 0
B="$(git -C "$REPO" cat-file -p "refs/holds/$SID")"
eq "newline reason leaves exactly three lines" "$(printf '%s\n' "$B" | grep -c '')" 3
eq "newline reason is flattened onto line 1" "$(printf '%s\n' "$B" | sed -n 1p)" "reason: line one line two Tabbed "
case "$(printf '%s\n' "$B" | sed -n 2p)" in
    set-at:\ ????-??-??T??:??:??Z) ok "newline reason: line 2 is still set-at" ;;
    *) no "newline reason: line 2 is still set-at" "got: $(printf '%s\n' "$B" | sed -n 2p)" ;;
esac
case "$(printf '%s\n' "$B" | sed -n 3p)" in
    head:\ *) ok "newline reason: line 3 still starts with head:" ;;
    *) no "newline reason: line 3 still starts with head:" "got: $(printf '%s\n' "$B" | sed -n 3p)" ;;
esac

new_repo
HIJ="$(printf 'sneaky\nset-at: 1999-01-01T00:00:00Z')"
run "$SID" set "$HIJ"
B="$(git -C "$REPO" cat-file -p "refs/holds/$SID")"
case "$(printf '%s\n' "$B" | sed -n 2p)" in
    set-at:\ 1999*) no "hijack: stored set-at is the real time, not 1999" "got: $(printf '%s\n' "$B" | sed -n 2p)" ;;
    set-at:\ ????-??-??T??:??:??Z) ok "hijack: stored set-at is the real time, not 1999" ;;
    *) no "hijack: stored set-at is the real time, not 1999" "got: $(printf '%s\n' "$B" | sed -n 2p)" ;;
esac
run "$SID" set "$HIJ again"
B="$(git -C "$REPO" cat-file -p "refs/holds/$SID")"
eq "hijack: second set exits 0" "$EC" 0
case "$(printf '%s\n' "$B" | sed -n 2p)" in
    set-at:\ 1999*) no "hijack: set-at after a SECOND set is not 1999" "got: $(printf '%s\n' "$B" | sed -n 2p)" ;;
    set-at:\ ????-??-??T??:??:??Z) ok "hijack: set-at after a SECOND set is not 1999" ;;
    *) no "hijack: set-at after a SECOND set is not 1999" "got: $(printf '%s\n' "$B" | sed -n 2p)" ;;
esac

echo "== set: blob format"
new_repo
HEAD_SHA="$(git -C "$REPO" rev-parse HEAD)"
run "$SID" set "waiting on review"
eq "set exits 0" "$EC" 0
eq "set creates refs/holds/<sid> as a blob" "$(git -C "$REPO" cat-file -t "refs/holds/$SID" 2>&1)" blob
BLOB="$(git -C "$REPO" cat-file -p "refs/holds/$SID")"
eq "blob line 1 is the reason" "$(printf '%s\n' "$BLOB" | sed -n 1p)" "reason: waiting on review"
case "$(printf '%s\n' "$BLOB" | sed -n 2p)" in
    set-at:\ ????-??-??T??:??:??Z) ok "blob line 2 is set-at in UTC ISO-8601" ;;
    *) no "blob line 2 is set-at in UTC ISO-8601" "got: $(printf '%s\n' "$BLOB" | sed -n 2p)" ;;
esac
eq "blob line 3 is head at hold time" "$(printf '%s\n' "$BLOB" | sed -n 3p)" "head: $HEAD_SHA"

echo "== set twice"
SETAT1="$(printf '%s\n' "$BLOB" | sed -n 2p)"
echo y >> "$REPO/f.txt"; git -C "$REPO" commit -q -am second
HEAD2="$(git -C "$REPO" rev-parse HEAD)"
sleep 1.1
run "$SID" set "new reason"
BLOB2="$(git -C "$REPO" cat-file -p "refs/holds/$SID")"
eq "second set exits 0" "$EC" 0
eq "second set replaces reason" "$(printf '%s\n' "$BLOB2" | sed -n 1p)" "reason: new reason"
eq "second set keeps the first set-at" "$(printf '%s\n' "$BLOB2" | sed -n 2p)" "$SETAT1"
eq "second set replaces head" "$(printf '%s\n' "$BLOB2" | sed -n 3p)" "head: $HEAD2"

echo "== release"
run "$SID" release
eq "release with no argument exits 0" "$EC" 0
eq "release with no argument removes the caller's own hold" "$(holds)" ""

new_repo
run "$OTHER" set "theirs"
eq "setup: other session's hold exists" "$(holds)" "refs/holds/$OTHER"
run "$SID" release "$OTHER"
eq "release of another sid exits 0" "$EC" 0
has "release of another sid says another session" "$OUT" "another session"
has "release of another sid names that sid" "$OUT" "$OTHER"
eq "release of another sid removes it" "$(holds)" ""

run "$SID" release
eq "release of an absent hold exits 0" "$EC" 0
has "release of an absent hold says no hold" "$OUT" "no hold"

# Collateral: releasing another sid must leave the caller's own hold alone.
new_repo
run "$SID" set "mine"
run "$OTHER" set "theirs"
run "$SID" release "$OTHER"
eq "release of another sid exits 0 (collateral case)" "$EC" 0
eq "releasing another sid leaves the caller's own hold present" "$(holds)" "refs/holds/$SID"

# Invalid sid arguments: message, exit 0, other holds intact.
for bad in '../x' 'a/b'; do
    run "$SID" release "$bad"
    eq "release '$bad' exits 0" "$EC" 0
    has "release '$bad' prints a message" "$OUT" "no session id"
    eq "release '$bad' leaves other holds intact" "$(holds)" "refs/holds/$SID"
done

echo "== release: a failed delete is not reported as released"
new_repo
run "$SID" set "stuck"
REAL_GIT="$(command -v git)"
SHIM="$(mktemp -d "$SCRATCH/shim-XXXXXX")"
printf '#!/usr/bin/env bash\nif [ "${1:-}" = update-ref ] && [ "${2:-}" = -d ]; then exit 1; fi\nexec "%s" "$@"\n' "$REAL_GIT" > "$SHIM/git"
chmod +x "$SHIM/git"
OUT="$(cd "$REPO" && PATH="$SHIM:$PATH" HOME="$FAKEHOME" CLAUDE_CODE_SESSION_ID="$SID" bash "$SCRIPT" release 2>&1)"
EC=$?
eq    "release whose delete fails exits non-zero" "$((EC != 0))" 1
hasnt "release whose delete fails never says released" "$OUT" "released"
has   "release whose delete fails says so" "$OUT" "could not release"
eq    "the hold is still there after the failed release" "$(holds)" "refs/holds/$SID"
run "$SID" release
has   "without the shim the same release succeeds (positive control)" "$OUT" "released your hold"
eq    "and the hold is gone" "$(holds)" ""

echo "== list"
new_repo
run "$SID" list
eq "list with no holds exits 0" "$EC" 0
eq "list is empty with no holds" "$OUT" ""

# Registry: HOME/.claude-test/sessions/*.json
mkdir -p "$FAKEHOME/.claude-test/sessions"
printf '{"sessionId":"%s","pid":999999,"messagingSocketPath":"%s/none.sock","procStart":"1"}\n' \
    "$OTHER" "$SCRATCH" > "$FAKEHOME/.claude-test/sessions/x.json"
run "$SID" set "mine"          # SID has no registry row
run "$OTHER" set "dead one"    # OTHER has a row whose pid is dead
run "$SID" list
eq "list exits 0 with holds" "$EC" 0
SIDROW="$(printf '%s\n' "$OUT" | grep -F "$SID" || true)"
OTHERROW="$(printf '%s\n' "$OUT" | grep -F "$OTHER" || true)"
eq "list marks a session with no registry row as ?" "$(printf '%s' "$SIDROW" | cut -f2)" "?"
eq "list marks a row whose pid is dead as gone" "$(printf '%s' "$OTHERROW" | cut -f2)" "gone"
eq "list row has the reason as fourth column" "$(printf '%s' "$SIDROW" | cut -f4)" "mine"
eq "list prints one row per hold" "$(printf '%s\n' "$OUT" | grep -c .)" 2
case "$(printf '%s' "$SIDROW" | cut -f3)" in
    ''|*[!0-9a-z]*) no "list row has a non-empty age column" "got: $(printf '%s' "$SIDROW" | cut -f3)" ;;
    *) ok "list row has a non-empty age column" ;;
esac

echo
echo "== list without resolve-sids.sh beside the script"
ALONE="$(mktemp -d "$SCRATCH/alone-XXXXXX")"
cp "$SCRIPT" "$ALONE/hold-publish.sh"
OUT="$(cd "$REPO" && HOME="$FAKEHOME" CLAUDE_CODE_SESSION_ID="$SID" bash "$ALONE/hold-publish.sh" list 2>&1)"
EC=$?
eq "list with the helper missing exits 0" "$EC" 0
eq "list with the helper missing still prints both holds" "$(printf '%s\n' "$OUT" | grep -c .)" 2
eq "list with the helper missing marks the first row ?" "$(printf '%s\n' "$OUT" | grep -F "$SID" | cut -f2)" "?"
eq "list with the helper missing marks the second row ?" "$(printf '%s\n' "$OUT" | grep -F "$OTHER" | cut -f2)" "?"
hasnt "list with the helper missing reports no 'command not found'" "$OUT" "command not found"

echo
echo "PASS: $PASS  FAIL: $FAIL"
[ "$FAIL" -eq 0 ]
