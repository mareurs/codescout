#!/usr/bin/env bash
#
# Discrimination matrix for scripts/attribute-red.py.
#
# WHY THIS EXISTS
# ---------------
# The tool speaks at the single worst moment for a wrong answer: a build is red and the
# reader is looking for a party to blame. Every case below discriminates against one of
# three ways it could be worse than silence.
#
#   1. LIFETIME AUTHORS AS CURRENT ONES. `scan()` returns everyone who ever wrote a path.
#      The question at a red is who made the UNCOMMITTED delta. Measured while building
#      this (2026-09-08): the first version skipped the window and attributed a bug file
#      THIS session had just edited to two peers who wrote it days earlier -- and not to
#      the actual editor. It shipped a confident wrong name into a blame moment.
#      Guarded by "== THE WINDOW ==" below.
#
#   2. MENTION AS SUBJECT. A red's text also carries the command line, the crate graph and
#      whatever prose a test printed. A path that merely APPEARS is not a path the red is
#      about. This is `tests/file-provenance.sh`'s mention-is-not-authorship law one layer
#      up: there the noise was a transcript, here it is the failure text.
#
#   3. ABSENCE AS EXONERATION. "No record names this path" is a statement about COVERAGE,
#      never about ownership -- a Bash write the heuristic misses looks identical to no
#      write at all. Rendering it as "nobody" or "not yours" is the failure mode
#      file-provenance.py refuses, and it costs more here.
#
# Fixtures are synthetic transcripts against a real throwaway git repo: the window is
# derived from `git log -1 --format=%cI`, so it cannot be tested without one.
set -u

SRC="$(cd "$(dirname "$0")/../scripts" && pwd)"
TOOL="$SRC/attribute-red.py"
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

empty() { # empty <label> <haystack> -- byte-emptiness, which grep cannot express:
          # `grep -qF ""` against zero bytes finds no LINE and returns 1, so the
          # natural spelling of "expect nothing" reports FAIL on a correct run.
    if [ -z "$2" ]; then
        PASS=$((PASS + 1)); echo "  ok   $1"
    else
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- expected NO output"
        printf '       got: %s\n' "$2" | head -8
    fi
}

hasnt() { # hasnt <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- must NOT contain: $3"
        printf '       got: %s\n' "$2" | head -8
    else
        PASS=$((PASS + 1)); echo "  ok   $1"
    fi
}

ME="11111111-aaaa-bbbb-cccc-000000000001"
PEER="22222222-aaaa-bbbb-cccc-000000000002"

T="$(mktemp -d)"
trap 'rm -rf "$T"' EXIT
PA="$T/profileA/projects/-repo"
PB="$T/profileB/projects/-repo"
ROOTS="$PA:$PB"
REG_A="$T/regA/sessions"
REG="$REG_A"
SOCKDIR="$T/socks"
mkdir -p "$PA" "$PB" "$REG_A" "$SOCKDIR" "$T/repo/src" "$T/repo/docs"

A="$PA/$ME.jsonl"
B="$PB/$PEER.jsonl"

# tool_use <session-file> <tool-name> <input-json> [iso-timestamp]
tool_use() {
    python3 - "$1" "$2" "$3" "${4:-}" <<'PY'
import json, sys
f, name, inp, ts = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
rec = {"type": "assistant", "message": {"content": [
    {"type": "tool_use", "name": name, "input": json.loads(inp)}]}}
if ts:
    rec["timestamp"] = ts
open(f, "a").write(json.dumps(rec) + "\n")
PY
}

# registry_row <sessions-dir> <pid> <sid> <name> <socket-path> [status]
registry_row() {
    python3 - "$1" "$2" "$3" "$4" "$5" "${6:-idle}" <<'PY'
import json, sys
d, pid, sid, name, sock, status = sys.argv[1:7]
json.dump({"pid": int(pid), "sessionId": sid, "name": name,
           "messagingSocketPath": sock, "status": status,
           "cwd": "/home/x/repo"}, open(f"{d}/{pid}.json", "w"))
PY
}

# run <stdin-text> [argv...] -- the PRODUCTION entry shape: the red arrives on stdin.
run() {
    local text="$1"; shift
    printf '%s' "$text" | REPO_ROOT="$T/repo" FILE_PROVENANCE_ROOTS="$ROOTS" \
        FILE_PROVENANCE_REGISTRY_ROOTS="$REG" \
        CLAUDE_CODE_SESSION_ID="$ME" python3 "$TOOL" "$@" 2>&1
}

G="git -C $T/repo"
$G init -q 2>/dev/null
$G config user.email t@t
$G config user.name t
$G config commit.gpgsign false

echo
echo "== STAGE 1: a clean tree is silent, and the silence is explainable =="
# Nothing is dirty, so no uncommitted state can be behind any red. This is the
# 0.01s question that must stop the 7s one, and it is the common case.
RED_FOO="error[E0277]: \`X\` doesn't implement Debug
  --> src/foo.rs:536:9"
