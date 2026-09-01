#!/usr/bin/env bash
#
# Discrimination matrix for scripts/file-provenance.py.
#
# WHY THIS EXISTS
# ---------------
# The tool answers "is this working-tree file mine?" on a checkout shared by several
# Claude Code sessions — the missing channel in
# docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md,
# where three parties produced three wrong authorship answers in one evening.
#
# Two failure modes make a naive implementation WORSE than nothing, and every case
# below exists to discriminate against one of them:
#
#   1. MENTION-AS-AUTHORSHIP. A transcript names a path when it reads it, greps it,
#      discusses it, or prints an error about it. Measured 2026-09-01: ranking the
#      project's transcripts by raw path-mention count puts d4bd6ec9 (149) and
#      e3a0b567 (46) ABOVE the known owner c2a08c22 (62). A tool that counts
#      mentions returns a confident wrong name — the exact defect it is built to fix.
#
#   2. ABSENCE-AS-EXONERATION. "No record names this path" and "records name another
#      session" are different answers. Only the second is evidence. Rendering the
#      first as "not mine" is the failure the bug file documents: a true limitation
#      quietly substituting for an answer. UNKNOWN must survive as its own verdict.
#
# Fixtures are synthetic transcripts, so the suite tests the DISCRIMINATOR rather than
# whatever this machine's real logs happen to contain today.
set -u

SRC="$(cd "$(dirname "$0")/../scripts" && pwd)"
TOOL="$SRC/file-provenance.py"
PASS=0
FAIL=0

has() { # has <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        PASS=$((PASS + 1)); echo "  ok   $1"
    else
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- expected to find: $3"
        printf '       got: %s\n' "$2" | head -5
    fi
}

hasnt() { # hasnt <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- must NOT contain: $3"
        printf '       got: %s\n' "$2" | head -5
    else
        PASS=$((PASS + 1)); echo "  ok   $1"
    fi
}

ME="11111111-aaaa-bbbb-cccc-000000000001"
PEER="22222222-aaaa-bbbb-cccc-000000000002"

T="$(mktemp -d)"
trap 'rm -rf "$T"' EXIT
ROOTS="$T/profileA/projects/-repo:$T/profileB/projects/-repo"
mkdir -p "$T/profileA/projects/-repo" "$T/profileB/projects/-repo"

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

# bash_cmd <session-file> <command verbatim>  -- json-encodes, so no hand-escaping
bash_cmd() {
    python3 - "$1" "$2" <<'BASH_FIXTURE'
import json, sys
f, cmd = sys.argv[1], sys.argv[2]
rec = {"type": "assistant", "message": {"content": [
    {"type": "tool_use", "name": "Bash",
     "input": {"command": cmd, "description": "fixture"}}]}}
open(f, "a").write(json.dumps(rec) + "\n")
BASH_FIXTURE
}

run() { REPO_ROOT="$T/repo" FILE_PROVENANCE_ROOTS="$ROOTS" \
        CLAUDE_CODE_SESSION_ID="$ME" python3 "$TOOL" "$@" 2>&1; }

A="$T/profileA/projects/-repo/$ME.jsonl"
B="$T/profileB/projects/-repo/$PEER.jsonl"
mkdir -p "$T/repo"

echo
echo "== codescout write tools are authorship =="
tool_use "$A" mcp__codescout__edit_file '{"path":"src/mine.rs","old_string":"a","new_string":"b"}'
out="$(run src/mine.rs)"
has "edit_file by me -> MINE" "$out" "MINE"
hasnt "and not UNKNOWN" "$out" "UNKNOWN"

tool_use "$B" mcp__codescout__edit_code '{"path":"src/theirs.rs","symbol":"f","action":"replace","body":"x"}'
out="$(run src/theirs.rs)"
has "edit_code by peer -> names the peer" "$out" "$PEER"
hasnt "peer write is not MINE" "$out" "MINE"

tool_use "$A" mcp__codescout__create_file '{"path":"docs/new.md","content":"hi"}'
has "create_file counts" "$(run docs/new.md)" "MINE"
tool_use "$A" mcp__codescout__edit_markdown '{"path":"docs/md.md","heading":"## X","action":"replace","content":"y"}'
has "edit_markdown counts" "$(run docs/md.md)" "MINE"

echo
echo "== MENTION IS NOT AUTHORSHIP (defeats the naive count) =="
tool_use "$B" mcp__codescout__read_file '{"path":"src/mentioned.rs"}'
tool_use "$B" mcp__codescout__grep '{"pattern":"fn","path":"src/mentioned.rs"}'
tool_use "$B" Bash '{"command":"cat src/mentioned.rs | head","description":"look"}'
tool_use "$B" Bash '{"command":"grep -n foo src/mentioned.rs","description":"look"}'
out="$(run src/mentioned.rs)"
has "4 read-only mentions -> UNKNOWN" "$out" "UNKNOWN"
hasnt "read-only mentions do not name the mentioner" "$out" "$PEER"

