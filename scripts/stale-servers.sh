#!/usr/bin/env bash
#
# Which running codescout processes are executing a binary that no longer exists on disk,
# and which of them is a thing you can actually do something about?
#
# WHY THIS EXISTS
#   `cargo rb` + `/mcp` makes the new code run in THIS session. It does nothing for every
#   other Claude Code session already holding a codescout server: those keep executing the
#   binary they started from, serving that build's guides, prompt surfaces and guide
#   routing until they are reconnected. Measured 2026-08-21 on this machine: 22 of 26
#   processes stale, oldest 17 days. Nothing raises — the commits are on the branch, the
#   binary on disk is current, the suite is green, and only /proc disagrees.
#
# WHY IT REPORTS TWO POPULATIONS AND NO COMBINED TOTAL
#   `pgrep -x codescout` matches on the process NAME, and a mux runs the same binary — so
#   an LSP multiplexer (`codescout mux --socket …`) is indistinguishable from an MCP server
#   (`codescout start`) by name alone. Until 2026-09-14 this script counted them together
#   and printed one `total=`, under a header that said "servers". Two things were wrong
#   with that, and the second is the one that cost a reader something:
#
#     - the number was a mixed unit. Measured 2026-09-14 05:45:56: `total=22` where one
#       row was a kotlin mux, i.e. 1 of the 4 rows reported `current`. The share is not
#       reliably small — a partition verified 2026-09-01 read 9 servers + 3 muxes.
#     - the closing remedy said "Reconnect those sessions (/mcp)", which a mux row cannot
#       obey. A mux has no Claude Code session and no /mcp; it is spawned by a server,
#       keyed by project hash, shared across sessions, and it EXITS BY ITSELF once idle
#       past its `--idle-timeout` (its own cmdline carries the value). A server has no
#       idle-timeout option, so a stale one never recycles. Opposite correct actions,
#       one sentence, nothing in the output telling them apart.
#
#   So there is deliberately no `total=` line to bring back. The unit error is not policed,
#   it is unrepresentable: every count this prints names its population.
#   (docs/issues/archive/2026-09-14-the-stale-server-probe-counts-lsp-muxes-under-a-name-that-excludes-them.md)
#
# WHAT THE PREDICATE LITERALLY COUNTS
#   Processes named exactly `codescout` whose /proc/<pid>/exe symlink resolves to a path
#   ending in " (deleted)" — the kernel reporting that the inode the process is executing
#   has been unlinked, which is what a release build does to the previous binary.
#
#   It is NOT a version comparison. A process started from a byte-identical rebuild also
#   reads STALE, because the inode changed even though the contents did not. And a process
#   reading `current` is only current relative to the inode now at that path.
#
# BLIND SPOTS — note which DIRECTION each one bounds the count in
#   Under-count (the number is a floor):
#     - Only processes whose /proc this user can read.
#     - `pgrep -x codescout` matches argv[0] exactly: a process launched through a wrapper
#       or a differently-named symlink is invisible here. (~/.cargo/bin/codescout is a
#       symlink to target/release/codescout, so `exe` resolves to the target — that case
#       IS covered.)
#     - The count is a floor if a process is mid-start.
#   Over-count (the number is a ceiling):
#     - A mux is counted, as a mux. Before the split above it was counted as a server, and
#       that was the only blind spot running in this direction while all the documented
#       ones ran the other way — so a reader who read the caveats carefully concluded the
#       number was a floor when it was neither.
#   Neither:
#     - Says nothing about WHICH build a stale process runs, only that it is not the
#       current inode. A deleted inode carries no version.
#
# TEST SEAMS
#   --classify        read one cmdline per line on stdin, print `server` or `mux` per line
#   --conn            read `pid|ppid|start|kind|parent` lines on stdin, print `pid|CONN`
#   --remedy N M [K]  print the remedy for N reconnectable stale servers, M stale muxes
#                     and K superseded servers
#   All three exist so `tests/stale-servers.sh` can drive the real classifiers and the real
#   remedy text without live processes of any kind: a mux exists only while a language
#   server is warm, which is exactly why the mux defect was absent from most runs, and a
#   superseded server exists only after a /mcp the harness failed to close.
#
# THE SECOND AXIS: CONN, whether anything still TALKS to a server (added 2026-09-25)
#   STATUS answers "is this the binary on disk?". It cannot answer "is anyone connected?",
#   and the two come apart. A `/mcp` spawns a new server, but the harness does not always
#   close the old one's stdin, so the old process lives on with nobody to send it a request
#   (docs/issues/2026-09-25-mcp-reconnect-can-leave-the-replaced-server-connected.md). On a
#   live binary it read `current` here, the one label guaranteed to stop a reader looking.
#   CONN marks a server:
#     live        the NEWEST server under a Claude Code session: the one it talks to
#     SUPERSEDED  an older server under the same session. A newer one replaced it.
#     NO-PARENT   its parent process no longer exists
#     -           a mux, or a parent that is not a Claude Code session
#   "Newest is the one it talks to" holds by construction of /mcp, which spawns the
#   replacement, and was observed directly: workspace(status) named the newest of three.
#   It is NOT applied to other parents. A codex process has been seen running four
#   servers at once, and nothing here says it uses only one, so those rows get `-`.
#   A session is recognised by its socket in cc-socks, never by `comm`: a version-pinned
#   install names its binary after the version, so its comm is `2.1.258`, not `claude`.
#   Starts are compared as NUMBERS (clock ticks), because the text "1000" sorts before "999".
#
# Reports only; never gates. Always exits 0.

