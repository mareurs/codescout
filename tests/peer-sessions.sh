#!/usr/bin/env bash
#
# Discrimination suite for `scripts/peer-sessions.sh`'s binary-freshness detector.
#
# WHY THIS EXISTS
# ---------------
# A Claude Code session's codescout server holds the text pages it exec'd. Rebuild the
# binary and that process keeps serving the old bytes — silently. Start time, cwd and a
# correct ~/.cargo/bin symlink all read healthy, so nothing else in the repo reports it.
# Measured 2026-09-01: three rebuilds in two hours (e7260ac5 -> 32ebfc9a -> 6c0498f7),
# each one silently demoting whichever servers were running, and one session hand-rolled
# the comparison three times because the script would not do it.
#
# THE FIX THIS PINS IS THE *SECOND* ONE, AND THAT IS THE WHOLE POINT.
# docs/issues/archive/2026-08-31-peer-sessions-never-compares-start-time-to-build-time.md first
# proposed comparing process start time against the binary's mtime:
#
#     exe=$(readlink "/proc/$pid/exe"); [ "$(stat -c %Y /proc/$pid)" -lt "$(stat -c %Y "$exe")" ]
#
# That FAILS OPEN in exactly its target case. Once a binary is replaced, `readlink`
# returns the path with a literal " (deleted)" suffix, `stat` on it fails, the comparison
# gets an empty string, `[ N -lt "" ]` errors to stderr and evaluates false. The one
# branch that must fire is the only one that cannot, and a caller redirecting stderr sees
# a clean report. The suffix IS the answer; the timestamp comparison was a proxy for a
# question readlink had already answered.
#
# So the case that matters here is REPLACED. A suite that only checked `current` would
# pass against the broken version, against a deleted function, and against a machine
# where nothing had been rebuilt.
#
# EVERY CASE BELOW IS A DISCRIMINATION, AND CASES 1+2 SHARE ONE PID.
# The same process is asserted `current` and then `REPLACED` with nothing changed but the
# file underneath it. That is what makes the pair non-vacuous by construction: a broken
# extraction, an empty source file or a stubbed function cannot satisfy both halves. A
# suite asserting only the second could pass while reporting REPLACED for everything.
#
# Usage:
#   tests/peer-sessions.sh          # non-zero exit on any failure
#
# Runs entirely under $TMPDIR against a copy of /bin/sleep. It never touches this
# checkout, which matters because several sessions share it.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/scripts/peer-sessions.sh"
PASS=0
FAIL=0

ok()   { echo "  PASS  $1"; PASS=$((PASS + 1)); }
bad()  { echo "  FAIL  $1"; echo "          want=[$2] got=[$3]"; FAIL=$((FAIL + 1)); }
eq()   { [ "$2" = "$3" ] && ok "$1" || bad "$1" "$2" "$3"; }

[ -r "$SRC" ] || { echo "cannot read $SRC" >&2; exit 1; }

T="$(mktemp -d)"
cleanup() {
    [ -n "${PID:-}" ] && kill -KILL "$PID" 2>/dev/null
    [ -n "${SPID:-}" ] && kill -KILL "$SPID" 2>/dev/null
    for _k in "${OLD:-}" "${NEW:-}" "${FAKE:-}"; do [ -n "$_k" ] && kill -KILL "$_k" 2>/dev/null; done
    rm -rf "$T"
}
trap cleanup EXIT

# Extract the two functions under test rather than sourcing the script, which runs its
# whole report at load. Asserted non-empty below: a sed that silently matched nothing
# would leave every case erroring identically to a genuine failure, and `command -v` is
# what tells those apart.
sed -n '/^binary_state() {/,/^}/p;/^binary_name() {/,/^}/p;/^proc_state() {/,/^}/p;/^scan_cs_children() {/,/^}/p' "$SRC" > "$T/fns.sh"
. "$T/fns.sh"

echo "== extraction =="
eq "binary_state was extracted from the script"  "yes" "$(command -v binary_state >/dev/null && echo yes || echo no)"
eq "binary_name was extracted from the script"   "yes" "$(command -v binary_name  >/dev/null && echo yes || echo no)"
eq "proc_state was extracted from the script"    "yes" "$(command -v proc_state   >/dev/null && echo yes || echo no)"
eq "scan_cs_children was extracted from the script" "yes" "$(command -v scan_cs_children >/dev/null && echo yes || echo no)"

echo
echo "== one process, two states =="
cp /bin/sleep "$T/prog"
"$T/prog" 300 &
PID=$!
# Give the exec a moment to land without a foreground sleep: poll /proc for the link.
for _ in $(seq 1 200); do [ -n "$(readlink "/proc/$PID/exe" 2>/dev/null)" ] && break; done

eq "an intact binary reads current"              "current"  "$(binary_state "$PID")"
eq "and its name carries no state suffix"        "prog"     "$(binary_name "$PID")"

rm -f "$T/prog"

eq "the SAME pid reads REPLACED once its binary is gone" "REPLACED" "$(binary_state "$PID")"
eq "and the name still strips the ' (deleted)' suffix"   "prog"     "$(binary_name "$PID")"