echo
echo "== native writes are seen (codescout tool log cannot see these) =="
tool_use "$A" Edit '{"file_path":"src/native_edit.rs","old_string":"a","new_string":"b"}'
has "native Edit -> MINE" "$(run src/native_edit.rs)" "MINE"
tool_use "$A" Write '{"file_path":"src/native_write.rs","content":"x"}'
has "native Write -> MINE" "$(run src/native_write.rs)" "MINE"
tool_use "$B" MultiEdit '{"file_path":"src/multi.rs","edits":[]}'
has "native MultiEdit by peer -> peer" "$(run src/multi.rs)" "$PEER"

echo
echo "== Bash writes are seen (the measured blind spot) =="
tool_use "$A" Bash '{"command":"sed -i s/a/b/ docs/swept.md","description":"sweep"}'
has "sed -i -> MINE" "$(run docs/swept.md)" "MINE"
tool_use "$B" Bash '{"command":"cat > docs/heredoc.md <<EOF\\nx\\nEOF","description":"write"}'
has "cat > redirect -> peer" "$(run docs/heredoc.md)" "$PEER"
tool_use "$A" Bash '{"command":"echo hi >> docs/appended.md","description":"append"}'
has "append redirect -> MINE" "$(run docs/appended.md)" "MINE"
tool_use "$A" Bash '{"command":"rm -f docs/deleted.md","description":"delete"}'
has "rm -> MINE" "$(run docs/deleted.md)" "MINE"

echo
echo "== the absent case is its own verdict, and never exoneration =="
out="$(run src/nobody_touched_this.rs)"
has "no records anywhere -> UNKNOWN" "$out" "UNKNOWN"
# The verdict LINE is what a reader acts on. The body below it may (and must) mention
# the phrase in order to warn against it, so grepping the whole output cannot tell a
# rendering apart from a caution -- assert on the line that carries the verdict.
verdict="$(printf '%s' "$out" | head -1)"
hasnt "verdict line never renders as 'not mine'" "$verdict" "not mine"
hasnt "verdict line never says NOT_MINE" "$verdict" "NOT_MINE"
hasnt "verdict line does not claim a peer wrote it" "$verdict" "PEER"
has "UNKNOWN states the coverage limit" "$out" "no record"
has "and warns against the exoneration reading" "$out" "NOT about ownership"

echo
echo "== both authors -> both named, neither hidden =="
tool_use "$A" mcp__codescout__edit_file '{"path":"src/shared.rs","old_string":"a","new_string":"b"}'
tool_use "$B" Edit '{"file_path":"src/shared.rs","old_string":"c","new_string":"d"}'
out="$(run src/shared.rs)"
has "shared file is SHARED, not MINE" "$out" "SHARED"
has "shared file still names me" "$out" "THIS session"
has "shared file names the peer too" "$out" "$PEER"

echo
echo "== path matching does not bleed across neighbours =="
tool_use "$B" mcp__codescout__edit_file '{"path":"src/server.rs.bak","old_string":"a","new_string":"b"}'
out="$(run src/server.rs)"
has "prefix neighbour does not answer for src/server.rs" "$out" "UNKNOWN"
hasnt "and does not name its author" "$out" "$PEER"

# Written ONLY as an absolute path in a different checkout. A tool that matched on
# basename, or that stripped the leading directories, would call this MINE.
tool_use "$A" mcp__codescout__edit_file '{"path":"/abs/elsewhere/src/only_there.rs","old_string":"a","new_string":"b"}'
out="$(run src/only_there.rs)"
has "a write in ANOTHER tree does not answer for this one" "$out" "UNKNOWN"
hasnt "and is not attributed to me" "$out" "THIS session"

echo
echo "== the window: a file's LIFETIME authors are not its dirty-state author =="
# Measured 2026-09-01: querying a months-old real file returned FIFTEEN sessions,
# every one of which had legitimately written it at some point. The bug's question is
# never "who has ever touched this?" -- it is "this file is dirty NOW and reds my
# build, is that mine?". Writes older than the path's last commit are baked into HEAD
# and carry no information about the working-tree delta.
tool_use "$B" mcp__codescout__edit_file '{"path":"src/old.rs","old_string":"a","new_string":"b"}' "2026-01-01T00:00:00.000Z"
tool_use "$A" mcp__codescout__edit_file '{"path":"src/old.rs","old_string":"c","new_string":"d"}' "2026-09-01T12:00:00.000Z"

out="$(run --since 2026-06-01T00:00:00Z src/old.rs)"
has "in-window write is attributed" "$out" "MINE"
hasnt "pre-window write is NOT attributed" "$out" "$PEER"

