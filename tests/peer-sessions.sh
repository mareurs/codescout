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
cleanup() { [ -n "${PID:-}" ] && kill -KILL "$PID" 2>/dev/null; rm -rf "$T"; }
trap cleanup EXIT

# Extract the two functions under test rather than sourcing the script, which runs its
# whole report at load. Asserted non-empty below: a sed that silently matched nothing
# would leave every case erroring identically to a genuine failure, and `command -v` is
# what tells those apart.
sed -n '/^binary_state() {/,/^}/p;/^binary_name() {/,/^}/p' "$SRC" > "$T/fns.sh"
. "$T/fns.sh"

echo "== extraction =="
eq "binary_state was extracted from the script"  "yes" "$(command -v binary_state >/dev/null && echo yes || echo no)"
eq "binary_name was extracted from the script"   "yes" "$(command -v binary_name  >/dev/null && echo yes || echo no)"

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
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" = "0" ]
