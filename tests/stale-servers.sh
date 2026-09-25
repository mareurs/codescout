#!/usr/bin/env bash
#
# Discrimination suite for `scripts/stale-servers.sh`'s server/mux partition and its
# split remedy.
#
# WHY THIS EXISTS
# ---------------
# The probe selects with `pgrep -x codescout`, which matches on the process NAME — and an
# LSP mux runs the same binary as an MCP server. Until 2026-09-14 it counted them together
# and printed one `total=` under a header that said "servers", closing with a remedy
# ("Reconnect those sessions (/mcp)") that a mux row cannot obey: a mux has no session, and
# exits by itself once idle past its --idle-timeout, where a server never recycles.
# docs/issues/archive/2026-09-14-the-stale-server-probe-counts-lsp-muxes-under-a-name-that-excludes-them.md
#
# WHY IT IS DRIVEN THROUGH FLAGS AND NOT THROUGH LIVE PROCESSES.
# A mux exists only while a language server is warm — within --idle-timeout (300s Kotlin,
# 180s Rust) of an LSP call. That is precisely why the defect was invisible on most runs:
# with no warm mux the old script's output was accidentally correct. A suite that waited
# for a live mux would inherit that same intermittency and would pass, silently, in CI
# where no mux ever exists. `--classify` and `--remedy` exist so the real classifier and
# the real remedy text are reachable with neither population present.
#
# THEY DRIVE PRODUCTION, NOT A COPY. `--classify` calls `kind_of_cmdline` and `--remedy`
# calls `remedy` — the same two functions the live loop calls. A second implementation
# living in this file would be indistinguishable from coverage right up until the shipped
# one broke (CLAUDE.md § Testing Discipline: mutate the PRODUCTION path).
#
# THE CASE THAT MATTERS IS `mux`. A stub that answered "server" unconditionally would pass
# every server case below and reproduce the exact defect this suite was written for, so the
# server cases are not the guard — they are the control that makes the mux ones mean
# something.
#
# Usage:
#   tests/stale-servers.sh          # non-zero exit on any failure
#
# Needs only bash and coreutils. Reads no /proc it does not own, spawns nothing, and never
# writes to this checkout, which matters because several sessions share it.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/scripts/stale-servers.sh"
PASS=0
FAIL=0

ok()   { echo "  PASS  $1"; PASS=$((PASS + 1)); }
bad()  { echo "  FAIL  $1"; echo "          want=[$2] got=[$3]"; FAIL=$((FAIL + 1)); }
eq()   { [ "$2" = "$3" ] && ok "$1" || bad "$1" "$2" "$3"; }

classify() { printf '%s\n' "$1" | bash "$SRC" --classify; }
has()      { case "$2" in *"$1"*) echo yes ;; *) echo no ;; esac; }

# Shaped from /proc on 2026-09-14 05:45:48, with the --env pairs trimmed and the account
# name neutralised to `dev` — `no_tracked_script_hardcodes_a_personal_home_path`
# (tests/committed_paths.rs) refuses a machine-specific home in a tracked script, and it is
# right to: the home directory is not what either fixture is about. The load-bearing detail
# is the literal " mux --socket " run of characters: that substring, and not the word "mux"
# and not the path, is what the partition keys on. Shorten it and the suite still passes
# while testing nothing.
MUX_CMD='/home/dev/work/claude/codescout/target/release/codescout mux --socket /run/user/1000/codescout-kotlin-mux-7e868829c00fa9b2.sock --lock /run/user/1000/codescout-kotlin-mux-7e868829c00fa9b2.lock --cwd /home/dev/work/claude/codescout --idle-timeout 300 -- kotlin-lsp --stdio'
SRV_CMD='/home/dev/.cargo/bin/codescout start --debug '

echo "== the partition =="
eq "a server cmdline classifies as server"      "server" "$(classify "$SRV_CMD")"
eq "a mux cmdline classifies as mux"            "mux"    "$(classify "$MUX_CMD")"

# The equals form of the flag. `mux --socket=/path` still contains the substring, so this
# passes today for a reason worth pinning: a partition rewritten to split on whitespace
# fields and compare `$3` to "--socket" would break here and nowhere else above.
eq "the --socket=VALUE form is still a mux"     "mux" \
   "$(classify '/x/codescout mux --socket=/run/user/1000/y.sock --idle-timeout 180')"

# MUTANT-KILLER for a classifier narrowed to `*mux*`. A server binary living under a path
# containing "tmux" is a server; only the subcommand makes a mux. Delete the " --socket"
# from the production `case` pattern and this is the case that reds.
eq "a server under a path containing 'mux' is a server" "server" \
   "$(classify '/opt/tmux-tools/codescout start --debug')"

# MUTANT-KILLER for a classifier keyed on the repo path rather than the subcommand. The
# observed mux happens to run out of this checkout; that is an accident of where the binary
# lives, not a property of muxes.
eq "a mux outside this checkout is still a mux" "mux" \
   "$(classify '/usr/local/bin/codescout mux --socket /tmp/z.sock --idle-timeout 180')"

# Reads EVERY line, not just the first — kills a `--classify` that resolves one input and
# exits, which would leave the live loop's per-process call the only untested path.
eq "three cmdlines in, three verdicts out, in order" "server mux server" \
   "$(printf '%s\n%s\n%s\n' "$SRV_CMD" "$MUX_CMD" "$SRV_CMD" | bash "$SRC" --classify | tr '\n' ' ' | sed 's/ $//')"

