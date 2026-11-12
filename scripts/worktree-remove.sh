#!/usr/bin/env bash
#
# worktree-remove.sh — remove a linked git worktree safely: find and classify the
# processes that hold it, stop the ones that are stoppable (only with --stop), remove the
# worktree, and verify that nothing is left behind.
#
# Usage:
#   scripts/worktree-remove.sh [--stop] [--wait SECS] <worktree path>
#
# Output:
#   stdout  one row per holder, `PID<TAB>KIND<TAB>CMDLINE`, ordered mux first, then the
#           other stoppable kinds, then `other`. KIND is one of
#           mux | rust-analyzer | cargo | rustc | sccache | build-output | other
#   stderr  the occupancy warning, refusals, `no holders under <path>`, the holders that
#           survive a stop (rows in the same format as stdout), and what git or the
#           verification found
#   exit    0 the worktree is removed and verified gone
#           2 refused (usage, unregistered path, main checkout, caller's cwd inside, no /proc)
#           3 holders exist and --stop was not given
#           4 --stop, but a blocker exists (nothing was signalled)
#           5 --stop, but a holder survived the stop (nothing was removed)
#           6 git refused to remove the worktree (untracked files, a lock): never forced
#           7 git removed it but the directory is still there: listed, never deleted
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
#   prints it and exits 4 without signalling anything. So is this script's own parent
#   when its cwd is inside the worktree, whatever its kind: the script never signals its
#   own pid, its subshells or its parent.
#
# HOW IT STOPS
#   SIGTERM only, never SIGKILL. The mux holders first (a graceful mux exit kills the
#   language server's process group), then, after a pause of at most one second or until
#   the mux is gone, the other stoppable holders. Liveness is a RESCAN of the holders, never
#   `kill -0` (a zombie answers it). The rescan runs about every 0.5 s for up to --wait
#   seconds in all (default 30) and signals what is new, at most three times, because a
#   dying `cargo` can start children. A holder still listed afterwards is a survivor:
#   exit 5, nothing removed. A signal that fails because the process is already gone is
#   not an error; if the rescan still lists it, it is a survivor like any other.
#
# HOW IT REMOVES
#   `git -C <main checkout> worktree remove <path>` without --force (exit 6 when git
#   refuses, with `git status --porcelain --ignored -uall` printed), then
#   `git worktree prune`, then a check that the path is gone (exit 7 when it is not: the
#   contents are listed, never deleted). The branch is not deleted.
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
#   * Between the last scan and `git worktree remove`, a new process can start in the
#     worktree; that surfaces as git's refusal or as exit 7, not as a signal.
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
# command line, is skipped. The script's own pid and its subshell are never holders. Its
# parent is exempt from the command-line test only (its command line names the worktree
# because it launched this script), never from the cwd test: a launcher that sits inside
# the worktree and starts the script from elsewhere holds the worktree like any other.
list_holders() {
    local wt=$1 d pid cwd a hit kind cmd argv_exempt rec
    local -a argv rows=()
    local -A cwds=()
    # On the real /proc one `find` reads every cwd link at once: a `readlink` per process is a
    # fork per process, seconds on a busy machine, and the stop loop rescans. The kernel
    # reports a canonical path, which is what `readlink -f` yields. A process that has no entry
    # (another user's, or started since) has no readable cwd, as before. A fixture tree
    # (WORKTREE_REMOVE_PROC) may hold non-canonical symlinks, so it keeps `readlink -f`.
    if [ "$PROC" = /proc ]; then
        while IFS= read -r -d '' rec; do
            cwds["${rec%%$'\t'*}"]=${rec#*$'\t'}
        done < <(find "$PROC" -mindepth 2 -maxdepth 2 -name cwd -printf '%h\t%l\0' 2>/dev/null)
    fi
    for d in "$PROC"/[0-9]*; do
        [ -d "$d" ] || continue
        pid=${d##*/}
        case $pid in *[!0-9]*) continue ;; esac
        if [ "$pid" = "$$" ] || [ "$pid" = "$BASHPID" ]; then
            continue
        fi
        argv_exempt=0
        [ "$pid" = "$PPID" ] && argv_exempt=1
        [ -r "$d/cmdline" ] || continue
        if [ "$PROC" = /proc ]; then
            cwd=${cwds["$d"]-}
        else
            cwd=$(readlink -f -- "$d/cwd" 2>/dev/null) || cwd=""
        fi
        argv=()
        while IFS= read -r -d '' a || [ -n "$a" ]; do
            argv+=("$a")
        done 2>/dev/null <"$d/cmdline"
        hit=0
        if [ -n "$cwd" ] && { [ "$cwd" = "$wt" ] || [[ $cwd == "$wt"/* ]]; }; then
            hit=1
        elif [ "$argv_exempt" -eq 0 ]; then
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

# main_checkout_of PATH: print the main checkout of the repository PATH belongs to, the first
# entry of `git worktree list --porcelain`. Git commands that remove a worktree run from
# there, never from inside the worktree being removed.
main_checkout_of() {
    local wt=$1 porcelain
    if [ -d "$wt" ]; then
        porcelain=$(git -C "$wt" worktree list --porcelain 2>/dev/null)
    else
        porcelain=$(git worktree list --porcelain 2>/dev/null)
    fi
    porcelain=${porcelain%%$'\n'*}
    [ "${porcelain#worktree }" != "$porcelain" ] || return 1
    printf '%s\n' "${porcelain#worktree }"
}

# set_now_ms: set NOW_MS to the current time in milliseconds.
set_now_ms() {
    local t=${EPOCHREALTIME/./}
    NOW_MS=$((t / 1000))
}

# send_term PID: SIGTERM, never to this script, its subshell or its parent. A failed signal (the
# process is already gone) is not an error and prints nothing; the rescan decides whether the
# holder is still there.
send_term() {
    local pid=$1
    case $pid in "$$" | "$BASHPID" | "$PPID") return 0 ;; esac
    kill -TERM "$pid" 2>/dev/null
    return 0
}

# stop_holders PATH WAIT: SIGTERM the stoppable holders of PATH, the mux first, and rescan until
# none is left. Return 0 when none remains, 5 when something does (the survivors are printed
# on stderr). Liveness is the rescan through list_holders, never `kill -0`.
#   * A pid is signalled once. A holder that appears in a later round (a dying `cargo` can
#     start children) is signalled in that round; at most three rounds signal anything.
#   * A holder of kind `other` is never signalled; one that appears later is a survivor.
#   * The wait is WAIT seconds in all, polled about every 0.5 s.
stop_holders() {
    local wt=$1 wait_s=$2 rows row pid kind cmd round=0 signalled=" " deadline i found
    local -a mux_pids other_pids
    set_now_ms
    deadline=$((NOW_MS + wait_s * 1000))
    while :; do
        rows=$(list_holders "$wt")
        [ -n "$rows" ] || return 0
        mux_pids=()
        other_pids=()
        while IFS=$'\t' read -r pid kind cmd; do
            case $signalled in *" $pid "*) continue ;; esac
            case $pid in "$$" | "$BASHPID" | "$PPID") continue ;; esac
            case $kind in
                other) ;;
                mux) mux_pids+=("$pid") ;;
                *) other_pids+=("$pid") ;;
            esac
        done <<<"$rows"
        if [ $((${#mux_pids[@]} + ${#other_pids[@]})) -gt 0 ] && [ "$round" -lt 3 ]; then
            round=$((round + 1))
            for pid in "${mux_pids[@]+"${mux_pids[@]}"}"; do
                signalled+="$pid "
                send_term "$pid"
            done
            if [ "${#mux_pids[@]}" -gt 0 ]; then
                # Give a graceful mux exit up to one second to take its language server with it.
                for i in 1 2 3 4 5 6 7 8 9 10; do
                    sleep 0.1
                    rows=$(list_holders "$wt")
                    found=0
                    for pid in "${mux_pids[@]}"; do
                        case $'\n'"$rows" in *$'\n'"$pid"$'\t'*) found=1 ;; esac
                    done
                    [ "$found" -eq 1 ] || break
                done
            fi
            for pid in "${other_pids[@]+"${other_pids[@]}"}"; do
                signalled+="$pid "
                send_term "$pid"
            done
        fi
        set_now_ms
        [ "$NOW_MS" -lt "$deadline" ] || break
        sleep 0.5
    done
    # The last scan may predate the last signals: look once more before calling anything a survivor.
    rows=$(list_holders "$wt")
    [ -n "$rows" ] || return 0
    {
        echo "worktree-remove: these holders are still there after SIGTERM and ${wait_s}s (nothing was removed; SIGKILL is never used):"
        printf '%s\n' "$rows"
    } >&2
    return 5
}

# remove_worktree ROOT PATH: `git worktree remove` without --force, `git worktree prune`, then check
# that PATH is gone. Return 0 when it is, 6 when git refuses, 7 when the directory is still
# there. It never deletes anything itself.
remove_worktree() {
    local root=$1 wt=$2 out
    if ! out=$(git -C "$root" worktree remove "$wt" 2>&1); then
        {
            echo "worktree-remove: git refused to remove $wt; nothing was forced:"
            printf '%s\n' "$out" | sed 's/^/  /'
            echo "  git -C $wt status --porcelain --ignored -uall:"
            git -C "$wt" status --porcelain --ignored -uall 2>&1 | sed 's/^/    /'
        } >&2
        return 6
    fi
    git -C "$root" worktree prune >/dev/null || true
    if [ -e "$wt" ] || [ -L "$wt" ]; then
        {
            echo "worktree-remove: git removed the worktree but $wt is still there (something wrote to it afterwards). Its contents, which this script does not delete:"
            ls -A -- "$wt" 2>&1 | sed 's/^/  /'
        } >&2
        return 7
    fi
    echo "worktree-remove: removed $wt" >&2
    return 0
}

main() {
    local path="" wt root holders count blockers row pid kind cmd rc
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
    root=$(main_checkout_of "$wt") || die "cannot find the main checkout of $wt"

    holders=$(list_holders "$wt")
    if [ -z "$holders" ]; then
        echo "no holders under $wt" >&2
        remove_worktree "$root" "$wt"
        exit $?
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
        elif [ "$pid" = "$PPID" ]; then
            blockers+="  $pid  $cmd  (the parent of this script)"$'\n'
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
        stop_holders "$wt" "$WAIT_SECS"
        rc=$?
        [ "$rc" -eq 0 ] || exit "$rc"
        remove_worktree "$root" "$wt"
        exit $?
    fi
    exit 3
}

# Sourced, the file only defines its functions.
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    main "$@"
fi
