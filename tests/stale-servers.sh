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
# docs/issues/2026-09-14-the-stale-server-probe-counts-lsp-muxes-under-a-name-that-excludes-them.md
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
echo "== the summary names its population =="
# Population-independent: on a runner with no codescout process at all the rows are empty
# and both counters print zero, so these hold in CI and on a loaded workstation alike.
LIVE="$(bash "$SRC")"
eq "the table carries a KIND column"            "yes" "$(has 'KIND' "$LIVE")"
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