set -uo pipefail

# The whole partition, in one place. Sourced by both the live loop and `--classify`, so a
# test that drives the flag is driving production and not a re-implementation of it.
#
# Matched on the process's OWN cmdline rather than on its parent's, deliberately. The
# parent test — "is the parent a `claude`?" — carries a latent false positive on any
# checkout whose own path contains a `claude` component, which this one does: a mux whose
# parent server was launched from `target/release/codescout` classifies as a server under it. That test
# happens not to fire only because live servers run via the ~/.cargo/bin/codescout symlink,
# which is an accident of launch path and not a property of the test. Found by a peer
# 2026-09-01 by checking the snippet against every live mux instead of assuming; the
# `mux --socket` form needs no parent lookup and gave the identical partition on the same
# population (9 servers + 3 muxes). Also recorded in memory `gotchas` § MCP Binary Symlink.
kind_of_cmdline() {
    case "$1" in
        *"mux --socket"*) echo mux ;;
        *)                echo server ;;
    esac
}

cmdline_of() {
    tr '\0' ' ' < "/proc/$1/cmdline" 2>/dev/null
}

SOCK_DIR="/run/user/$(id -u)/cc-socks"

# `session` / `other` / `gone`. /proc first: a dead session can leave its socket file behind,
# and a socket with no process is not a session anyone is in.
parent_kind() {
    if [ ! -d "/proc/$1" ]; then
        echo gone
    elif [ -S "$SOCK_DIR/$1.sock" ]; then
        echo session
    else
        echo other
    fi
}

# Start time in clock ticks since boot: /proc/<pid>/stat field 22, read after the LAST ')'
# because a comm may contain spaces and ')'. Finer than `etimes`, whose one-second
# resolution ties a server with its replacement when /mcp runs twice in a second.
start_ticks() {
    _st=$(cat "/proc/$1/stat" 2>/dev/null) || { echo 0; return; }
    _st=${_st##*) }
    awk '{ print $20 }' <<< "$_st"
}

# The CONN axis, over the whole population at once, because "superseded" is a property of
# a server RELATIVE to its siblings and no per-process test can see it. Reads
# `pid|ppid|start|kind|parent` lines; prints `pid|CONN` in input order.
mark_superseded() {
    awk -F'|' '
        {
            pid[NR] = $1; pp[NR] = $2; st[NR] = $3 + 0; kd[NR] = $4; par[NR] = $5
            if ($4 == "server" && $5 == "session" && (!($2 in newest) || $3 + 0 > newest[$2]))
                newest[$2] = $3 + 0
        }
        END {
            for (i = 1; i <= NR; i++) {
                if (kd[i] != "server")          c = "-"
                else if (par[i] == "gone")      c = "NO-PARENT"
                else if (par[i] != "session")   c = "-"
                else if (st[i] < newest[pp[i]]) c = "SUPERSEDED"
                else                            c = "live"
                print pid[i] "|" c
            }
        }'
}

# Split by addressee, because the two populations' correct next actions are opposite and a
# reader who follows the wrong one either hunts a session that does not exist or leaves a
# process running that will never recycle on its own. Each branch is emitted only when its
# own population is non-empty, so the message never names an action with nobody to perform
# it — the failure CLAUDE.md § Testing Discipline names as the untested half of a guard.
remedy() {
    local stale_servers="$1" stale_muxes="$2" superseded="${3:-0}"
    [ "$stale_servers" -eq 0 ] && [ "$stale_muxes" -eq 0 ] && [ "$superseded" -eq 0 ] && return 0
    echo
    if [ "$stale_servers" -gt 0 ]; then
        echo "SERVERS: a stale server serves the guides, prompt surfaces and guide routing of"
        echo "the build it started from, and never recycles: 'codescout start' takes no"
        echo "idle-timeout. Reconnect those sessions (/mcp) to pick up the current binary."
    fi
    if [ "$superseded" -gt 0 ]; then
        # Its own branch because the SERVERS remedy is wrong for it: /mcp is what left it.
        echo "SUPERSEDED: a newer server under the same session replaced this one on a /mcp, and"
        echo "nothing will send it a request again, whatever its STATUS says. /mcp does not reap"
        echo "it, because /mcp is what left it. Nor does SIGTERM (measured 2026-09-25); SIGKILL"
        echo "does, and the session's live server is unaffected. Killing one is the operator's"
        echo "call for that session. Before doing it, check that its stdin peer is still the"
        echo "session's claude process and that a newer server is live:"
        echo "docs/issues/2026-09-25-mcp-reconnect-can-leave-the-replaced-server-connected.md."
    fi
    if [ "$stale_muxes" -gt 0 ]; then
        echo "MUXES: no action, and there is no session to reconnect. A mux is spawned by a"
        echo "server, keyed by project hash, shared across sessions, and exits by itself once"
        echo "idle past the --idle-timeout its own cmdline carries. Stale exactly while it is"
        echo "being queried; healthy the moment nobody cares."
    fi
}

case "${1-}" in
    --classify)
        while IFS= read -r line; do kind_of_cmdline "$line"; done
        exit 0
        ;;
    --conn)
        mark_superseded
        exit 0
        ;;
    --remedy)
        remedy "${2-0}" "${3-0}" "${4-0}"
        exit 0
        ;;
