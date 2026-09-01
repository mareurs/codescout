#!/usr/bin/env bash
#
# Record WHICH SESSION staged each blob currently in the index.
#
# WHY THIS EXISTS
# ---------------
# A checkout has exactly one `.git/index`, and on this machine several Claude Code
# sessions share it. `git add <path>` writes to that shared index, and a bare
# `git commit` commits the WHOLE index — so a session that stages one file and then
# commits takes everything any peer left staged, under its own message.
#
# Measured 2026-09-01: a `git add` on a single file joined an index that already held
# 16 foreign staged files; a bare `git commit` would have taken all 16. Separately,
# a peer's entire OB-6 promotion was committed this way inside a commit about
# something else. Full record:
#   docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md
#
# This hook does the recording; scripts/pre-commit-foreign-index.sh does the refusing.
# They are separate because the two moments are separate git hooks, and because
# pre-commit.com CANNOT host this one: `post-index-change` is absent from HOOK_TYPES
# in its clientlib.py (verified against the installed 4.6.2). See
# scripts/install-hooks.sh.
#
# MECHANISM, measured 2026-09-01
# ------------------------------
# `post-index-change` fires on every index write (`man githooks`: "invoked when the
# index is written in read-cache.c do_write_locked_index"), which includes `git add`.
# `CLAUDE_CODE_SESSION_ID` reaches a git hook and is injected per child spawn, so the
# value here is the CURRENT conversation id.
#
# Ownership is keyed on the pair (staged blob, path), not on the path alone. A blob is
# the content itself, so a later index write that does not change the content — a
# `git status` stat-refresh, for instance — introduces no new pair and reassigns
# nothing. Rows for pairs no longer staged are dropped on each run, so the log is
# self-pruning and needs no post-commit truncation step.
#
# THE STAGER WINS, NOT THE FIRST OBSERVER. This is the correction that matters.
# `post-index-change` fires on EVERY index write, and measured 2026-09-01 that includes
# `git status --short` as well as `git add`, `git rm --cached` and `git commit`. The
# original rule assigned a new pair to whichever session's hook saw it first — so with
# five sessions polling `git status`, your staged batch was claimed by whoever ran
# `git status` next, not by you. It failed live within two hours: one session came to
# own all 13 staged paths, `src/tools/ast.rs` moved from its real stager to a peer, and
# the guard then passed on a bare commit that swept a staged deletion and broke the build
# (docs/issues/archive/2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md).
#
# The discriminator is `/proc/$PPID/cmdline`: this hook runs as a child of the git
# process that wrote the index, so it can read which operation caused the write.
# Measured, one line per invocation:
#
#     git add a.txt        -> [git add a.txt]
#     git commit -qm base  -> [git commit -qm base]
#     git status --short   -> [git status --short]      <- claims nothing now
#     git rm -q --cached   -> [git rm -q --cached a.txt]
#
# A pair first seen during a NON-staging write is recorded `-` (unknown) rather than
# claimed. Unknown reads as foreign to everyone, so the guard over-refuses until the
# pair churns out — recoverable by reading a message — where claiming it goes silent,
# which is not recoverable because nothing is emitted.
#
# WHAT THIS DOES NOT CATCH — say it here, not at the guard
# -------------------------------------------------------
# There is a fail-open race. If a peer runs `git add` and THIS session's hook fires
# first for an unrelated reason before the peer's own hook does, this session records
# the new pair under its own id, and the guard then stays silent on content that is
# not ours. The window is small — a peer's hook runs as part of the same `git add`
# that created the pair — but it is real, and the failure direction is silence rather
# than a false alarm. The common case measured tonight, a peer staging a batch and
# leaving it sitting, is caught reliably, because those rows are written by the
# peer's own hook at their `git add`.
#
# With no `CLAUDE_CODE_SESSION_ID` the owner is recorded as `-`. That is not a
# fabricated owner: at refuse time `-` is simply not equal to any real session id, so
# terminal-staged content reads as foreign to a session, which is the conservative
# answer. When NOBODY has an id the whole log is `-` and the guard is silent, which is
# honest — without ids there is nothing to discriminate.

