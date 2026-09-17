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
tool_use "$A" mcp__codescout__edit_markdown '{"path":"docs/md.md","heading":"## X","action":"replace","content":"y"}'  # legacy: retired tool name, still present in older transcripts
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

# ... and they must not fire on a FILENAME that merely CONTAINS one. src/librarian/tools/mv.rs
# is a real file here, and a read-only command naming it is by far the commonest way it is
# ever mentioned. Observed 2026-09-15: the attribution hook named a peer as the author of
# that file off two `git diff --stat` calls, while it held one hunk written entirely by the
# reader -- and every command run to ask "did I write mv.rs?" added another such record, so
# the wrong answer gained confidence with investigation.
#
# TWO commands in one body is load-bearing, not padding. ONE read yields the harmless
# fragment `.rs`, which resolves to no file; it takes a SECOND `--` for the first verb's
# operand tail to run past the line end and honour it. A single-command fixture passes
# against the unfixed tool and would have guarded nothing.
#
# Both assertions below are ABSENCE assertions, monotone under removal -- deleting relocator
# support entirely would satisfy them. What makes them a discrimination is the positive mv/cp
# block ~20 lines above, which runs against the same $B session: if you remove that, these
# stop being evidence of anything.
bash_cmd "$B" 'cargo fmt --check -- src/mv.rs
git diff --stat -- src/mv.rs'
out="$(run --all src/mv.rs)"
has "a verb inside a FILENAME is not a relocation" "$out" "UNKNOWN"
hasnt "and the mentioned file names no author" "$out" "$PEER"

# The same tail overrun aimed elsewhere: the lifted path need not be the one that triggered
# the match. Neither command here can write anything, and the victim is a THIRD file named
# only by the second of them.
bash_cmd "$B" 'cargo fmt --check -- src/mv.rs
git diff --stat -- src/lifted.rs'
out="$(run --all src/lifted.rs)"
has "a later command's operand is not this one's destination" "$out" "UNKNOWN"
hasnt "and the lifted path names no author" "$out" "$PEER"

# The two cases above guard the CONJUNCTION of the two bounds in RELOCATORS and neither
# bound alone: measured 2026-09-15, restoring `\b` with the `\n` bound in place SURVIVED
# them, and so did dropping the `\n` bound with `(?=\s)` in place. Each needs its own
# case, because each is its own site.
#
# Site 1, the `(?=\s)` lookahead: one line, one command, and a SECOND path after the
# verb-shaped filename, so there is no line end for the other bound to rescue. `\b` reads
# `mv.rs` as a relocation and hands back `.rs` plus the innocent sibling.
bash_cmd "$B" 'git diff --stat -- src/mv.rs src/sibling.rs'
out="$(run --all src/sibling.rs)"
has "a path beside a verb-shaped filename is not its destination" "$out" "UNKNOWN"
hasnt "and the sibling names no author" "$out" "$PEER"

# ...and that pair STOPPED isolating `(?=\s)` the moment command-position anchoring was
# added below it. Both put the verb-shaped filename after a `/`, which position already
# rejects, so dropping the whitespace bound SURVIVED them -- measured 2026-09-15, the same
# day the pair was written for exactly that purpose. **Adding a bound can silently un-guard
# a site an older bound still owns**, and nothing about the suite going green says so.
#
# This case is what isolates the whitespace bound now: the verb-shaped token OPENS a line,
# so command-position is satisfied and only `(?=\s)` can refuse it.
bash_cmd "$B" "cat <<'EOF'
mv.rs src/regenerated.rs
EOF"
out="$(run --all src/regenerated.rs)"
has "a line OPENING with a verb-shaped filename is not a relocation" "$out" "UNKNOWN"
hasnt "and the path beside it names no author" "$out" "$PEER"

# CONTROL: a real relocation opening a line still resolves. Without it the assertion above
# is satisfied by refusing every line-initial verb, which would delete the feature.
bash_cmd "$B" "cat src/whatever.rs
mv src/lineinit_from.rs src/lineinit_to.rs"
has "and a real relocation opening a line still resolves" "$(run --all src/lineinit_to.rs)" "$PEER"

# Site 2, the `\n` in the negated class: a REAL relocation, so the lookahead is satisfied
# and cannot help. Unbounded, the tail runs into the next command and _operands() honours
# ITS `--`. Note both directions are asserted -- the unbounded form does not merely ADD the
# victim, it REPLACES the mv's own operands with it, so the second assertion fails too and
# a false negative cannot hide behind a passing false positive.
bash_cmd "$B" 'mv src/relocated_from.txt src/relocated_to.txt
git diff --stat -- src/victim.rs'
out="$(run --all src/victim.rs)"
has "a real mv does not annex the next command's operand" "$out" "UNKNOWN"
hasnt "and the next command's path names no author" "$out" "$PEER"
has "and the mv's own destination is still attributed" "$(run --all src/relocated_to.txt)" "$PEER"

# A verb bounded by whitespace on both sides is still not necessarily a COMMAND. Two ways
# it is not, and the whitespace bound above cannot see either, because in both the
# whitespace is real:
#
#   1. it is a SUBCOMMAND of another program -- `cargo install`, `apt install`;
#   2. it is inside a HEREDOC BODY, text a shell passes through as data.
#
# Both must be command-POSITION failures rather than token-shape ones, so each case below
# is paired with a control that keeps the real verb working.
bash_cmd "$B" 'cargo install ripgrep'
out="$(run --all ripgrep)"
has "a subcommand of another program is not a relocation" "$out" "UNKNOWN"
hasnt "and the package name names no author" "$out" "$PEER"