echo
echo "== unreadable process =="
# A pid that has exited: readlink fails, and the detector must say so rather than
# guessing either way. Reporting `current` here would hide a dead peer; reporting
# REPLACED would invent one.
kill -KILL "$PID" 2>/dev/null
wait "$PID" 2>/dev/null
eq "a dead pid is unknown, not current and not REPLACED" "?" "$(binary_state "$PID")"
eq "binary_name agrees it is unknown"                    "?" "$(binary_name "$PID")"
PID=""

echo
echo "== the caller wires it through =="
# binary_state is only useful if the report actually prints its verdict. Guarding the
# function alone would pass against a script that computes the state and drops it —
# which is the shape of every 'declared but not wired' defect in this corpus.
eq "the report emits the REPLACED verdict"  "yes" \
   "$(grep -q 'REPLACED' "$SRC" && echo yes || echo no)"
eq "and counts replaced rows for the summary" "yes" \
   "$(grep -q 'replaced=\$((replaced + 1))' "$SRC" && echo yes || echo no)"

echo
echo "== one process, stopped and resumed =="
# A stopped session keeps its socket and registry row, so every other column reads it as
# a busy peer. Same shape as the pair above: ONE pid read ok, then STOPPED, then ok again,
# with nothing changed but a signal to a process this suite spawned itself.
#
# The binary's NAME is load-bearing: `a) T b` puts `) T ` inside the comm, so a parser
# that takes the field after the FIRST ')' reads `T` and calls a sleeping process
# stopped. Rename it to anything without a ')' and the first assertion stops
# discriminating that parse, while still passing.
cp /bin/sleep "$T/a) T b"
"$T/a) T b" 300 &
SPID=$!
for _ in $(seq 1 200); do [ -n "$(readlink "/proc/$SPID/exe" 2>/dev/null)" ] && break; done

eq "a sleeping process whose comm contains ') T ' reads ok" "ok" "$(proc_state "$SPID")"
kill -STOP "$SPID"
for _ in $(seq 1 200); do [ "$(proc_state "$SPID")" = "STOPPED" ] && break; done
eq "the SAME pid reads STOPPED once SIGSTOPped" "STOPPED" "$(proc_state "$SPID")"
kill -CONT "$SPID"
for _ in $(seq 1 200); do [ "$(proc_state "$SPID")" = "ok" ] && break; done
eq "and ok again once resumed" "ok" "$(proc_state "$SPID")"
kill -KILL "$SPID" 2>/dev/null
wait "$SPID" 2>/dev/null
eq "a dead pid is unknown, not ok and not STOPPED" "?" "$(proc_state "$SPID")"
SPID=""

# Wired through, for both processes a row describes: the session's STATE column, and
# the codescout server's, which is the one holding catalog locks.
eq "the report prints the session's state" "yes" \
   "$(grep -q 'state=\$(proc_state "\$pid")' "$SRC" && echo yes || echo no)"
eq "and the codescout server's" "yes" \
   "$(grep -q 'cs_st=\$(proc_state "\$cs_pid")' "$SRC" && echo yes || echo no)"
eq "and counts rows that cannot answer for the summary" "yes" \
   "$(grep -q 'unanswerable=\$((unanswerable + 1))' "$SRC" && echo yes || echo no)"

echo
echo "== a session with two codescout children =="
# A /mcp can leave the replaced server alive beside its replacement
# (docs/issues/2026-09-25-mcp-reconnect-can-leave-the-replaced-server-connected.md). The
# stand-in session is a subshell whose two children are a copy of sleep named `codescout`,
# the second started at least one clock tick after the first. The real scan runs over
# the real /proc, and its entries for this subshell's pid are ours alone.
#
# WHAT THIS CANNOT DISCRIMINATE, stated so nobody credits it: the NEWER child also has
# the higher pid here, with the same digit count, so "the last child the lexical glob
# visits" and "the newest" pick the same one, and a scan that just kept the last visited
# passes the first assertion. The field case that broke it was a newer server with a
# lexically SMALLER pid, which only pid wrap produces, and no test can arrange that. The
# superseded count is what this fixture does guard.
cp /bin/sleep "$T/codescout"
( "$T/codescout" 300 & echo $! > "$T/old"; sleep 0.05; "$T/codescout" 300 & echo $! > "$T/new"; wait ) &
FAKE=$!
for _ in $(seq 1 500); do
    [ -s "$T/new" ] && [ -n "$(readlink "/proc/$(cat "$T/new")/exe" 2>/dev/null)" ] && break
    sleep 0.01
done
OLD=$(cat "$T/old"); NEW=$(cat "$T/new")
declare -A CS_OF_SESSION CS_START CS_EXTRA
scan_cs_children
eq "the newest codescout child is the one reported" "$NEW" "${CS_OF_SESSION[$FAKE]:-}"
eq "and the older one is counted as superseded"     "1"    "${CS_EXTRA[$FAKE]:-0}"
eq "the report prints the superseded count" "yes" \
   "$(grep -q ' superseded"' "$SRC" && echo yes || echo no)"
kill -KILL "$OLD" "$NEW" "$FAKE" 2>/dev/null
OLD=""; NEW=""; FAKE=""

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" = "0" ]