set -uo pipefail

# Our own `git diff` can write the index (an opportunistic stat-refresh), which would
# re-enter this hook. Two independent brakes: refuse re-entry, and forbid git from
# taking the optional index lock at all.
[ -n "${CODESCOUT_STAGE_LOG_RUNNING:-}" ] && exit 0
export CODESCOUT_STAGE_LOG_RUNNING=1
export GIT_OPTIONAL_LOCKS=0

git_dir="$(git rev-parse --git-dir 2>/dev/null)" || exit 0
[ -n "$git_dir" ] || exit 0

log="$git_dir/session-stage-log"
tmp="$log.$$"
me="${CLAUDE_CODE_SESSION_ID:--}"

# Did a STAGING operation cause this index write? Only then may we claim a new pair.
# Tokens are NUL-separated; skip argv[0] up to `git`, then skip global flags (`-c`) and
# their `key=value` arguments, and take the first real subcommand. An unreadable or
# unrecognised parent yields "not staging", which downgrades to `-` — the conservative
# direction, since a wrong `-` over-refuses loudly and a wrong claim goes silent.
staging_op() {
    [ -r "/proc/$PPID/cmdline" ] || return 1
    _so_seen_git=0
    _so_skip_value=0
    while IFS= read -r _so_tok; do
        [ -n "$_so_tok" ] || continue
        if [ "$_so_seen_git" = "0" ]; then
            [ "$(basename -- "$_so_tok")" = "git" ] && _so_seen_git=1
            continue
        fi
        if [ "$_so_skip_value" = "1" ]; then
            _so_skip_value=0
            continue
        fi
        case "$_so_tok" in
            # Global flags that consume the NEXT argv token, so the value lands in its
            # own slot. Without this arm the value is classified as the subcommand: a
            # path matches no verb, falls to `*) return 1`, and `git -C <path> add`
            # silently records `-` instead of the stager.
            #
            # SCOPE, measured 2026-09-01 rather than argued: 32 of 1586 real `git add`
            # invocations across this project's session transcripts use `-C` (2.0%), and
            # 0 use the separated `--git-dir <path>` form. Small, not empty -- those 32
            # were mis-attributed, and `-` is recoverable, so the fix is cheap and right.
            #
            # An earlier version of this comment claimed the companion's worktree guard
            # MANDATES `git -C`, making the two guards directly conflict. That was WRONG,
            # and a peer caught it. Probed 2026-09-01: a bare `git add` is NOT blocked by
            # that guard -- it refuses the commit-family verbs (commit/push/reset/rebase/
            # merge/checkout -b). Attribution is recorded at STAGING time, so the
            # compliant path leaves staging bare and attributable, and the mandated `-C`
            # lands on the commit, which claims nothing. The conflict was overstated; the
            # parse bug was not.
            #
            # HAND-MAINTAINED SUBSET, and deliberately so: git exposes no way to ask it
            # which global flags take a value, so this list can only ever be a snapshot.
            # It is safe incomplete. An unlisted value-taking flag leaves its value to be
            # classified below, where it hits `*) return 1` and the pair is recorded `-`
            # -- the OVER-refusing direction this hook already prefers (a987df96):
            # `-` is loud and recoverable, a wrong claim is silent. So a future git flag
            # degrades attribution, never fakes it. Add to this list when one appears.
            #
            # The joined form (`--git-dir=X`) is ONE token and must NOT match here; it
            # falls through to `-*` below, consuming no following token.
            -C | -c | --git-dir | --work-tree | --namespace | --exec-path | --super-prefix)
                _so_skip_value=1
                continue
                ;;
            -*) continue ;;
            *=*) continue ;;
            add | rm | mv | restore | apply | update-index | stash) return 0 ;;
            *) return 1 ;;
        esac
    done < <(tr '\0' '\n' < "/proc/$PPID/cmdline" 2>/dev/null)
    return 1
}

