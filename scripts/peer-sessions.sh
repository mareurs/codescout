#!/usr/bin/env bash
# peer-sessions.sh — enumerate the Claude Code sessions actually sharing a checkout.
#
# WHY THIS EXISTS
#
# `ListAgents` is scoped to ONE profile's socket registry, while the socket
# directory is machine-global. This machine runs three profiles (~/.claude,
# ~/.claude-sdd, ~/.claude-kat), so a session started under a different profile is
# invisible to it — and the omission is silent: the short count is reported as
# complete, with no hint that it is a subset.
#
# Measured 2026-08-30: ListAgents reported 3 sessions in the codescout checkout
# (self + 2 peers). This probe found FIVE, one of which had been running since
# 11:10:52 — before any inter-session message that day — and was invisible to
# every participant. Six authorship misattributions were made that afternoon, each
# a correct elimination over a population short by at least two.
#
# See docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md
# (BL-58). The underlying defect is in the harness and cannot be fixed here; this
# is the local mitigation, made runnable so it fires at the moment of use rather
# than being remembered.
#
# USAGE
#   ./scripts/peer-sessions.sh            # every session, all checkouts
#   ./scripts/peer-sessions.sh .          # only sessions whose cwd is HERE
#   ./scripts/peer-sessions.sh /some/repo # only sessions in that tree
set -euo pipefail

SOCK_DIR="/run/user/$(id -u)/cc-socks"
FILTER="${1:-}"
if [ -n "$FILTER" ]; then
    FILTER=$(cd "$FILTER" && pwd)
fi

if [ ! -d "$SOCK_DIR" ]; then
    echo "no socket directory at $SOCK_DIR — no Claude Code sessions registered" >&2
    exit 0
fi

# Identify the caller by walking up from THIS shell, which is a child of its own
# MCP server by construction. Do NOT use `pgrep … | head -1`: several codescout
# servers run at once and head -1 samples arbitrarily — measured 2026-08-30
# returning a chain that terminated at a PEER's session, i.e. it identified the
# caller as someone who sends the caller messages.
self_pid=""
p=$$
for _ in 1 2 3 4 5 6; do
    [ -r "/proc/$p/comm" ] || break
    if [ "$(tr -d '\0' < "/proc/$p/comm")" = "claude" ]; then
        self_pid="$p"
        break
    fi
    p=$(awk '{print $4}' "/proc/$p/stat" 2>/dev/null) || break
    [ -z "$p" ] && break
    [ "$p" = "1" ] && break
done

printf '%-9s %-4s %-46s %-24s %s\n' PID SELF CWD STARTED STATE
live=0
stale=0
matched=0
for sock in "$SOCK_DIR"/*.sock; do
    [ -e "$sock" ] || continue
    pid=$(basename "$sock" .sock)

    if [ ! -d "/proc/$pid" ]; then
        stale=$((stale + 1))
        printf '%-9s %-4s %-46s %-24s %s\n' "$pid" "" "" "" "stale socket (no process)"
        continue
    fi

    comm=$(tr -d '\0' < "/proc/$pid/comm" 2>/dev/null || echo "?")
    cwd=$(readlink "/proc/$pid/cwd" 2>/dev/null || echo "(unreadable)")
    started=$(ps -o lstart= -p "$pid" 2>/dev/null | sed 's/^ *//')
    live=$((live + 1))

    if [ -n "$FILTER" ] && [ "$cwd" != "$FILTER" ]; then
        continue
    fi
    matched=$((matched + 1))

    mark=""
    [ "$pid" = "$self_pid" ] && mark="<--"
    printf '%-9s %-4s %-46s %-24s %s\n' "$pid" "$mark" "$cwd" "$started" "$comm"
done

echo
if [ -n "$FILTER" ]; then
    echo "$matched session(s) with cwd = $FILTER; $live live overall, $stale stale socket(s)."
    if [ "$stale" -gt 0 ]; then
        echo "Stale sockets are listed regardless of the filter and are NOT in the $matched:"
        echo "a dead process has no readable cwd, so whether it was in this tree is unknowable."
    fi
else
    echo "$live live session(s), $stale stale socket(s)."
fi
echo
echo "Any pid above is addressable whether or not ListAgents lists it:"
echo "  SendMessage(to: \"uds:$SOCK_DIR/<pid>.sock\", …)"
echo
echo "This bounds the POPULATION. It does not attribute a write — eliminating over"
echo "a complete set is still elimination. To attribute, ask, or find a positive"
echo "identification; do not infer authorship from who else was present."
