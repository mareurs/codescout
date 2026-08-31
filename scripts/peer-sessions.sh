#!/usr/bin/env bash
# peer-sessions.sh — enumerate the Claude Code sessions actually sharing a checkout.
#
# WHY THIS EXISTS
#
# `ListAgents` is scoped to ONE profile's session registry, while the socket
# directory is machine-global. This machine runs three profiles (~/.claude,
# ~/.claude-sdd, ~/.claude-kat), so a session started under a different profile is
# invisible to it — and the omission is silent: the short count is reported as
# complete, with no hint that it is a subset.
#
# MECHANISM, measured 2026-08-31 (was inferred until then). Two layers, different
# scopes:
#
#   discovery  $CLAUDE_CONFIG_DIR/sessions/<pid>.json   PER-PROFILE
#   delivery   /run/user/<uid>/cc-socks/<pid>.sock      PER-USER, shared
#
# ListAgents renders the registry; SendMessage writes to the socket. That is the
# whole asymmetry: 3 records in ~/.claude/sessions + 4 in ~/.claude-sdd/sessions =
# 7 = every live session, while each session's ListAgents shows only its own
# profile's registry minus itself. Confirmed from BOTH cells, n=7, no exceptions.
#
# This script reads the socket directory, which cannot be scoped by config dir even
# in principle — that is why it sees all of them, and why the PROFILE column below
# can tell you exactly WHICH ONES your own ListAgents is blind to. Knowing the
# partition is enumerable is the difference between "the population is unknowable,
# use sockets" and "here is the population, and here is your blind half."
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

# Which config profile a session belongs to — the axis ListAgents is scoped to.
# An UNREADABLE environ yields "?" rather than the default: "we could not look"
# and "it is the default profile" are different facts, and collapsing them would
# make this probe commit the same sin it exists to expose.
profile_of() {
    _pp_pid="$1"
    if [ ! -r "/proc/$_pp_pid/environ" ]; then
        echo "?"
        return
    fi
    _pp_cfg=$(tr '\0' '\n' < "/proc/$_pp_pid/environ" 2>/dev/null \
              | sed -n 's/^CLAUDE_CONFIG_DIR=//p') || _pp_cfg=""
    # Unset is meaningful: it means the default profile, not "unknown".
    [ -z "$_pp_cfg" ] && _pp_cfg="$HOME/.claude"
    basename "$_pp_cfg"
}

self_profile="?"
if [ -n "$self_pid" ]; then
    self_profile=$(profile_of "$self_pid")
fi

printf '%-9s %-4s %-13s %-6s %-40s %-24s %s\n' PID SELF PROFILE LISTED CWD STARTED STATE
live=0
stale=0
matched=0
blind=0
visible=0
for sock in "$SOCK_DIR"/*.sock; do
    [ -e "$sock" ] || continue
    pid=$(basename "$sock" .sock)

    if [ ! -d "/proc/$pid" ]; then
        stale=$((stale + 1))
        printf '%-9s %-4s %-13s %-6s %-40s %-24s %s\n' \
            "$pid" "" "?" "?" "" "" "stale socket (no process)"
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

    prof=$(profile_of "$pid")
    # Would the CALLER's ListAgents show this row? It lists its own profile's
    # registry minus itself, so: same profile and not us.
    if [ "$pid" = "$self_pid" ]; then
        vis="self"
    elif [ "$prof" = "?" ] || [ "$self_profile" = "?" ]; then
        vis="?"
    elif [ "$prof" = "$self_profile" ]; then
        vis="yes"
        visible=$((visible + 1))
    else
        vis="NO"
        blind=$((blind + 1))
    fi

    printf '%-9s %-4s %-13s %-6s %-40s %-24s %s\n' \
        "$pid" "$mark" "$prof" "$vis" "$cwd" "$started" "$comm"
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

# The payoff line. Counted from the rows rather than derived by arithmetic, so it
# cannot drift when a row lands in the "?" bucket that is neither visible nor blind.
echo
if [ "$self_profile" = "?" ]; then
    echo "Could not read this session's own profile, so the LISTED column is unknown"
    echo "rather than false. Do not read the blanks as \"visible\"."
else
    # `visible` and `blind` are incremented AFTER the filter's `continue`, so under a
    # filter they count matching rows only. Pairing them with $live would state a
    # filtered numerator against an unfiltered denominator — a count answering a
    # different question than the sentence it sits in, which is the exact defect this
    # script exists to expose. Measured 2026-08-31: it read "2 of the 7" when the
    # scope held 5.
    if [ -n "$FILTER" ]; then
        scope_n="$matched"
        scope_word="session(s) with cwd = $FILTER"
    else
        scope_n="$live"
        scope_word="live session(s)"
    fi
    echo "ListAgents in THIS session (profile $self_profile) shows $visible of the $scope_n"
    echo "$scope_word. $blind are invisible to it — the LISTED=NO rows — and it reports its"
    echo "short count as complete. The blind half is ENUMERABLE, not unknowable: that is the"
    echo "whole difference this column buys you."
fi
echo
echo "Any pid above is addressable whether or not ListAgents lists it:"
echo "  SendMessage(to: \"uds:$SOCK_DIR/<pid>.sock\", …)"
echo
echo "This bounds the POPULATION. It does not attribute a write — eliminating over"
echo "a complete set is still elimination. To attribute, ask, or find a positive"
echo "identification; do not infer authorship from who else was present."
