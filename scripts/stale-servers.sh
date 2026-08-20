#!/usr/bin/env bash
#
# Which running codescout servers are executing a binary that no longer exists on disk?
#
# WHY THIS EXISTS
#   `cargo rb` + `/mcp` makes the new code run in THIS session. It does nothing for every
#   other Claude Code session already holding a codescout server: those keep executing the
#   binary they started from, serving that build's guides, prompt surfaces and guide
#   routing until they are reconnected. Measured 2026-08-21 on this machine: 22 of 26
#   servers stale, oldest 17 days. Nothing raises — the commits are on the branch, the
#   binary on disk is current, the suite is green, and only /proc disagrees.
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
# BLIND SPOTS
#   - Only processes whose /proc this user can read.
#   - `pgrep -x codescout` matches argv[0] exactly: a server launched through a wrapper or
#     a differently-named symlink is invisible here. (~/.cargo/bin/codescout is a symlink
#     to target/release/codescout, so `exe` resolves to the target — that case IS covered.)
#   - Says nothing about WHICH build a stale process runs, only that it is not the current
#     inode. A deleted inode carries no version.
#   - The count is a floor if a server is mid-start.
#
# Reports only; never gates. Always exits 0.

set -uo pipefail

rows=""
total=0
stale=0

for p in $(pgrep -x codescout 2>/dev/null); do
    exe=$(readlink "/proc/$p/exe" 2>/dev/null) || continue
    total=$((total + 1))
    case "$exe" in
        *" (deleted)") flag=STALE;   stale=$((stale + 1)) ;;
        *)             flag=current ;;
    esac
    # Sort key is etimes (elapsed SECONDS, numeric). Deliberately NOT `ps lstart`: that is
    # a "Wed Aug 20 …" string, and sorting it lexically orders by WEEKDAY NAME — which on
    # 2026-08-21 reported two-day-old processes as the newest on a machine whose newest
    # was 17 seconds old (reconnaissance-patterns:R-104).
    etimes=$(ps -o etimes= -p "$p" 2>/dev/null | tr -d ' ')
    ppid=$(ps -o ppid= -p "$p" 2>/dev/null | tr -d ' ')
    started=$(ps -o lstart= -p "$p" 2>/dev/null)
    rows="${rows}${etimes:-999999999}|${p}|${ppid}|${flag}|${started}"$'\n'
done

printf '%-10s %-9s %-9s %-8s %s\n' AGE_SEC PID PPID STATUS STARTED
printf '%s' "$rows" | sort -n -t'|' -k1 | while IFS='|' read -r e pid pp fl st; do
    [ -n "${pid:-}" ] || continue
    printf '%-10s %-9s %-9s %-8s %s\n' "$e" "$pid" "$pp" "$fl" "$st"
done

echo
echo "total=${total}  stale-exe=${stale}  current=$((total - stale))"
if [ "$stale" -gt 0 ]; then
    echo
    echo "A stale server serves the guides, prompt surfaces and guide routing of the build"
    echo "it started from. Reconnect those sessions (/mcp) to pick up the current binary."
fi
exit 0