out="$(run --since 2026-01-01T00:00:00Z src/old.rs)"
has "widening the window readmits the peer" "$out" "$PEER"
has "and still names me" "$out" "THIS session"

# A record with no timestamp cannot be placed in or out of the window. Dropping it
# silently would turn a substrate gap into a clean exoneration -- the same
# substitution the UNKNOWN verdict exists to refuse.
# NOTE the fixture path deliberately shares NO substring with the marker asserted
# below. Its first spelling was src/undated.rs, and "flagged as undated" then passed
# by matching the PATH ECHOED BACK in the verdict line -- green against a tool with no
# window support at all. A fixture name that can satisfy the assertion by itself makes
# the assertion unable to fail.
tool_use "$B" mcp__codescout__edit_file '{"path":"src/no_clock.rs","old_string":"a","new_string":"b"}'
out="$(run --since 2026-06-01T00:00:00Z src/no_clock.rs)"
has "a write with no timestamp is kept, not silently dropped" "$out" "$PEER"
has "and is flagged as unplaceable in time" "$out" "undated"

echo
echo "== relocating verbs write their DESTINATION, not their source =="
# Measured 2026-09-01 over this project's transcripts: 69 Bash calls (3.6%) carry a
# mutating verb the redirect/sed/rm patterns do not match, cp and mv chief among them.
# A miss degrades to UNKNOWN, which is the safe direction -- but UNKNOWN is the verdict
# this tool exists to avoid returning when it does not have to.
tool_use "$A" Bash '{"command":"cp src/cp_source.rs src/copied.rs","description":"copy in"}'
out="$(run --all src/copied.rs)"
has "cp destination is a write" "$out" "MINE"

# The SOURCE of a cp is read, not written. A tool attributing both would name an author
# for every file anyone ever copied FROM. NOTE the source is deliberately IN-REPO: its
# first spelling was /tmp/staged.rs, which normalize() discards as out-of-tree before any
# verb logic runs -- so the assertion passed against a tool with no cp support at all.
out="$(run --all src/cp_source.rs)"
has "cp source is not a write" "$out" "UNKNOWN"
hasnt "cp source names no author" "$out" "THIS session"

tool_use "$B" Bash '{"command":"mv docs/a.md docs/b.md","description":"rename"}'
has "mv destination is a write" "$(run --all docs/b.md)" "$PEER"
has "mv source is a write too (it disappears)" "$(run --all docs/a.md)" "$PEER"

# Destination name is unique to this case: docs/new.md (the obvious choice) is already
# written by $A in the codescout-write-tools section, and answered this assertion from
# that record while git-mv support did not exist.
tool_use "$A" Bash '{"command":"git mv docs/gitmv_src.md docs/gitmv_dest.md","description":"tracked rename"}'
has "git mv destination" "$(run --all docs/gitmv_dest.md)" "MINE"

tool_use "$B" Bash '{"command":"git checkout -- src/reverted.rs","description":"discard"}'
has "git checkout -- <path> rewrites the file" "$(run --all src/reverted.rs)" "$PEER"

# Discrimination: these verbs must not fire on their READ-ONLY relatives.
tool_use "$B" Bash '{"command":"git log --oneline -- src/readonly.rs","description":"history"}'
tool_use "$B" Bash '{"command":"git diff src/readonly.rs","description":"inspect"}'
out="$(run --all src/readonly.rs)"
has "git log/diff are not writes" "$out" "UNKNOWN"
hasnt "and name no author" "$out" "$PEER"

echo
echo "== artifact() writes are addressed by ID, and must still resolve to a path =="
# Found by DOGFOODING: three files this session had just edited reported UNKNOWN, because
# in this repo trackers and bug files are written almost exclusively through artifact(),
# whose input carries `id` and no path at all. The catalog is the id -> abs_path map.
CAT="$T/catalog.db"
python3 - "$CAT" "$T/repo" <<'CATALOG_SEED'
import sqlite3, sys, os
db, root = sys.argv[1], sys.argv[2]
c = sqlite3.connect(db)
c.execute("CREATE TABLE artifact (id TEXT PRIMARY KEY, abs_path TEXT)")
c.execute("INSERT INTO artifact VALUES (?,?)", ("deadbeef00000001", os.path.join(root, "docs/tracked.md")))
c.commit()
CATALOG_SEED
runc() { REPO_ROOT="$T/repo" FILE_PROVENANCE_ROOTS="$ROOTS" FILE_PROVENANCE_CATALOG="$CAT"          CLAUDE_CODE_SESSION_ID="$ME" python3 "$TOOL" "$@" 2>&1; }

tool_use "$A" mcp__codescout__artifact '{"action":"update","id":"deadbeef00000001","patch":{"body":"x"}}'
has "artifact(update) by id resolves through the catalog" "$(runc --all docs/tracked.md)" "MINE"

