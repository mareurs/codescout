#!/usr/bin/env bash
#
# worktree-remove.sh — find and classify the processes that hold a linked git worktree,
# before anyone removes it.
#
# Usage:
#   scripts/worktree-remove.sh [--stop] [--wait SECS] <worktree path>
#
# Output:
#   stdout  one row per holder, `PID<TAB>KIND<TAB>CMDLINE`, ordered mux first, then the
#           other stoppable kinds, then `other`. KIND is one of
#           mux | rust-analyzer | cargo | rustc | sccache | build-output | other
#   stderr  the occupancy warning, refusals, and `no holders under <path>`
#   exit    0 no holders · 2 refused (usage, unregistered path, main checkout, caller's
#           cwd inside, no /proc) · 3 holders exist · 4 --stop, but a blocker exists
#
# WHY THIS EXISTS
#   A worktree that served `symbols` on a Rust file keeps a live
#   `codescout mux ... rust-analyzer` process and its `cargo check` child. After
#   `git worktree remove` deregisters the worktree, `cargo check` writes `target/` again,
#   so git fails with "Directory not empty" and a directory is left behind (bug
#   3dde1548c3e5c310). Nothing stops those processes when a session activates another
#   project. This script lists them, so the operator can stop them first.
#   Design: docs/superpowers/specs/2026-11-12-worktree-removal-helper-design.md
#
# WHY IT CLASSIFIES, AND WHY `other` IS A BLOCKER
#   A mux is keyed by workspace and shared by every session working in that checkout;
#   this script cannot see which session is a client. So stopping holders is never
#   implicit: it needs `--stop` and prints an occupancy warning
#   (docs/conventions/reaching-a-peer-session.md, Step 4: ask the occupant first).
#   Only the processes a worktree's own rust tooling leaves behind are stoppable. Any
#   other holder (an editor, a shell, a test run) is a blocker: with `--stop` the script
#   prints it and exits 4 without signalling anything.
#
# HOW A PROCESS IS MATCHED
#   Holder   its cwd is the worktree or below it, OR an element of its command line
#            holds the worktree path as a whole path component: preceded by the start of
#            the element or by a character that is not part of a path, followed by `/` or
#            the end of the element. `/p/wt`, `/p/wt/Cargo.toml`, `--cwd=/p/wt` match;
#            `/p/wt-other`, `/p/wt2/x`, `/x/p/wt` do not.
#   Kind     the basename of argv[0] from /proc/<pid>/cmdline, never `comm` (which is
#            cut at 15 characters and settable by the process). `codescout` is `mux`
#            only when argv[1] is exactly `mux`; any other codescout (an MCP server) is
#            `other`, wherever its binary lives. `rust-analyzer*`, `cargo`, `rustc` and
#            `sccache` are themselves. Any other argv[0] under `<worktree>/target/` is
#            `build-output`.
#
# WHAT THIS DOES NOT AND CANNOT DO
#   * It cannot see peers. A listed mux may have a live client in another session; a
#     green run says nothing about that.
#   * It sees only processes whose /proc entry this user can read (cwd and cmdline).
#     A holder owned by another user is invisible, so no row is not proof of no holder.
#   * A process that holds the worktree only through an open file descriptor, with a cwd
#     elsewhere and no path in its command line, is invisible.
#   * The path is matched in its canonical spelling. A command line that spells the
#     worktree through a symlink is not matched.
#   * It needs Linux /proc, like scripts/stale-servers.sh; without it the script refuses.
#   * This version only lists and refuses. It never signals a process and never removes
#     anything.
#
# TEST SEAM
#   WORKTREE_REMOVE_PROC (default /proc) points the scan at a fixture tree:
#   `<dir>/<pid>/cwd` (a symlink) and `<dir>/<pid>/cmdline` (NUL-separated arguments),
#   so tests/worktree-remove.sh needs no real process.

set -uo pipefail
export LC_ALL=C
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE

PROC="${WORKTREE_REMOVE_PROC:-/proc}"
STOP=0
WAIT_SECS=30

die() {
    echo "worktree-remove: $*" >&2
    exit 2
}

usage() {
    echo "usage: worktree-remove.sh [--stop] [--wait SECS] <worktree path>" >&2
}