# The pathspec the staging command actually NAMED, one per line.
#
# This is what lets a cold log stay honest. Rebuilding from `git diff --cached --raw` sees
# every staged pair, not just the ones this command touched, so claiming all of them for the
# current writer hands a session its peers' work (measured 2026-09-01: A staged f+g, one hook
# run was missed, B staged h, and B owned all three). Naming is the only signal in the write
# itself that separates "pair I just created" from "pair whose row was lost".
#
# Emits nothing for a blanket form (`-A`, `-u`, `.`, a directory), which is deliberate --
# see names_path.
argv_paths() {
    [ -r "/proc/$PPID/cmdline" ] || return 0
    _ap_seen_git=0
    _ap_seen_verb=0
    _ap_skip=0
    while IFS= read -r _ap_tok; do
        [ -n "$_ap_tok" ] || continue
        if [ "$_ap_seen_git" = "0" ]; then
            [ "$(basename -- "$_ap_tok")" = "git" ] && _ap_seen_git=1
            continue
        fi
        if [ "$_ap_skip" = "1" ]; then
            _ap_skip=0
            continue
        fi
        if [ "$_ap_seen_verb" = "0" ]; then
            case "$_ap_tok" in
                -C | -c | --git-dir | --work-tree | --namespace | --exec-path | --super-prefix)
                    _ap_skip=1
                    continue
                    ;;
                -*) continue ;;
                *=*) continue ;;
                add | rm | mv | restore | apply | update-index | stash)
                    _ap_seen_verb=1
                    continue
                    ;;
                *) return 0 ;;
            esac
        fi
        case "$_ap_tok" in
            --) continue ;;
            -*) continue ;;
            *) printf '%s\n' "$_ap_tok" ;;
        esac
    done < <(tr '\0' '\n' < "/proc/$PPID/cmdline" 2>/dev/null)
}

# Did argv name this repo-relative path? STRICT on purpose, and the asymmetry is the whole
# design: a miss records `-`, which over-refuses and a reader recovers from; a false hit
# claims a peer's file, which under-refuses and nothing is emitted for anyone to recover
# from. `a987df96`'s ruling, applied to the fallback it never reached.
#
# Deliberately NOT matched: a directory prefix. `git add src/` stages every file beneath
# src/ INCLUDING a peer's, so treating the subtree as named would be exactly the false hit
# this function exists to avoid. Blanket forms degrade to `-` and that is correct -- a
# blanket add followed by a BARE commit is the capture this guard exists for, so making it
# loud is the point. The pathspec-commit remedy never reads the shared index and is
# unaffected.
names_path() {
    _np_target="$1"
    [ -n "${_NAMED:-}" ] || return 1
    while IFS= read -r _np_tok; do
        [ -n "$_np_tok" ] || continue
        [ "$_np_tok" = "$_np_target" ] && return 0
        case "$_np_target" in */"$_np_tok") return 0 ;; esac
        case "$_np_tok" in */"$_np_target") return 0 ;; esac
    done <<EOF
$_NAMED
EOF
    return 1
}

if staging_op; then
    claimant="$me"
    _NAMED="$(argv_paths)"
else
    claimant="-"
    _NAMED=""
fi

: > "$tmp" || exit 0

# `--raw` gives ":<srcmode> <dstmode> <srcsha> <dstsha> <status>\t<path>"; field 4 of
# the first tab-separated column is the staged (post-image) blob.
while IFS=$'\t' read -r blob path; do
    [ -n "$path" ] || continue
    owner=""
    if [ -s "$log" ]; then
        owner="$(awk -F'\t' -v b="$blob" -v p="$path" \
            '$2 == b && $3 == p { print $1; exit }' "$log")"
    fi
    if [ -z "$owner" ]; then
        if [ "$claimant" != "-" ] && names_path "$path"; then
            owner="$claimant"
        else
            owner="-"
        fi
    fi
    printf '%s\t%s\t%s\n' "$owner" "$blob" "$path" >> "$tmp"
done < <(git diff --cached --raw 2>/dev/null |
    awk -F'\t' '{ split($1, a, " "); print a[4] "\t" $2 }')

# Atomic replace: peers run this hook concurrently against the same file.
mv -f "$tmp" "$log" 2>/dev/null || rm -f "$tmp"

exit 0