empty "clean tree says nothing at all"      "$(run "$RED_FOO")"
hasnt "clean tree emits no banner"          "$(run "$RED_FOO")" "UNCOMMITTED"
has "and --explain names WHY it was silent" "$(run "$RED_FOO" --explain)" "working tree is clean"

echo
echo "== STAGE 2: a red that names no DIRTY path is silent =="
echo "v1" > "$T/repo/src/committed.rs"
echo "v1" > "$T/repo/src/other.rs"
$G add -A && $G commit -q -m seed
echo "dirty" >> "$T/repo/src/other.rs"          # dirty, but the red does not name it
hasnt "a red naming only CLEAN files is silent" "$(run "$RED_FOO")" "UNCOMMITTED"
has "and --explain reports both counts"         "$(run "$RED_FOO" --explain)" "names no dirty path"
# The silence must not read as an exoneration -- it is a scope statement.
has "silence disclaims itself"                  "$(run "$RED_FOO" --explain)" "says nothing about whether the red is yours"

echo
echo "== STAGE 2: which paths a red is ABOUT (mention is not subject) =="
# Every case here pipes real toolchain text on STDIN rather than passing --paths, so
# named_paths() is exercised in its production shape. A suite that only ever used
# --paths would leave the stdin parser -- the only way run_command reaches this --
# entirely unobserved while reporting full coverage.
echo "dirty" >> "$T/repo/src/committed.rs"
tool_use "$B" mcp__codescout__edit_file '{"path":"src/committed.rs","old_string":"a","new_string":"b"}'

has "rustc span --> path:line:col" \
    "$(run "error[E0277]: nope
  --> src/committed.rs:536:9
   |")" "src/committed.rs"
has "a panic site" \
    "$(run "thread 'x' panicked at src/committed.rs:12:5:
assertion failed")" "src/committed.rs"
has "a quoted panic site" \
    "$(run "thread 'x' panicked at 'src/committed.rs:12:5'")" "src/committed.rs"
has "an error: line naming the file" \
    "$(run "error: could not compile src/committed.rs because reasons")" "src/committed.rs"
has "a bare Error: line for non-rust files" \
    "$(run "Error: src/committed.rs failed the check")" "src/committed.rs"

