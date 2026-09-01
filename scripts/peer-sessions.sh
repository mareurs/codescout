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
# See docs/issues/archive/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md
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

# Which BINARY a process is running, and whether that binary has since been
# replaced. `readlink /proc/<pid>/exe` appends a literal " (deleted)" once the
# inode has been unlinked, which is exactly what a rebuild or a CC upgrade does.
#
# THAT SUFFIX IS THE ANSWER. Do not compare process start time against binary
# mtime, which was the fix originally proposed for this: it is a proxy for an
# event the script cannot observe (*did this process load the current bytes?*)
# and it is wrong in both directions — a process started between the build
# finishing and the rename completing reads fresh and is not, and a
# byte-identical rebuild reads stale and is not. Worse, it FAILS OPEN in exactly
# the case it targets: `stat` on the "(deleted)" string errors, the comparison
# gets an empty operand, `[ N -lt "" ]` errors to stderr and evaluates false,
# and the stale row prints nothing at all. Measured 2026-09-01 against pid
# 997544. See docs/issues/archive/2026-08-31-peer-sessions-never-compares-start-time-to-build-time.md.
binary_state() {
    _bs=$(readlink "/proc/$1/exe" 2>/dev/null) || { echo "?"; return; }
    [ -z "$_bs" ] && { echo "?"; return; }
    case "$_bs" in
        *" (deleted)") echo "REPLACED" ;;
        *)            echo "current" ;;
    esac
}

# The identifying part of a binary's path. For `claude` this is the version
# directory (2.1.252) — the axis that actually varies between sessions, and one
# worth printing: six distinct CC versions were live in this checkout on
# 2026-09-01. For codescout it is always "codescout", so callers print the state
# word alone rather than a name that carries no information.
binary_name() {
    _bn=$(readlink "/proc/$1/exe" 2>/dev/null) || { echo "?"; return; }
    [ -z "$_bn" ] && { echo "?"; return; }
    basename "${_bn% (deleted)}"
}

# The codescout MCP server is a CHILD of the session process, so its freshness is
# a DIFFERENT question from the session's own — and it is the one that moves,
# because codescout is rebuilt many times a day while `claude` upgrades rarely.
# The originally proposed fix read /proc/<session>/exe only, which answers the
# question that almost never changes and leaves the one that does invisible.
# Measured 2026-09-01: 5 of 11 codescout servers held replaced inodes while every
# other instrument — this script, ListAgents, and a correct ~/.cargo/bin symlink
# — reported them healthy.
#
# One pass over /proc rather than one scan per session. Processes exit while the
# glob is being walked, so every read is guarded: an unreadable entry is a race,
# not an error, and letting it reach stderr would put noise above the table.
declare -A CS_OF_SESSION
for _stat in /proc/[0-9]*/stat; do
    _cp=${_stat#/proc/}; _cp=${_cp%/stat}
    [ -r "/proc/$_cp/comm" ] || continue
    [ "$(tr -d '\0' < "/proc/$_cp/comm" 2>/dev/null || true)" = "codescout" ] || continue
    [ -r "$_stat" ] || continue
    _cpp=$(awk '{print $4}' "$_stat" 2>/dev/null || true)
    [ -n "$_cpp" ] && CS_OF_SESSION[$_cpp]=$_cp
done

self_profile="?"
if [ -n "$self_pid" ]; then
    self_profile=$(profile_of "$self_pid")
fi

printf '%-9s %-4s %-13s %-6s %-40s %-24s %s\n' PID SELF PROFILE LISTED CWD STARTED BINARIES
live=0
stale=0
matched=0
blind=0
visible=0
replaced=0
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

    # Two binaries, two independent freshness questions. `cc` is the Claude Code
    # version this session runs; `cs` is the codescout MCP server it talks to.
    # They move on different clocks — codescout is rebuilt many times a day,
    # `claude` upgrades rarely — which is why reading only the session's own exe
    # answers the question that almost never changes.
    cc_ver=$(binary_name "$pid")
    cc_st=$(binary_state "$pid")
    [ "$cc_st" = "current" ] && cc_show="cc $cc_ver" || cc_show="cc $cc_ver $cc_st"
    cs_pid="${CS_OF_SESSION[$pid]:-}"
    if [ -n "$cs_pid" ]; then
        cs_show="cs $(binary_state "$cs_pid")"
    else
        cs_show="cs none"
    fi
    binaries="$cc_show / $cs_show"
    case "$binaries" in *REPLACED*) replaced=$((replaced + 1)) ;; esac

    printf '%-9s %-4s %-13s %-6s %-40s %-24s %s\n' \
        "$pid" "$mark" "$prof" "$vis" "$cwd" "$started" "$binaries"
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
if [ "$replaced" -gt 0 ]; then
    echo "$replaced of the $matched row(s) above run a binary that has since been REPLACED —"
    echo "the process holds an inode that no longer exists at that path. A rebuild or a CC"
    echo "upgrade does that, and nothing else reports it: start time, cwd and a correct"
    echo "~/.cargo/bin symlink all read healthy. Such a session's numbers are evidence about"
    echo "the build it LOADED, not about the tree on disk — the same discipline the note below"
    echo "applies to authorship, extended to the other thing a peer report invites you to infer."
else
    echo "No row above runs a replaced binary. Note what that does and does not say: it means"
    echo "each process still holds the inode at its path, not that the path holds current"
    echo "source — an unbuilt commit is invisible to this check."
fi

echo
echo "Any pid above is addressable whether or not ListAgents lists it:"
echo "  SendMessage(to: \"uds:$SOCK_DIR/<pid>.sock\", …)"
echo
echo "This bounds the POPULATION. It does not attribute a write — eliminating over"
echo "a complete set is still elimination. To attribute, ask, or find a positive"
echo "identification; do not infer authorship from who else was present."