echo
echo "== the remedy is split by addressee =="
# CLAUDE.md § Testing Discipline: a suite tests a guard's PREDICATE and never its REMEDY
# TEXT, so that half is untested by construction. These do not check the remedy is CORRECT
# — only that each branch reaches exactly the population that can act on it. That is the
# regression that actually happened: one sentence addressed to both.
SERVERS_ONLY="$(bash "$SRC" --remedy 3 0)"
MUXES_ONLY="$(bash "$SRC" --remedy 0 2)"
BOTH="$(bash "$SRC" --remedy 2 2)"

eq "stale servers are told to reconnect"        "yes" "$(has '/mcp' "$SERVERS_ONLY")"
eq "and are NOT told about an idle-timeout"     "no"  "$(has 'idle past' "$SERVERS_ONLY")"
eq "stale muxes are told to do nothing"         "yes" "$(has 'no action' "$MUXES_ONLY")"
eq "and are told what makes that safe"          "yes" "$(has 'idle past' "$MUXES_ONLY")"
# The one that matters: a mux row must never receive an instruction it cannot perform.
eq "and are NOT sent to /mcp"                   "no"  "$(has '/mcp' "$MUXES_ONLY")"
eq "both populations stale names both branches" "yes" \
   "$([ "$(has '/mcp' "$BOTH")" = yes ] && [ "$(has 'no action' "$BOTH")" = yes ] && echo yes || echo no)"
eq "neither stale says nothing at all"          "" "$(bash "$SRC" --remedy 0 0)"

echo
echo "== the second axis: CONN =="
# docs/issues/2026-09-25-mcp-reconnect-can-leave-the-replaced-server-connected.md. A /mcp
# can leave the replaced server alive with its stdin open and nobody sending requests.
# STATUS is about the binary, so on a live binary it said `current`. Driven through
# `--conn`, which calls the same mark_superseded the live loop does. Input lines are
# `pid|ppid|start|kind|parent`, with start in clock ticks.
conn() { printf '%s\n' "$@" | bash "$SRC" --conn | tr '\n' ' ' | sed 's/ $//'; }

eq "under one session, the older server is SUPERSEDED and the newer live" \
   "10|SUPERSEDED 11|live" \
   "$(conn '10|500|100|server|session' '11|500|200|server|session')"
# Kills a rule that marks by input position ("all but the last row") rather than by start.
eq "and input order does not change which one" "11|live 10|SUPERSEDED" \
   "$(conn '11|500|200|server|session' '10|500|100|server|session')"
# The starts differ in DIGIT COUNT on purpose: the text "1000" sorts before "999", so a
# comparison that went lexical would call the newer one superseded. Give both starts the
# same number of digits and this case stops discriminating that, while still passing.
eq "starts compare as numbers, so 1000 is newer than 999" "20|SUPERSEDED 21|live" \
   "$(conn '20|600|999|server|session' '21|600|1000|server|session')"
eq "a lone server under a session is live"     "30|live" "$(conn '30|700|100|server|session')"
# The rule is not applied where it was never shown to hold. A codex process has been seen
# running four servers at once, and nothing here says it uses only one.
eq "under a non-session parent, no server is judged" "40|- 41|-" \
   "$(conn '40|800|100|server|other' '41|800|200|server|other')"
# A mux started later must not supersede the server beside it. Muxes are children of
# servers in practice; sharing the ppid here isolates the kind filter from the parentage.
eq "a newer mux never supersedes a server"     "50|live 51|-" \
   "$(conn '50|900|100|server|session' '51|900|900|mux|session')"
eq "a server whose parent is gone is NO-PARENT" "60|NO-PARENT" "$(conn '60|999999|100|server|gone')"
eq "two sessions' servers never compete"       "70|live 71|live" \
   "$(conn '70|1001|100|server|session' '71|1002|900|server|session')"

# The remedy for a superseded server is its OWN branch. The SERVERS one sends its session
# to /mcp, which is the act that produced it.
SUPERSEDED_ONLY="$(bash "$SRC" --remedy 0 0 2)"
eq "superseded servers get their own remedy"   "yes" "$(has 'SUPERSEDED:' "$SUPERSEDED_ONLY")"
eq "which says /mcp does not reap them"        "yes" "$(has '/mcp does not reap' "$SUPERSEDED_ONLY")"
eq "and names who decides"                     "yes" "$(has "operator's" "$SUPERSEDED_ONLY")"
eq "and does NOT tell them to reconnect"       "no"  "$(has 'Reconnect those sessions' "$SUPERSEDED_ONLY")"

echo
echo "== the summary names its population =="
# Population-independent: on a runner with no codescout process at all the rows are empty
# and both counters print zero, so these hold in CI and on a loaded workstation alike.
LIVE="$(bash "$SRC")"
eq "the table carries a KIND column"            "yes" "$(has 'KIND' "$LIVE")"
eq "and a CONN column"                          "yes" "$(has 'CONN' "$LIVE")"
eq "superseded servers are counted"             "yes" "$(has 'superseded=' "$LIVE")"
eq "servers are counted under their own name"   "yes" "$(has 'servers=' "$LIVE")"
eq "muxes are counted under theirs"             "yes" "$(has 'muxes=' "$LIVE")"
# ABSENCE ASSERTION, monotone under removal — deleting the whole summary satisfies it. The
# two presence cases immediately above are its pair: together they say the counts exist AND
# that no combined one does. Neither is worth anything without the other.
eq "and no combined total is printed"           "no"  "$(has 'total=' "$LIVE")"
eq "it reports and never gates"                 "0"   "$(bash "$SRC" >/dev/null 2>&1; echo $?)"

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" = "0" ]