# ...and the negative half, which is the one that makes the above worth having.
hasnt "a path in the COMMAND LINE is not the subject" \
    "$(run "     Running \`cargo test --manifest-path src/committed.rs\`
test result: FAILED")" "UNCOMMITTED"
hasnt "a path in ordinary prose is not the subject" \
    "$(run "note: consider looking at src/committed.rs for context")" "UNCOMMITTED"
hasnt "a path inside a diff hunk header is not the subject" \
    "$(run "+++ b/src/committed.rs
@@ -1 +1 @@")" "UNCOMMITTED"

echo
echo "== STAGE 3: a PEER's dirty file resolves to an address, not just a name =="
touch "$SOCKDIR/live.sock"
registry_row "$REG_A" "$$" "$PEER" "peer-name-42" "$SOCKDIR/live.sock" busy
out="$(run "$(printf 'error[E0277]: nope\n  --> src/committed.rs:536:9\n')")"
has "the banner fires"                    "$out" "UNCOMMITTED"
has "and names the peer"                  "$out" "$PEER"
has "and marks them live"                 "$out" "[LIVE]"
has "and gives the cross-profile address" "$out" "uds:$SOCKDIR/live.sock"
has "and says who is NOT being accused"   "$out" "names who WROTE the file, never who broke the build"
# The ceiling, at the site. An alarm nothing reaches is as informative as no alarm, and
# a reader who does not know Bash bypasses this will read silence as "nobody".
has "and names the Bash reachability ceiling" "$out" "Bash"
has "and refuses to let silence mean nobody"  "$out" "silence is not 'nobody'"

echo
echo "== STAGE 3: MY OWN dirty file is not routed to me as someone else's =="
echo "dirty" >> "$T/repo/src/other.rs"
tool_use "$A" mcp__codescout__edit_file '{"path":"src/other.rs","old_string":"a","new_string":"b"}'
out="$(run "$(printf 'error: bad\n  --> src/other.rs:1:1\n')")"
has "my own write is named as mine"    "$out" "THIS session"
has "and says the red is likely mine"  "$out" "likely yours"
hasnt "and offers no peer to go ask"   "$out" "ask it:"

echo
echo "== STAGE 3: no record is a COVERAGE statement, never an exoneration =="
echo "orphan" > "$T/repo/src/unrecorded.rs"    # dirty (untracked), zero transcript records
# NOT named `nobody.rs`: the case below asserts the word "nobody" never appears, and a
# fixture path containing it can only ever fail. Renaming the fixture is load-bearing.
out="$(run "$(printf 'error: bad\n  --> src/unrecorded.rs:1:1\n')")"
has "an unrecorded write still reports the file" "$out" "src/unrecorded.rs"
has "and calls the gap COVERAGE"                 "$out" "COVERAGE"
# The proposition is "it never ASSERTS that nobody wrote this", and the bare word
# "nobody" is a bad proxy for it: the closing scope line legitimately contains that
# word, in the sentence refusing exactly this reading. Assert the claim shape.
hasnt "and never asserts nobody wrote it"        "$out" "nobody wrote"
hasnt "nor its polite spelling"                  "$out" "no one wrote"
hasnt "and never exonerates the reader"          "$out" "not yours"

echo
echo "== THE WINDOW -- the defect that was actually shipped =="
# scan() returns a path's LIFETIME authors. The question at a red is who made the
# UNCOMMITTED delta, so the floor is the path's last commit. Without this, a file the
# reader edited thirty seconds ago is attributed to whoever touched it last week.
# This is a per-SITE mutation target: file-provenance's own main() derives the same
# floor, and a kill there says nothing about this call.
echo "v1" > "$T/repo/src/windowed.rs"
$G add src/windowed.rs && $G commit -q -m "commit windowed"
WTS="$($G log -1 --format=%cI -- src/windowed.rs)"
BEFORE="$(python3 -c "
import datetime; d=datetime.datetime.fromisoformat('$WTS')
print((d-datetime.timedelta(days=2)).isoformat())")"
AFTER="$(python3 -c "
import datetime; d=datetime.datetime.fromisoformat('$WTS')
print((d+datetime.timedelta(minutes=5)).isoformat())")"
echo "dirty" >> "$T/repo/src/windowed.rs"

tool_use "$B" mcp__codescout__edit_file '{"path":"src/windowed.rs","old_string":"a","new_string":"b"}' "$BEFORE"
RED_W="$(printf 'error: bad\n  --> src/windowed.rs:1:1\n')"
out="$(run "$RED_W")"
hasnt "a write BEFORE the last commit is not the dirty author" "$out" "$PEER"
has "and the file still reports as unattributed"               "$out" "COVERAGE"

tool_use "$A" mcp__codescout__edit_file '{"path":"src/windowed.rs","old_string":"c","new_string":"d"}' "$AFTER"
out="$(run "$RED_W")"
has "a write AFTER it IS the dirty author"        "$out" "THIS session"
hasnt "and the pre-commit author stays excluded"  "$out" "$PEER"

echo
echo "== a SECONDARY span into a CLEAN file is not attributed =="
# A real rustc error carries more than one span. Observed live 2026-09-09: an E0061 in a
# peer's dirty file pointed its second span at src/librarian/session_registry.rs, which was
# CLEAN -- and attributing that file would have named whoever last touched it for a break
# they had no part in, at the exact moment someone wants a party to blame. That is the same
# hazard the author-side checker guards with an `is_primary` span filter.
#
# THIS TOOL AVOIDS IT INCIDENTALLY, WHICH IS WHY THIS CASE EXISTS. Stage 2 intersects the
# named paths with `git status` BEFORE authorship resolution runs, so a span into a clean
# file cannot reach stage 3 -- no span-role parsing involved. Nothing else in this suite
# asserts that ordering, so moving the intersection after resolution would reintroduce the
# defect silently and every other case would stay green. The load-bearing detail is that
# src/clean_bystander.rs is COMMITTED AND UNMODIFIED; dirty it and this case proves nothing.
# Raised by 5399543d after their own is_primary fix; the live observation is c9ab2c8d's.
echo "v1" > "$T/repo/src/clean_bystander.rs"
$G add src/clean_bystander.rs && $G commit -q -m "clean bystander"
out="$(run "$(printf 'error[E0061]: this function takes 1 argument but 0 were supplied\n  --> src/committed.rs:12:5\n  --> src/clean_bystander.rs:34:8\n')")"
has  "the DIRTY file in the same red is still attributed" "$out" "src/committed.rs"
hasnt "but the clean bystander is not named at all"       "$out" "src/clean_bystander.rs"

echo
echo "== a dead session is named but not offered as an address =="
rm -f "$SOCKDIR/live.sock"
out="$(run "$(printf 'error: bad\n  --> src/committed.rs:1:1\n')")"
has "the peer is still named"          "$out" "$PEER"
has "but marked unreachable"           "$out" "not live"
hasnt "and no address is offered"      "$out" "ask it:"

echo
echo "== both git columns count as dirty =="
# A peer mid-`git add` shows ` M` in one column and `M ` in the other within the same
# second. "Is this uncommitted right now" does not care which side of the index it is on.
$G add src/committed.rs
out="$(run "$(printf 'error: bad\n  --> src/committed.rs:1:1\n')")"
has "a STAGED change is still dirty" "$out" "src/committed.rs"

echo
echo "== --paths bypasses stdin for callers that already know =="
has "--paths reaches stage 3 directly" "$(run "" --paths src/committed.rs)" "src/committed.rs"

echo
echo "== a tooling failure is never rendered as an answer =="
# If the engine cannot import its provenance module it must say so as a TOOLING failure.
# Silently returning 0 here would be indistinguishable from "clean tree".
bad="$T/broken"; mkdir -p "$bad"
cp "$TOOL" "$bad/"                     # copied WITHOUT file-provenance.py beside it
out="$(printf 'x' | REPO_ROOT="$T/repo" python3 "$bad/attribute-red.py" --explain 2>&1)"
has "a missing dependency is a TOOLING failure" "$out" "NOT evidence"

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" = "0" ]
