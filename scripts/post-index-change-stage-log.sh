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
# The paths a patch FILE names, for `git apply`. `apply`'s positional is the patch, never a
# staged path, so without this the verb sits in staging_op()'s list while argv_paths() feeds
# names_path() a filename that can never match -- every `apply --cached` records `-` and the
# foreign-index guard then refuses the stager's own commit
# (docs/issues/archive/2026-09-01-git-apply-cached-stages-but-records-no-owner.md).
#
# STRICT for the same reason names_path is: only DEFAULT-PREFIX headers are read. A patch
# written with --no-prefix, or applied with -p0, emits nothing and the write degrades to `-`.
# Matching loosely here would let a stray `--- ` line inside a patch's own CONTENT -- a diff
# of a diff, which this repo produces -- name a path the invocation never touched, and that
# is the false hit the whole design is built to avoid.
#
# There is deliberately NO separate bail on -p<n>/--directory, and the absence is the
# considered answer rather than an omission. A first version carried one; mutation testing
# killed zero tests with it removed, and no reachable caller could be named for it. Two
# things already cover the ground it claimed: a --no-prefix patch fails the `a/` match above
# and emits nothing, and where a prefixed patch under an odd -p COULD reach names_path, the
# existing-owner lookup (see the `[ -s "$log" ]` branch in the main loop) resolves the row
# before names_path is consulted at all. A guard nothing reaches is decoration however
# loudly it is written, so it was deleted rather than given a test to justify it.
patch_paths() {
    [ -f "$1" ] || return 0
    awk '
        /^--- a\// || /^\+\+\+ b\// {
            p = substr($0, 5)
            sub(/\t.*$/, "", p)
            # NOT INDEPENDENTLY GUARDED, and that is recorded rather than papered over.
            # Mutation-tested 2026-09-01: deleting this line kills zero tests, because
            # names_path() suffix-matches (`*/$tok`), so an unstripped `a/f.txt` still
            # matches target `f.txt`. No killing case was constructible. It stays because
            # it is a CONTRACT line, not a guard: names_path documents its input as a
            # repo-relative path, and emitting `a/f.txt` would rely on that suffix match to
            # paper over a value that is not one. If names_path ever tightens to exact
            # matching -- the direction its own comment leans -- this line becomes
            # load-bearing with no test to notice it had been removed.
            sub(/^[ab]\//, "", p)
            if (p != "") print p
        }
    ' "$1" 2>/dev/null
}
argv_paths() {
    [ -r "/proc/$PPID/cmdline" ] || return 0
    _ap_seen_git=0
    _ap_seen_verb=0
    _ap_verb=""
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
                    _ap_verb="$_ap_tok"
                    continue
                    ;;
                *) return 0 ;;
            esac
        fi
        case "$_ap_tok" in
            --) continue ;;
            -*) continue ;;
            *)
                if [ "$_ap_verb" = "apply" ]; then
                    patch_paths "$_ap_tok"
                else
                    printf '%s\n' "$_ap_tok"
                fi
                ;;
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
# Why the ROUTE is recorded and not just the owner.
#
# `-` is reachable by four branches and the reason is discarded at write time, so the
# paired guard can only ENUMERATE candidates -- and an enumeration in prose froze while
# this file grew two new routes under it
# (docs/issues/archive/2026-09-02-foreign-index-refusal-names-a-cause-no-route-produces.md).
# A fourth column costs one field and makes the true branch knowable at refuse time,
# which is the difference between listing what MIGHT have happened and reporting what
# did. The enumeration cannot be kept correct by care: it was already short by one route
# on the day it was proposed.
#
# STRICTLY DIAGNOSTIC. The refusal decision still keys on the owner column alone, so a
# mis-recorded route can only produce a wrong EXPLANATION -- never a wrong refusal, and
# never a capture. Keep it that way: the moment a route value gates the refusal, a
# recorder bug becomes a correctness bug on a shared index.
# ROUTE ONLY, NEVER OWNERSHIP: is $1 beneath a path argv actually named?
#
# `names_path` deliberately refuses this match, and must keep refusing it -- claiming a
# subtree would hand a session its peers' files, which is the false hit it exists to
# avoid. For the DIAGNOSTIC field the same fact is safe and is the only thing that
# separates a blanket form (argv named a directory covering this path) from a lost row
# (argv named other files, so this pair was already in the index). Getting this wrong is
# not hypothetical: keying the split on "argv named nothing" instead reported every
# directory add as a lost row, because `git add sub/` DOES put `sub` in argv.
under_named() {
    _un_target="$1"
    [ -n "${_NAMED:-}" ] || return 1
    while IFS= read -r _un_tok; do
        [ -n "$_un_tok" ] || continue
        _un_tok="${_un_tok%/}"
        [ "$_un_tok" = "." ] && return 0
        case "$_un_target" in "$_un_tok"/*) return 0 ;; esac
    done <<EOF
$_NAMED
EOF
    return 1
}
if staging_op; then
    claimant="$me"
    _NAMED="$(argv_paths)"
    # `-` here means the id was absent, which is route 1 and NOT the blanket form.
    [ "$me" = "-" ] && claim_route="id-unset" || claim_route=""
else
    claimant="-"
    _NAMED=""
    claim_route="not-staging"
fi
: > "$tmp" || exit 0
# `--raw` gives ":<srcmode> <dstmode> <srcsha> <dstsha> <status>\t<path>"; field 4 of
# the first tab-separated column is the staged (post-image) blob.
while IFS=$'\t' read -r blob path; do
    [ -n "$path" ] || continue
    owner=""
    route=""
    if [ -s "$log" ]; then
        # $4 is empty on a row written before route recording; awk still emits the tab,
        # so `read` yields an empty route rather than folding the two fields together.
        prior="$(awk -F'\t' -v b="$blob" -v p="$path" \
            '$2 == b && $3 == p { print $1 "\t" $4; exit }' "$log")"
        IFS=$'\t' read -r owner route <<< "$prior"
    fi
    if [ -n "$owner" ]; then
        # Carried over from an existing row: preserve the route that row recorded, and
        # label a pre-route row as such instead of guessing a branch for it.
        [ -n "$route" ] || route="pre-route"
    elif [ "$claimant" != "-" ] && names_path "$path"; then
        owner="$claimant"
        route="named"
    elif [ -z "${_NAMED:-}" ] || under_named "$path"; then
        owner="-"
        # Argv named nothing (a `-A`/`-u` form, a prefix-less patch, or a pathspec-less
        # verb like `stash`), or it named a directory covering this path. Either way the
        # staging command reached this pair on purpose: its own work, almost certainly.
        route="${claim_route:-unnamed}"
    else
        owner="-"
        # The command DID name paths, just not this one -- so this pair was already in the
        # index when it ran and its row was lost or never written. That is the case the
        # recorder cannot attribute, and it is frequently a PEER's: `unnamed` must not be
        # the fallback here, because `unnamed` asserts a blanket add and the guard turns
        # that into "probably your own staging" -- advice that invites the capture this
        # pair exists to prevent. Caught by codescout-0a reviewing the diff; the class is
        # the one this very fix addresses, one level down.
        #
        # Both arms now key on an OBSERVABLE (did argv name anything?) rather than on an
        # inferred cause, and the two are exhaustive -- so a fifth route reaching here
        # reads as the observable that is true of it, never as a wrong specific answer.
        route="${claim_route:-pre-staged}"
    fi
    printf '%s\t%s\t%s\t%s\n' "$owner" "$blob" "$path" "$route" >> "$tmp"
done < <(git diff --cached --raw 2>/dev/null |
    awk -F'\t' '{ split($1, a, " "); print a[4] "\t" $2 }')
# Atomic replace: peers run this hook concurrently against the same file.
mv -f "$tmp" "$log" 2>/dev/null || rm -f "$tmp"
exit 0