tool_use "$B" mcp__codescout__artifact '{"action":"append_entry","id":"deadbeef00000001","id_prefix":"F","entry":{}}'
has "artifact(append_entry) counts too" "$(runc --all docs/tracked.md)" "$PEER"

# Discrimination: the READ actions of the same tool must not make an author of a reader.
tool_use "$B" mcp__codescout__artifact '{"action":"get","id":"deadbeef00000002"}'
tool_use "$B" mcp__codescout__artifact '{"action":"find","kind":"bug"}'
out="$(runc --all docs/untouched_artifact.md)"
has "artifact(get)/(find) are not writes" "$out" "UNKNOWN"

# An id absent from the catalog cannot be resolved. It must not be guessed at, and must
# not crash the run.
tool_use "$A" mcp__codescout__artifact '{"action":"update","id":"ffffffffffffffff","patch":{"body":"x"}}'
out="$(runc --all docs/tracked.md)"
has "an unresolvable id does not break the run" "$out" "THIS session"
has "and the resolvable ones still answer" "$out" "SHARED"

echo
echo "== a python heredoc that writes a file is a write =="
# The other half of the same dogfooding finding: this session edited docs/PROBES.md with
# `python3 - <<SCRIPT ... open(p,'w').write(s)`, which is a repo write whose target appears
# only inside the script body.
tool_use "$A" Bash '{"command":"python3 - <<EOF\np=\"docs/via_python.md\"\nopen(p,\"w\").write(s)\nEOF","description":"patch"}'
has "open(...,'w') names its target" "$(runc --all docs/via_python.md)" "MINE"

# NOTE escaped DOUBLE quotes: single quotes here are eaten by the enclosing single-quoted
# shell word, and the fixture then records an UNQUOTED path that no matcher should accept.
bash_cmd "$B" 'python3 -c "from pathlib import Path; Path(\"docs/via_pathlib.md\").write_text(x)"'
has "Path(...).write_text names its target" "$(runc --all docs/via_pathlib.md)" "$PEER"

# Discrimination: reading a file in python is not writing it.
bash_cmd "$B" 'python3 -c "s=open(\"docs/only_read.md\").read()"'
out="$(runc --all docs/only_read.md)"
has "open() without a write mode is not a write" "$out" "UNKNOWN"

echo
echo "== the DEFAULT window derives from git, and is the load-bearing half =="
# Mutation-driven: every window case above passes --since explicitly, so `floor = None`
# in the DEFAULT branch killed zero tests -- the derivation that turns 15 lifetime authors
# into 1 was entirely unguarded. It needs a real repo, because it is `git log -1 --format=%cI`
# that supplies the floor.
git -C "$T/repo" init -q 2>/dev/null
git -C "$T/repo" config user.email t@t
git -C "$T/repo" config user.name t
mkdir -p "$T/repo/src"
echo "v1" > "$T/repo/src/committed.rs"
git -C "$T/repo" add src/committed.rs
git -C "$T/repo" commit -q -m "seed"
COMMIT_TS="$(git -C "$T/repo" log -1 --format=%cI -- src/committed.rs)"
BEFORE="$(python3 -c "
import sys,datetime
d=datetime.datetime.fromisoformat('$COMMIT_TS')
print((d-datetime.timedelta(days=1)).isoformat())")"
AFTER="$(python3 -c "
import sys,datetime
d=datetime.datetime.fromisoformat('$COMMIT_TS')
print((d+datetime.timedelta(minutes=5)).isoformat())")"

tool_use "$B" mcp__codescout__edit_file '{"path":"src/committed.rs","old_string":"a","new_string":"b"}' "$BEFORE"
out="$(run src/committed.rs)"
has "a write BEFORE the last commit is baked into HEAD -> UNKNOWN" "$out" "UNKNOWN"
hasnt "and its author is not named" "$out" "$PEER"
has "but the run says the writes exist" "$out" "predate the window"

tool_use "$A" mcp__codescout__edit_file '{"path":"src/committed.rs","old_string":"c","new_string":"d"}' "$AFTER"
out="$(run src/committed.rs)"
has "a write AFTER it is the dirty-state author" "$out" "MINE"
hasnt "and the pre-commit author stays excluded" "$out" "$PEER"

# --all must still reach past the derived floor, or the escape hatch is decorative.
has "--all reaches past the derived floor" "$(run --all src/committed.rs)" "$PEER"

# A path with NO commits has no floor to derive; every write must remain visible.
tool_use "$B" mcp__codescout__edit_file '{"path":"src/never_committed.rs","old_string":"a","new_string":"b"}' "$BEFORE"
has "an uncommitted path keeps its full history" "$(run src/never_committed.rs)" "$PEER"

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" = "0" ]