# resolve_worktree PATH: print the canonical path of a registered linked worktree, or
# print the reason on stderr and return 2. The three refusals the design names: the main
# checkout, an unregistered path, and a path that contains the caller's current directory.
resolve_worktree() {
    local arg=${1:-} canon porcelain line p here first=1 is_main=0 found=0
    if [ -z "$arg" ]; then
        echo "worktree-remove: no worktree path given" >&2
        return 2
    fi
    canon=$(realpath -m -- "$arg" 2>/dev/null) || {
        echo "worktree-remove: cannot resolve [$arg]" >&2
        return 2
    }
    # The repository is the one the path belongs to; only a path that no longer exists
    # falls back to the repository of the caller's current directory.
    if [ -d "$canon" ]; then
        porcelain=$(git -C "$canon" worktree list --porcelain 2>/dev/null)
    else
        porcelain=$(git worktree list --porcelain 2>/dev/null)
    fi
    while IFS= read -r line; do
        case $line in
            "worktree "*)
                p=$(realpath -m -- "${line#worktree }" 2>/dev/null) || continue
                if [ "$p" = "$canon" ]; then
                    found=1
                    is_main=$first
                fi
                first=0
                ;;
        esac
    done <<<"$porcelain"
    if [ "$found" -ne 1 ]; then
        echo "worktree-remove: $canon is not a registered worktree of any git repository" >&2
        return 2
    fi
    if [ "$is_main" -eq 1 ]; then
        echo "worktree-remove: $canon is the main checkout, not a linked worktree" >&2
        return 2
    fi
    here=$(pwd -P)
    if [ "$here" = "$canon" ] || [[ $here == "$canon"/* ]]; then
        echo "worktree-remove: the current directory ($here) is inside $canon; run this from elsewhere" >&2
        return 2
    fi
    printf '%s\n' "$canon"
}

# mentions_path TEXT PATH: succeed when TEXT holds PATH as a whole path component.
mentions_path() {
    local text=$1 p=$2 head pre pos end pc nc i=0 len=${#2} n=${#1}
    while :; do
        head=${text:i}
        [[ $head == *"$p"* ]] || return 1
        pre=${head%%"$p"*}
        pos=$((i + ${#pre}))
        end=$((pos + len))
        pc=""
        nc=""
        [ "$pos" -gt 0 ] && pc=${text:pos-1:1}
        [ "$end" -lt "$n" ] && nc=${text:end:1}
        if { [ -z "$pc" ] || [[ $pc != [[:alnum:]._/+~%@-] ]]; } && { [ -z "$nc" ] || [ "$nc" = / ]; }; then
            return 0
        fi
        i=$((pos + 1))
    done
}

# kind_of_argv ARGV0 ARGV1 PATH: print the kind of a process from its command line.
kind_of_argv() {
    local argv0=${1:-} argv1=${2:-} wt=$3 base
    base=${argv0##*/}
    case $base in
        codescout)
            if [ "$argv1" = mux ]; then echo mux; else echo other; fi
            return 0
            ;;
        rust-analyzer*) echo rust-analyzer; return 0 ;;
        cargo) echo cargo; return 0 ;;
        rustc) echo rustc; return 0 ;;
        sccache) echo sccache; return 0 ;;
    esac
    case $argv0 in
        "$wt"/target/*) echo build-output; return 0 ;;
    esac
    echo other
}

# kind_rank KIND: the order rows are printed in, mux first and blockers last.
kind_rank() {
    case $1 in
        mux) echo 1 ;;
        rust-analyzer) echo 2 ;;
        cargo) echo 3 ;;
        rustc) echo 4 ;;
        sccache) echo 5 ;;
        build-output) echo 6 ;;
        *) echo 7 ;;
    esac
}

# list_holders PATH: print one `PID<TAB>KIND<TAB>CMDLINE` row per holder of the worktree.
# An entry whose cmdline cannot be read, or that has neither a cwd nor a path in its
# command line, is skipped. The script's own pid, its subshell and its parent are never
# holders: their command line names the worktree because they were asked about it.
list_holders() {
    local wt=$1 d pid cwd a hit kind cmd
    local -a argv rows=()
    for d in "$PROC"/[0-9]*; do
        [ -d "$d" ] || continue
        pid=${d##*/}
        case $pid in *[!0-9]*) continue ;; esac
        if [ "$pid" = "$$" ] || [ "$pid" = "$BASHPID" ] || [ "$pid" = "$PPID" ]; then
            continue
        fi
        [ -r "$d/cmdline" ] || continue
        cwd=$(readlink -f -- "$d/cwd" 2>/dev/null) || cwd=""
        argv=()
        while IFS= read -r -d '' a || [ -n "$a" ]; do
            argv+=("$a")
        done 2>/dev/null <"$d/cmdline"
        hit=0
        if [ -n "$cwd" ] && { [ "$cwd" = "$wt" ] || [[ $cwd == "$wt"/* ]]; }; then
            hit=1
        else
            for a in "${argv[@]+"${argv[@]}"}"; do
                if mentions_path "$a" "$wt"; then
                    hit=1
                    break
                fi
            done
        fi
        [ "$hit" -eq 1 ] || continue
        if [ "${#argv[@]}" -gt 0 ]; then
            kind=$(kind_of_argv "${argv[0]}" "${argv[1]-}" "$wt")
            cmd="${argv[*]}"
        else
            kind=other
            cmd="(empty command line)"
        fi
        cmd=${cmd//$'\t'/ }
        cmd=${cmd//$'\n'/ }
        rows+=("$(kind_rank "$kind")"$'\t'"$pid"$'\t'"$kind"$'\t'"$cmd")
    done
    if [ "${#rows[@]}" -gt 0 ]; then
        printf '%s\n' "${rows[@]}" | sort -t$'\t' -k1,1n -k2,2n | cut -f2-
    fi
}

main() {
    local path="" wt holders count blockers row pid kind cmd
    while [ $# -gt 0 ]; do
        case $1 in
            --stop) STOP=1 ;;
            --wait)
                [ $# -ge 2 ] || { usage; die "--wait needs a number of seconds"; }
                WAIT_SECS=$2
                shift
                ;;
            --wait=*) WAIT_SECS=${1#--wait=} ;;
            -h | --help) usage; exit 0 ;;
            --)
                shift
                break
                ;;
            -*) usage; die "unknown option: $1" ;;
            *)
                [ -z "$path" ] || { usage; die "only one worktree path may be given"; }
                path=$1
                ;;
        esac
        shift
    done
    if [ $# -gt 0 ]; then
        [ -z "$path" ] && [ $# -eq 1 ] || { usage; die "only one worktree path may be given"; }
        path=$1
    fi
    [[ $WAIT_SECS =~ ^[0-9]+$ ]] || { usage; die "--wait needs a whole number of seconds, got [$WAIT_SECS]"; }
    [ -n "$path" ] || { usage; die "no worktree path given"; }
    [ -d "$PROC" ] || die "cannot scan processes: [$PROC] is not a directory; this script needs Linux /proc"

    wt=$(resolve_worktree "$path") || exit 2

    holders=$(list_holders "$wt")
    if [ -z "$holders" ]; then
        echo "no holders under $wt" >&2
        exit 0
    fi

    printf '%s\n' "$holders"
    count=$(printf '%s\n' "$holders" | wc -l)
    {
        echo "worktree-remove: $count process(es) hold $wt."
        echo "  Each may have an occupant: a codescout mux is shared by every session working in this"
        echo "  checkout, and stopping it ends their language server. Ask the occupant before"
        echo "  stopping anything."
    } >&2

    blockers=""
    while IFS=$'\t' read -r pid kind cmd; do
        if [ "$kind" = other ]; then
            blockers+="  $pid  $cmd"$'\n'
        fi
    done <<<"$holders"

    if [ "$STOP" -eq 1 ]; then
        if [ -n "$blockers" ]; then
            {
                echo "worktree-remove: --stop refused; these holders are not stoppable (nothing was signalled):"
                printf '%s' "$blockers"
            } >&2
            exit 4
        fi
        echo "worktree-remove: --stop: this version does not signal; nothing was signalled." >&2
    fi
    exit 3
}

# Sourced, the file only defines its functions.
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    main "$@"
fi