esac

rows=""
conn_in=""
n_server=0
n_mux=0
stale_server=0
stale_mux=0

for p in $(pgrep -x codescout 2>/dev/null); do
    exe=$(readlink "/proc/$p/exe" 2>/dev/null) || continue
    kind=$(kind_of_cmdline "$(cmdline_of "$p")")
    case "$exe" in
        *" (deleted)") flag=STALE ;;
        *)             flag=current ;;
    esac
    if [ "$kind" = mux ]; then
        n_mux=$((n_mux + 1))
        [ "$flag" = STALE ] && stale_mux=$((stale_mux + 1))
    else
        n_server=$((n_server + 1))
        [ "$flag" = STALE ] && stale_server=$((stale_server + 1))
    fi
    # Sort key is etimes (elapsed SECONDS, numeric). Deliberately NOT `ps lstart`: that is
    # a "Wed Aug 20 …" string, and sorting it lexically orders by WEEKDAY NAME — which on
    # 2026-08-21 reported two-day-old processes as the newest on a machine whose newest
    # was 17 seconds old (reconnaissance-patterns:R-104).
    etimes=$(ps -o etimes= -p "$p" 2>/dev/null | tr -d ' ')
    ppid=$(ps -o ppid= -p "$p" 2>/dev/null | tr -d ' ')
    started=$(ps -o lstart= -p "$p" 2>/dev/null)
    rows="${rows}${etimes:-999999999}|${p}|${ppid}|${kind}|${flag}|${started}"$'\n'
    conn_in="${conn_in}${p}|${ppid}|$(start_ticks "$p")|${kind}|$(parent_kind "${ppid:-0}")"$'\n'
done

declare -A CONN
while IFS='|' read -r cp cc; do
    [ -n "${cp:-}" ] && CONN[$cp]=$cc
done < <(printf '%s' "$conn_in" | mark_superseded)

# A superseded server is counted out of the SERVERS remedy even when it is stale: telling
# its session to /mcp is the one instruction that cannot help it.
superseded=0
stale_live=0
for cp in "${!CONN[@]}"; do
    [ "${CONN[$cp]}" = SUPERSEDED ] && superseded=$((superseded + 1))
done
while IFS='|' read -r _e pid _pp kd fl _st; do
    [ -n "${pid:-}" ] || continue
    [ "$kd" = server ] && [ "$fl" = STALE ] && [ "${CONN[$pid]:-}" != SUPERSEDED ] &&
        stale_live=$((stale_live + 1))
done <<< "$rows"

printf '%-10s %-9s %-9s %-7s %-8s %-11s %s\n' AGE_SEC PID PPID KIND STATUS CONN STARTED
printf '%s' "$rows" | sort -n -t'|' -k1 | while IFS='|' read -r e pid pp kd fl st; do
    [ -n "${pid:-}" ] || continue
    printf '%-10s %-9s %-9s %-7s %-8s %-11s %s\n' "$e" "$pid" "$pp" "$kd" "$fl" "${CONN[$pid]:-?}" "$st"
done

echo
# One line per population, and no combined figure. A `total=` here would be the exact
# mixed unit this script was fixed to stop printing.
printf 'servers=%-4s stale=%-4s current=%-4s superseded=%s\n' \
    "$n_server" "$stale_server" "$((n_server - stale_server))" "$superseded"
printf 'muxes=%-6s stale=%-4s current=%s\n'   "$n_mux"    "$stale_mux"    "$((n_mux - stale_mux))"

remedy "$stale_live" "$stale_mux" "$superseded"
exit 0