# CONTROL for the pair. Without it, deleting `install` from the alternation passes the two
# assertions above -- suppression reading as discrimination.
bash_cmd "$B" 'install -m 755 build/staged_tool bin/installed_tool'
has "a real install still writes its destination" "$(run --all bin/installed_tool)" "$PEER"
out="$(run --all build/staged_tool)"
has "and its source is read, not written" "$out" "UNKNOWN"

# The heredoc. `<<'PY'` exists PRECISELY to mean "the following is data, not syntax", and
# every shell scanner in this process has had to learn that separately. The quoted path here
# is the exact shape this repo's own mutation probes write, which is how it was found: the
# probes verifying the previous fix each recorded a relocation of files nobody touched.
bash_cmd "$B" "python3 - <<'PY'
cases = [(\"a real relocation\", \"mv src/heredoc_alpha.rs src/heredoc_beta.rs\")]
PY"
# NOTE the query is the FIRST operand. The second comes back as `src/heredoc_beta.rs")]`,
# with the Python syntax still attached, so it resolves to no file and asserting on it
# passes against the unfixed tool. That trailing garbage is also the sharpest evidence the
# matched text was never a command.
out="$(run --all src/heredoc_alpha.rs)"
has "a relocation inside a heredoc BODY is data, not a command" "$out" "UNKNOWN"
hasnt "and the quoted path names no author" "$out" "$PEER"

# CONTROL: the command CARRYING the heredoc is still a command, and its own verbs resolve.
# Without this, a scanner that gave up on any input containing `<<` would pass the pair above.
bash_cmd "$B" "mv src/wrapper_from.rs src/wrapper_to.rs && python3 - <<'PY'
print('nothing here')
PY"
has "and the enclosing command's own relocation still resolves" "$(run --all src/wrapper_to.rs)" "$PEER"

# KNOWN LOSS, asserted so that widening it is a deliberate edit and not an accident.
# Anchoring on command position means a wrapper word -- sudo, time, env, xargs -- hides the
# verb behind it, exactly as `cargo` does, because nothing distinguishes a wrapper from a
# program with subcommands without a list of one or the other. The miss degrades to UNKNOWN,
# which is this tool's safe direction; a wrong name is what it exists to avoid. If you make
# wrappers work, change this assertion on purpose.
bash_cmd "$B" 'sudo mv src/wrapped_from.rs src/wrapped_to.rs'
has "a verb behind a wrapper word is missed, in the safe direction" "$(run --all src/wrapped_to.rs)" "UNKNOWN"

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

tool_use "$A" mcp__codescout__artifact '{"action":"update","id":"deadbeef00000001","patch":{"body":"x"}}'  # legacy: retired tool name, still present in older transcripts
has "artifact(update) by id resolves through the catalog" "$(runc --all docs/tracked.md)" "MINE"

tool_use "$B" mcp__codescout__artifact '{"action":"append_entry","id":"deadbeef00000001","id_prefix":"F","entry":{}}'  # legacy: retired tool name, still present in older transcripts
has "artifact(append_entry) counts too" "$(runc --all docs/tracked.md)" "$PEER"

# Discrimination: the READ actions of the same tool must not make an author of a reader.
tool_use "$B" mcp__codescout__artifact '{"action":"get","id":"deadbeef00000002"}'  # legacy: retired tool name, still present in older transcripts
tool_use "$B" mcp__codescout__artifact '{"action":"find","kind":"bug"}'  # legacy: retired tool name, still present in older transcripts
out="$(runc --all docs/untouched_artifact.md)"
has "artifact(get)/(find) are not writes" "$out" "UNKNOWN"

# An id absent from the catalog cannot be resolved. It must not be guessed at, and must
# not crash the run.
tool_use "$A" mcp__codescout__artifact '{"action":"update","id":"ffffffffffffffff","patch":{"body":"x"}}'  # legacy: retired tool name, still present in older transcripts
out="$(runc --all docs/tracked.md)"
has "an unresolvable id does not break the run" "$out" "THIS session"
has "and the resolvable ones still answer" "$out" "SHARED"

echo
echo "== doc() is that SAME tool renamed, and the rename made every librarian write invisible =="
# The block above kept passing across a rename that broke the thing it guards. The tool was
# `artifact` until ceb5b57a (2026-09-02) and is `doc` now; the fixture re-types the same dead
# string the matcher holds, so both halves agreed with each other and neither with the running
# server. Measured 2026-09-08: 501 write-action doc() calls in this project invisible for six
# days, while docs/PROBES.md said the route was handled. UNKNOWN is the failure mode, and the
# script's own docstring forbids reading it as "not mine" -- so the blindness presented as the
# tool being appropriately humble.
#
# LOAD-BEARING: these cases keep their OWN artifact id and path. Reusing docs/tracked.md would
# inherit the writes above, making every verdict here SHARED and unable to distinguish MINE.
# LOAD-BEARING: do NOT fold these into the artifact block. The legacy name must keep its own
# cases -- old transcripts still hold it -- and these must red if `doc` alone is dropped.
python3 - "$CAT" "$T/repo" <<'CATALOG_SEED_DOC'
import sqlite3, sys, os
db, root = sys.argv[1], sys.argv[2]
c = sqlite3.connect(db)
for aid, rel in (("deadbeef00000003", "docs/doc_tracked.md"),
                 ("deadbeef00000004", "docs/doc_augmented.md"),
                 ("deadbeef00000005", "docs/doc_read_only.md")):
    c.execute("INSERT INTO artifact VALUES (?,?)", (aid, os.path.join(root, rel)))
c.commit()
CATALOG_SEED_DOC

tool_use "$A" mcp__codescout__doc '{"action":"update","id":"deadbeef00000003","patch":{"body":"x"}}'
has "doc(update) by id resolves through the catalog" "$(runc --all docs/doc_tracked.md)" "MINE"

tool_use "$B" mcp__codescout__doc '{"action":"append_entry","id":"deadbeef00000003","id_prefix":"F","entry":{}}'
has "doc(append_entry) counts too" "$(runc --all docs/doc_tracked.md)" "$PEER"

# augment was a SEPARATE TOOL (artifact_augment), carried in write_targets by a
# `name.endswith("_augment")` test. As a doc() ACTION that test never fires, so the action set
# has to name it -- which is why repairing the tool name alone leaves this case red.
#
# LOAD-BEARING: its OWN artifact id and path, and it must stay the only write to that path.
# Asserted against docs/doc_tracked.md this was VACUOUS -- B's append_entry above already
# satisfies "$PEER", so dropping "augment" from the set killed zero assertions (measured, it
# survived the mutation). An aggregate cannot answer a question about one member.
tool_use "$B" mcp__codescout__doc '{"action":"augment","id":"deadbeef00000004","augment":{"prompt":"p"}}'
has "doc(augment) counts -- the arm _augment used to carry" "$(runc --all docs/doc_augmented.md)" "$PEER"

# create carries rel_path rather than an id: a new artifact has no catalog row to resolve yet.
tool_use "$A" mcp__codescout__doc '{"action":"create","rel_path":"docs/created_by_doc.md","kind":"bug","title":"t"}'
has "doc(create) attributes its rel_path" "$(runc --all docs/created_by_doc.md)" "MINE"

# LOAD-BEARING, and found by dogfooding this very repair against a peer's staged archive
# move: `move` RE-KEYS the artifact (id = sha256(abs_path)), so by the time anyone reads the
# transcript the recorded `id` names a catalog row that no longer exists. The lookup returns
# None and the call yields nothing at all. `new_rel_path` is the only key in that input which
# still resolves -- and archiving is a bug file's NORMAL end state, so this is not an edge
# case but the librarian write most likely to be asked about.
#
# The id here is deliberately ABSENT from the catalog. Seeding it would test the pre-move
# world, which is the one state this call can never be observed in.
tool_use "$B" mcp__codescout__doc '{"action":"move","id":"deadbeef00000009","new_rel_path":"docs/archive/moved_by_doc.md"}'
has "doc(move) attributes new_rel_path after the re-key" "$(runc --all docs/archive/moved_by_doc.md)" "$PEER"

# THE CONTROL, and the only one here that can red under the obvious wrong fix: counting EVERY
# doc() call satisfies all five assertions above while making a reader an author. find/get are
# 315 of this project's 816 doc calls, so that mutation would mis-attribute more than it fixed.
#
# LOAD-BEARING: the id must be one the catalog RESOLVES. Written against an absent id this was
# vacuous -- the lookup returned None under the mutation too, so the case passed for a reason
# having nothing to do with get/find being excluded (measured: `if True:` survived it). A
# control that cannot reach the failing value is not a control.
tool_use "$B" mcp__codescout__doc '{"action":"get","id":"deadbeef00000005"}'
tool_use "$B" mcp__codescout__doc '{"action":"find","kind":"bug"}'
has "doc(get)/(find) are not writes" "$(runc --all docs/doc_read_only.md)" "UNKNOWN"
hasnt "and name no author" "$(runc --all docs/doc_read_only.md)" "$PEER"

echo
echo "== run_command is codescout's own shell, and writes through it are writes =="
# 2b9cbd34630cf340: the shell branch was keyed on the harness's `Bash` alone, so every
# redirect, `sed -i` and `rm` issued through codescout's own shell was invisible to the
# instrument built to attribute exactly those. Same function and same class as the doc() miss,
# which is why both land together. Both tools carry the command under `command`.
tool_use "$A" mcp__codescout__run_command '{"command":"echo hi > docs/via_run_command.md"}'
has "run_command redirect names its target" "$(runc --all docs/via_run_command.md)" "MINE"

tool_use "$B" mcp__codescout__run_command '{"command":"sed -i s/a/b/ docs/sed_by_run_command.md"}'
has "run_command sed -i names its target" "$(runc --all docs/sed_by_run_command.md)" "$PEER"

# THE CONTROL. run_command is overwhelmingly a READ tool here, so a branch that attributed
# every path it mentions would make an author of every grep -- the same over-attribution the
# script's docstring rejects for mention-counting, arriving through a different door.
tool_use "$B" mcp__codescout__run_command '{"command":"grep -n fn docs/read_by_run_command.md"}'
has "a read-only run_command is not a write" "$(runc --all docs/read_by_run_command.md)" "UNKNOWN"

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

# -- a variable ALREADY holding a Path, which is how snippets are actually written -----
# The two patterns above both require the name to be WRAPPED at the call site
# (`Path(p).write_text`), so the ordinary multi-line form was invisible. Measured
# 2026-09-08: ad379a7c wrote src/peer/server.rs exactly this way, the tool returned
# UNKNOWN, and two other sessions each spent a round trip attributing one file by hand.
bash_cmd "$B" 'python3 - <<PY
from pathlib import Path
F = Path("docs/via_path_var.md")
txt = F.read_text()
F.write_text(txt)
PY'
has "a bare var.write_text names its target" "$(runc --all docs/via_path_var.md)" "$PEER"

# The other narrowing, independent of the first: the resolver bound only `var = "literal"`,
# so a Path()-wrapped binding reaching open() resolved to nothing.
bash_cmd "$B" 'python3 - <<PY
from pathlib import Path
p = Path("docs/via_path_open.md")
open(p, "w").write(s)
PY'
has "open(var) resolves a Path()-bound name" "$(runc --all docs/via_path_open.md)" "$PEER"

bash_cmd "$B" 'python3 -c "from pathlib import Path; F=Path(\"docs/via_bytes.bin\"); F.write_bytes(b)"'
has "write_bytes is the same construct" "$(runc --all docs/via_bytes.bin)" "$PEER"

# The MODULE-QUALIFIED form. `import pathlib` + `pathlib.Path(...)` is as common as the
# `from pathlib import Path` form above, and narrowing the resolver to bare `Path(` killed
# nothing until this case existed.
bash_cmd "$B" 'python3 - <<PY
import pathlib
F = pathlib.Path("docs/via_qualified.md")
F.write_text(x)
PY'
has "pathlib.Path(...) binds as well as Path(...)" \
    "$(runc --all docs/via_qualified.md)" "$PEER"

# THE REGRESSION GUARD FOR THAT WIDENING, and the reason it is not optional.
# Binding is not writing. Widening the ASSIGNMENT side -- "any path bound in the snippet"
# -- is the tempting fix and would make every path a snippet merely READS into an author:
# the mention-as-authorship failure this whole tool exists to refuse, which returns a
# confident WRONG name rather than a vague one. The discriminator must stay the CALL.
bash_cmd "$B" 'python3 - <<PY
from pathlib import Path
cfg = Path("docs/only_read_pathvar.md")
d = cfg.read_text()
print(d)
PY'
has "a Path-valued var that is only READ is not a write" \
    "$(runc --all docs/only_read_pathvar.md)" "UNKNOWN"

# The mode check has to guard the VARIABLE path too. Found by mutation, not by reading:
# the read-only case above addresses open() with a LITERAL, so deleting `[wax]` from the
# variable pattern -- making every `open(p)` a write -- killed nothing and the suite
# called it correct. One mutation answers a question about one SITE; a law implemented at
# two call sites needs two.
bash_cmd "$B" 'python3 - <<PY
p = "docs/only_read_var.md"
s = open(p).read()
PY'
has "open(var) without a write mode is not a write" \
    "$(runc --all docs/only_read_var.md)" "UNKNOWN"

# ...and an EXPLICIT read mode, because the case above passes no mode at all, so widening
# the [wax] class to [waxr] was invisible to it. The mode class is the discriminator;
# guard the character, not just the argument's absence.
bash_cmd "$B" 'python3 - <<PY
p = "docs/explicit_read_var.md"
s = open(p, "r").read()
PY'
has "open(var, r) is a read, not a write" \
    "$(runc --all docs/explicit_read_var.md)" "UNKNOWN"

# An f-string target stays missed ON PURPOSE -- the path is not in the snippet, so no
# static reading recovers it. Annotated as INERT for coverage: this case proves the
# residual is bounded and named, not that the matcher handles the construct.
bash_cmd "$B" 'python3 -c "from pathlib import Path; Path(f\"docs/{n}.md\").write_text(x)"'
has "an f-string target remains an honest UNKNOWN" \
    "$(runc --all docs/via_fstring.md)" "UNKNOWN"

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
# MINE is the verdict a reader acts on to license a commit -- so it is the one that most
# needs the same "writes exist outside the window" caveat UNKNOWN already prints. Before
# this case existed the caveat was scoped to the empty-who_set branch only, so a hidden
# peer write on a MINE-verdict path was invisible at exactly the point a reader would act.
has "MINE also surfaces a write the window hid" "$out" "predate the window"

# --all must still reach past the derived floor, or the escape hatch is decorative.
has "--all reaches past the derived floor" "$(run --all src/committed.rs)" "$PEER"

# A path with NO commits has no floor to derive; every write must remain visible.
tool_use "$B" mcp__codescout__edit_file '{"path":"src/never_committed.rs","old_string":"a","new_string":"b"}' "$BEFORE"
has "an uncommitted path keeps its full history" "$(run src/never_committed.rs)" "$PEER"

echo
echo "== a sessionId is an ADDRESS, not just evidence -- the registry join =="
#
# The bug: the tool named a session and stopped, so every answer cost a round trip
# through a peer's turn to become actionable -- and a reader could not tell an
# exited session from a reachable one without a separate manual walk.
# docs/issues/2026-09-01-claude-md-denies-a-pid-to-session-join-the-registry-carries.md
#
# LIVENESS is the load-bearing half. A registry file OUTLIVES its session, so
# "a row exists" is not "you can ask it": routing to a dead session ENOENTs, and
# that error is byte-identical to a cross-profile name refusal, which the peer skill
# tells you to answer by switching address form. So the reader retries instead of
# re-attributing. Cases B/C/G exist to keep that discrimination.

REG_A="$T/regA/sessions"
REG_B="$T/regB/sessions"
REG="$REG_A:$REG_B"
SOCKDIR="$T/socks"
mkdir -p "$REG_A" "$REG_B" "$SOCKDIR"

# A pid that is provably absent, derived rather than assumed -- a hardcoded "dead"
# pid silently becomes a LIVE one the day the kernel reuses it, and the case would
# then pass for the wrong reason instead of failing.
DEADPID="$(python3 -c "
import os
p = 4000000
while p > 300 and os.path.exists('/proc/%d' % p):
    p -= 1
print(p)")"

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

runr() { REPO_ROOT="$T/repo" FILE_PROVENANCE_ROOTS="$ROOTS" \
    FILE_PROVENANCE_REGISTRY_ROOTS="$REG" \
    CLAUDE_CODE_SESSION_ID="$ME" python3 "$TOOL" "$@" 2>&1; }

# A never-committed path, so the derived floor cannot hide the write.
tool_use "$B" mcp__codescout__edit_file \
    '{"path":"src/registry_probe.rs","old_string":"a","new_string":"b"}'

# -- A. a LIVE peer resolves to an address the reader can act on -----------------
touch "$SOCKDIR/live.sock"
registry_row "$REG_A" "$$" "$PEER" "peer-name-42" "$SOCKDIR/live.sock" busy
out="$(runr src/registry_probe.rs)"
has "a live session is marked LIVE"            "$out" "[LIVE]"
has "and names the session"                    "$out" "peer-name-42"
has "and its status"                           "$out" "[busy]"
has "and its pid"                              "$out" "pid $$"
has "and the PROFILE it lives under"           "$out" "profile regA"
has "and the uds: form, the only cross-profile address" \
    "$out" "ask it: SendMessage to=\"uds:$SOCKDIR/live.sock\""
hasnt "a live session is not called unreachable" "$out" "not live"

# -- B. a registry row whose SOCKET is gone is not reachable ---------------------
# The row persists after the session exits; only the socket disappears.
rm -f "$SOCKDIR/live.sock"
out="$(runr src/registry_probe.rs)"
has "a row without its socket is not live"     "$out" "not live — cannot be asked"
hasnt "and is not offered as an address"       "$out" "ask it:"
hasnt "and its stale name is not printed"      "$out" "peer-name-42"

# -- C. a registry row whose PID is dead is not reachable ------------------------
# Socket present this time, so this case fails if only the socket half is checked.
touch "$SOCKDIR/live.sock"
rm -f "$REG_A"/*.json
registry_row "$REG_A" "$DEADPID" "$PEER" "ghost-name" "$SOCKDIR/live.sock"
out="$(runr src/registry_probe.rs)"
has "a row with a dead pid is not live"        "$out" "not live — cannot be asked"
hasnt "and its name is not offered"            "$out" "ghost-name"

# -- D. no registry row at all -> still an answer, still not an address ----------
rm -f "$REG_A"/*.json "$REG_B"/*.json
out="$(runr src/registry_probe.rs)"
has "an unregistered sid is not live"          "$out" "not live — cannot be asked"
has "but is STILL named -- attribution survives" "$out" "$PEER"
has "and the verdict is unchanged"             "$out" "PEER"

# -- E. the OTHER profile is reached ---------------------------------------------
# The whole point: one profile's registry is what ListAgents reads and it reports
# as complete. A join that only searched the first root would pass every case
# above and still be the subset bug it exists to fix.
registry_row "$REG_B" "$$" "$PEER" "other-profile-peer" "$SOCKDIR/live.sock"
out="$(runr src/registry_probe.rs)"
has "a peer in the SECOND profile is found"    "$out" "other-profile-peer"
has "and is labelled with that profile"        "$out" "profile regB"

# -- F. the footer carries unit, scope and instant -------------------------------
has "the footer counts live against named"     "$out" "1 of 1 named session(s) live at"
has "and names the scope it searched"          "$out" "across 2 profile(s)"
has "and says which field is durable"          "$out" "the sessionId does not"

# -- G. one sid, two live rows -> say so rather than pick ------------------------
# IC-6's no-disambiguator half: silently addressing one of two is a coin flip.
registry_row "$REG_A" "$$" "$PEER" "twin-in-A" "$SOCKDIR/live.sock"
out="$(runr src/registry_probe.rs)"
has "a duplicated sid is reported, not resolved" "$out" "2 live rows carry this sessionId"
has "and both are named -- A"                  "$out" "twin-in-A"
has "and both are named -- B"                  "$out" "other-profile-peer"

# -- H. UNKNOWN is untouched by all of this --------------------------------------
# The join must not create an address where there was no attribution; UNKNOWN's
# coverage caveat is the one verdict that must never acquire a session line.
out="$(runr src/no_such_write.rs)"
has "UNKNOWN survives the join"                "$out" "UNKNOWN"
hasnt "and gains no liveness claim"            "$out" "not live"
hasnt "and gains no footer"                    "$out" "named session(s) live at"

# -- I. a row that cannot be READ is not a session that is ABSENT ----------------
# Measured 2026-09-17 on a 100%-full btrfs: a live session's own registry row was
# truncated to 0 bytes by an ENOSPC rewrite. pid alive, socket open, mid-tool-call --
# and `json.loads("")` raises ValueError, so the row was skipped and the session read
# as exited by this tool, by ListAgents, and by a hand-rolled socket walk alike.
#
# The DIRECTION is what makes it dangerous. The reader concludes "that session has
# exited, its resources are reclaimable" -- and disk exhaustion is exactly the
# condition under which someone goes looking for reclaimable per-session state, so
# the instrument degrades precisely when it is being consulted.
# docs/issues/archive/2026-09-17-a-full-disk-truncates-a-live-sessions-registry-row-so-provenance-reports-it-dead.md
sleep 300 & TRUNCPID=$!
# 0 bytes is not a contrived fixture: it is exactly what a truncating rewrite under
# ENOSPC leaves behind, and it is the state the real incident was observed in.
: > "$REG_B/$TRUNCPID.json"
out="$(runr src/registry_probe.rs)"
has "an unreadable row for a LIVE pid is reported"      "$out" "unreadable registry row"
# One needle, not two: asserted separately, `regB` is satisfied by the profile label
# on an unrelated live row elsewhere in the same output -- it passed in the pre-fix
# red state, which is the tell for an assertion that can never discriminate.
has "and names the pid WITH its profile, so it resolves by hand" "$out" "pid $TRUNCPID (regB)"
has "and says which way the error runs"                 "$out" "LOWER BOUND"

# The discriminator, and the reason the case above is not satisfied by accident: a
# corrupt row for a DEAD pid is ordinary garbage, not a hidden session. Without this,
# the assertion passes for any unparseable file and every stale row on disk shouts.
: > "$REG_B/$DEADPID.json"
out="$(runr src/registry_probe.rs)"
hasnt "a corrupt row for a DEAD pid stays silent"       "$out" "pid $DEADPID"
has "while the live one still speaks"                   "$out" "pid $TRUNCPID (regB)"

# The case that matters most. UNKNOWN plus a live-but-unidentifiable session is
# exactly when "nobody owns this file" is the wrong conclusion to draw -- so this
# warning must reach the verdict that carries no session line at all.
out="$(runr src/no_such_write.rs)"
has "UNKNOWN carries the warning too"                   "$out" "unreadable registry row"

kill "$TRUNCPID" 2>/dev/null
rm -f "$REG_B/$TRUNCPID.json" "$REG_B/$DEADPID.json"

echo
echo "== profile DISCOVERY: the default path every case above bypasses =="
#
# Every case above injects FILE_PROVENANCE_*_ROOTS to stay hermetic -- which also
# makes the production default unobservable to all of them. Measured 2026-09-08:
# reverting discovery to a hardcoded three-profile list, AND making it return
# nothing at all, both left 94/94 green. That is not a thin sample it could be
# widened out of; the refuting outcome leaves no artifact in a recording that
# filters the default path out. Only a case that omits the override can see it.
#
# Discovery matters because the profile set is per-machine: this box has 7
# .claude* directories, 5 of them carrying sessions/, against the 3 a fixed list
# named -- so a hardcoded default is the per-profile-subset hazard this whole tool
# exists to defeat, reappearing inside the instrument, reporting as complete.

FAKEHOME="$T/fakehome"
mkdir -p "$FAKEHOME/.claude/sessions" \
         "$FAKEHOME/.claude-sdd/sessions" \
         "$FAKEHOME/.claude-brandnew/sessions" \
         "$FAKEHOME/.claude-noreg" \
         "$FAKEHOME/.claudeish/sessions" \
         "$FAKEHOME/notclaude/sessions" \
         "$FAKEHOME/.claude-brandnew/projects/-x-y"

# No FILE_PROVENANCE_REGISTRY_ROOTS: this is the branch real runs take.
disco="$(env -u FILE_PROVENANCE_REGISTRY_ROOTS HOME="$FAKEHOME" python3 -c "
import importlib.util
s = importlib.util.spec_from_file_location('fp', '$TOOL')
m = importlib.util.module_from_spec(s); s.loader.exec_module(m)
print('\n'.join(str(p) for p in m.registry_roots()))")"
has "discovery finds the default profile"        "$disco" "/.claude/sessions"
has "and a suffixed profile"                     "$disco" "/.claude-sdd/sessions"
has "and one on NO hardcoded list -- the point"  "$disco" "/.claude-brandnew/sessions"
hasnt "but not a profile lacking sessions/"      "$disco" ".claude-noreg"
# .claudeish shares the ".claude" prefix without the separator. A startswith(".claude")
# test admits it -- the same prefix-swallow that made a git-verb regex refuse
# read-only plumbing. The separator is load-bearing; do not relax it to a prefix.
hasnt "and not a lookalike without the dash"     "$disco" ".claudeish"
hasnt "and nothing outside the .claude* family"  "$disco" "notclaude"

# transcript_roots must share that discovery, or fixing one leaves its twin.
disco_t="$(env -u FILE_PROVENANCE_ROOTS HOME="$FAKEHOME" python3 -c "
import importlib.util
from pathlib import Path
s = importlib.util.spec_from_file_location('fp', '$TOOL')
m = importlib.util.module_from_spec(s); s.loader.exec_module(m)
print('\n'.join(str(p) for p in m.transcript_roots(Path('/x/y'))))")"
has "transcripts discover the same way"          "$disco_t" "/.claude-brandnew/projects/-x-y"
hasnt "and skip profiles without this project"   "$disco_t" ".claude-sdd/projects"

echo "== a SUBAGENT write is authorship, and its record lives one directory DOWN =="
# Claude Code 2.1.x dispatches via `Agent` and writes the subagent's records to
#     <project-dir>/<PARENT-session-id>/subagents/agent-<id>.jsonl
# and NOT into the parent's own <session-id>.jsonl. `scan()` globbed "*.jsonl"
# NON-RECURSIVELY, so every one of those files sat outside its window.
#
# Why this went two corrections without being found: the "zero isSidechain records"
# measurement that concluded first the substrate and then the 2.1.x VERSION could not
# supply subagent records was itself taken through that same non-recursive glob, so it
# could only ever return zero. A windowed instrument's zero is scoped to its window.
# Re-derived 2026-09-12 OUTSIDE the window, on this machine, for this checkout across 3
# profiles: 756 subagent transcript files, 755 carrying isSidechain:true, 192,797 such
# records -- the newest written that same day by Claude Code 2.1.267, which is INSIDE the
# exact version range the old comment cited as emitting none.
sub_tool_use() { # sub_tool_use <file> <tool> <input-json> [parent-sid] -- omit sid to drop the field
    python3 - "$1" "$2" "$3" "${4:-}" <<'PY'
import json, sys
f, name, inp, sid = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
# Mirrors a real 2.1.x subagent record, field for field, as measured on disk.
rec = {"parentUuid": None, "isSidechain": True, "agentId": "a0123456789abcdef",
       "type": "assistant", "timestamp": "2026-09-12T08:37:12.000Z",
       "message": {"content": [
           {"type": "tool_use", "name": name, "input": json.loads(inp)}]}}
if sid:
    rec["sessionId"] = sid
open(f, "a").write(json.dumps(rec) + "\n")
PY
}

SUBA="$T/profileA/projects/-repo/$ME/subagents/agent-a0123456789abcdef.jsonl"
mkdir -p "$(dirname "$SUBA")"
sub_tool_use "$SUBA" mcp__codescout__create_file '{"path":"src/by_subagent.rs","content":"x"}' "$ME"
out=$(run src/by_subagent.rs)
has   "a subagent write attributes to its PARENT session" "$out" "MINE"
hasnt "and is no longer the coverage verdict"             "$out" "UNKNOWN"

# CONTROL. Without it, "glob recursively and take the credit" passes the assertion above:
# a fix that attributed every subagent record to the running session would be green there
# and would hand this session write authority over a peer's files -- which is what
# fmt-mine.sh then acts on.
SUBB="$T/profileB/projects/-repo/$PEER/subagents/agent-b0123456789abcdef.jsonl"
mkdir -p "$(dirname "$SUBB")"
sub_tool_use "$SUBB" mcp__codescout__create_file '{"path":"src/by_peer_subagent.rs","content":"x"}' "$PEER"
out=$(run src/by_peer_subagent.rs)
has   "a PEER's subagent write attributes to the PEER" "$out" "PEER"
hasnt "and never to this session"                      "$out" "MINE"

# CONTROL, the other direction. `scan()` falls back to the FILENAME when a record carries
# no sessionId -- and a subagent file's stem is `agent-<id>`, which is not a session id and
# addresses nobody. The fallback for these files has to be the PARENT DIRECTORY. A refusal
# naming `agent-a0123456789abcdef` as the owner is worse than UNKNOWN: UNKNOWN says "cannot
# tell", this would say "ask a session that does not exist".
SUBC="$T/profileA/projects/-repo/$ME/subagents/agent-anosessionid00000.jsonl"
sub_tool_use "$SUBC" mcp__codescout__create_file '{"path":"src/no_sid.rs","content":"x"}'
out=$(run src/no_sid.rs)
has   "a subagent record with no sessionId falls back to the PARENT DIR" "$out" "MINE"
hasnt "and never to the agent- filename, which addresses nobody"         "$out" "agent-a"

# The UNKNOWN message is the surface a reader ACTS on, and it is the half no assertion
# covered -- the bug file proposed one and it was never written, which is how the message
# went on telling readers "stop looking for the owner" of a subagent write. This is a
# SHAPE test, not a prose pin: it reds on the sentence coming back, and survives rewording.
unk=$(run src/never_written_by_anyone.rs)
has   "UNKNOWN still names Bash as the real blind spot" "$unk" "Bash write"
hasnt "and no longer tells the reader to stop looking"  "$unk" "stop looking for the owner"
hasnt "nor repeats the falsified zero-records count"    "$unk" "no subagent activity"

echo
echo "== UNKNOWN names the frame it was computed in =="
# The window line printed on MINE/SHARED/PEER and NOT on UNKNOWN -- the one verdict whose
# entire meaning IS the window. Four readers misread it in one day:
# docs/issues/archive/2026-09-16-file-provenance-unknown-branch-omits-the-window-it-names.md.
# Three read a clean file's UNKNOWN as an attribution failure; one could not price the gap
# without a second --all run, which then named three LIVE peers.
#
# NOTE the fixture path deliberately shares NO substring with the marker asserted below,
# for the reason given at the --since section: a fixture name that can satisfy the
# assertion by itself makes the assertion unable to fail.
tool_use "$B" mcp__codescout__edit_file \
    '{"path":"src/frame_probe.rs","old_string":"a","new_string":"b"}' "2026-01-01T00:00:00.000Z"
fr=$(run --since 2026-06-01T00:00:00Z src/frame_probe.rs)
has   "a floor with no surviving writer still reaches UNKNOWN" "$fr" "UNKNOWN"
has   "and UNKNOWN now names its window"                       "$fr" "window: writes at or after"
has   "and the floor VALUE, not merely the word"               "$fr" "2026-06-01"

# The opposite direction, and the constraint the fix had to respect: with no floor there is
# no frame to name, so the silence is ALREADY correct and a window line would name a frame
# that does not exist. $unk is the no-writer fixture from the section above; its path is not
# committed in $T/repo, so last_commit_time() finds nothing and the floor stays None.
# This assertion is monotone under REMOVAL -- it passes against a tool with no window
# support at all -- so it is evidence only beside the three above, never on its own.
hasnt "and stays silent when no floor exists" "$unk" "window:"

# And where git cannot answer, the tool must not answer either. src/frame_probe.rs is not
# TRACKED in $T/repo, and that is the case a bare `git status --porcelain -- <path>` gets
# WRONG: it succeeds and prints nothing, byte-identical to a clean tracked file. The first
# draft of worktree_is_dirty() read exactly that and printed a dispositive clearance here,
# about a path git holds no baseline for -- caught by this assertion, not by reading, while
# its own docstring already called the three-valued None load-bearing.
hasnt "and infers no cause for a path git holds no baseline for" "$fr" "LIKELY CAUSE"

echo
echo "== UNKNOWN names the LIKELY cause, not only the rare one =="
# Half 2 of the same bug, and an ACTIVE FALSE CLAIM rather than an omission. The message
# led with "The one blind spot is a Bash write" -- a completeness assertion -- while the
# common cause of UNKNOWN is a committed file whose default floor sits at its own commit
# time. A reader doing exactly what the message said went to investigate the rarer cause.
#
# Reuses the git fixture $T/repo already seeded by the DEFAULT-window section above. A
# first draft of this section stood up a SECOND repo, on the strength of a `git init` grep
# whose output `head -40` truncated before line 602 -- an absence read off a cap, which is
# the same class of error as the verdict this section is about.
echo "v1" > "$T/repo/src/settled.rs"
git -C "$T/repo" add src/settled.rs
git -C "$T/repo" commit -q -m "seed settled"
# A write recorded long BEFORE that commit, so every write on record predates the derived
# floor and no writer survives into the window.
tool_use "$B" mcp__codescout__edit_file \
    '{"path":"src/settled.rs","old_string":"a","new_string":"b"}' "2020-01-01T00:00:00.000Z"

st=$(run src/settled.rs)
has   "a clean committed path still reaches UNKNOWN"          "$st" "UNKNOWN"
has   "and names the window the default floor produced"       "$st" "window: writes at or after"
has   "and a clean tree is reported as settling the question" "$st" "dispositive"
hasnt "and the completeness claim is gone"                    "$st" "The one blind spot"

# THE HEDGE MUST NAME ITS OWN SUBJECT, and this regressed the moment the clearance was
# added above it. `no record … in the window. That is a statement about coverage` read
# unambiguously while it was the FIRST line after the verdict -- "That" could only be the
# record absence. Inserting LIKELY CAUSE between them displaced the antecedent, and the
# clearance's own closing clause is `rather than a coverage gap`, so two adjacent sentences
# both opened "That is" and said opposite things about coverage. A reader taking the last
# one as the verdict's summary gets back the exact misreading this file was opened for.
# Found by codescout-61 running the fix rather than reading it; the displaced-antecedent
# mechanism is not visible in the diff, only in the rendered output.
# The `hasnt` is monotone under removal -- deleting the prose satisfies it -- so it is
# evidence only beside the `has`. Together they say: the sentence is present AND it is not
# the bare-demonstrative form. Neither can tell you the wording is CLEAR.
has   "the hedge names the absence as its subject"            "$st" "That absence is a statement"
hasnt "and not the bare demonstrative the clearance collides with" "$st" "window. That is a statement"

# The expensive direction, and the row the hidden hint CANNOT separate from the one above:
# same floor, same records, same UNKNOWN -- and uncommitted bytes really are held. This is
# incident 2, where --all went on to name three LIVE peers. git cleanliness is a SECOND
# instrument sharing no blind spot with the transcript heuristics, which is the only reason
# either verdict here is worth stating.
printf 'v2\n' >> "$T/repo/src/settled.rs"
dt=$(run src/settled.rs)
has   "a DIRTY path with every write outside the window says so" "$dt" "too narrow"
hasnt "and claims no clearance on the identical record set"      "$dt" "dispositive"

# ZERO records with a floor: the window still prints, and there is nothing to infer a cause
# FROM, so the tool must say nothing. This row is absent from the filed fix plan's table,
# and the reproduction is what produced it -- the plan's `hidden == len(records)` guard is
# 0 == 0 here and would have claimed "no session holds uncommitted bytes" about a path it
# knows nothing about. The mutation dropping `records` from the guard SURVIVED all 150
# assertions before this case existed.
echo "v1" > "$T/repo/src/unrecorded.rs"
git -C "$T/repo" add src/unrecorded.rs
git -C "$T/repo" commit -q -m "seed unrecorded"
nr=$(run src/unrecorded.rs)
has   "a committed path with zero records still names its window" "$nr" "window: writes at or after"
hasnt "and infers no cause from zero records"                     "$nr" "LIKELY CAUSE"

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" = "0" ]
