#!/usr/bin/env bash
#
# Refuse an INDEX commit that would carry another session's staged paths.
#
# WHY THIS EXISTS
# ---------------
# `git commit` with no pathspec commits the WHOLE index, and a checkout has exactly
# one index shared by every session working it. So `git add <one file>` followed by a
# bare `git commit` takes whatever any peer left staged, under your message.
#
# This is the INDEX-commit twin of scripts/pre-commit-unreviewed-content.sh, which
# guards PATHSPEC commits. Neither covers the other's form, and the two hazards pull
# in opposite directions — the pathspec form is the one that ignores the shared index,
# and it is the form that hook refuses when your content is unstaged. Together they
# cover both, and the safe composition is to satisfy both at once:
#
#     git add <paths>            # staging is what satisfies unreviewed-content
#     git commit -- <same paths> # the pathspec is what ignores the shared index
#
# Measured 2026-09-01, in this repo: a `git add` on one file joined an index that
# already held 16 foreign staged files, and a peer's entire OB-6 promotion was
# committed inside a commit about something else. Both are this guard's case.
#
# WHAT THIS GUARD DOES NOT COVER — and it is the incident people will assume it does
# ------------------------------------------------------------------------------------
# It does NOT catch `d617051b`, the capture that prompted this work. Established by a
# peer review on 2026-09-01 rather than by its author, which is the point.
#
# Two different axes:
#
#   CROSS-path  my index holds YOUR file      -> this guard
#   INTRA-path  my file holds YOUR lines      -> nothing here
#
# In `d617051b` the committing session verified `git diff` on one path, then ran
# `git add` on that same path ~40s later, and a peer wrote INTO that file during the
# window. The path was staged by the committing session, legitimately; the stage log
# records them as its stager; a bare commit sees only their own staged path and this
# guard exits 0. It would have passed. Path ownership was never in dispute — the
# contamination was inside a path both parties agree is yours, so no ownership check
# can see it.
#
# The remedy for that half is a time-of-check/time-of-use guard: record each path's
# blob at `git add`, re-hash at pre-commit, refuse if it moved. Not built here. Say so
# out loud, because the next session to hit that race will otherwise have been told
# this covered them — which is the guard-narrower-than-its-name defect
# (docs/trackers/issue-clusters.md IC-14) shipped inside a guard against capture.
#
# Full record of both shapes:
#   docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md
#
# MECHANISM
# ---------
# scripts/post-index-change-stage-log.sh records `<owner>\t<blob>\t<path>` for every
# staged blob as it appears. This reads that log, keyed on the same (blob, path) pair,
# and refuses when any currently-staged pair is owned by a different session id.
#
# WHAT THIS DOES NOT CATCH
# ------------------------
# Everything the stage log cannot see. Its fail-open race is documented at the top of
# that script and is inherited here unchanged: where the log mis-attributes a peer's
# pair to THIS session, this guard stays SILENT.
#
# Name the direction precisely, because the short form of this sentence is the part
# that travels into a commit message or an IC entry. The mechanism OVER-attributes
# ownership to SELF, and that over-attribution is exactly what produces the silence.
# It under-reports FOREIGN paths, and raises no foreign-path false alarms — which
# sounds like the safe direction and is not, because a missing refusal emits nothing
# while a false alarm would at least print something a reader could act on. So a clean
# run is not proof the index is yours; it is proof that nothing recorded says
# otherwise.
#
# (Corrected 2026-09-01 after peer review. This paragraph read "it under-reports and
# never over-reports", which is true only with "foreign paths" as the referent and
# resolves the reassuring way on a skim. The reviewer's argument for bothering was
# that a short reassuring clause is the one that gets pasted onward, at which point a
# reader concludes the silence is the safe failure and stops looking.)
#
# `git diff --cached --name-only`, read in its own call before you commit, remains the
# check that answers the question directly and depends on none of this.
#
# With no CLAUDE_CODE_SESSION_ID this exits silently rather than refusing. A commit
# from a plain terminal is a deliberate human act, and this guard has no id to
# discriminate with — blocking it would be a false alarm, and a guard that fires on
# ordinary work teaches `--no-verify`, which disarms the quiet one that works.

set -uo pipefail

me="${CLAUDE_CODE_SESSION_ID:-}"
[ -n "$me" ] || exit 0

# A PATHSPEC commit gets a temporary index named `next-index-<pid>.lock` and IGNORES
# the shared index entirely, so it cannot capture staged content and needs no guard.
# This is the same discriminator scripts/pre-commit-unreviewed-content.sh uses, read
# in the opposite direction.
idx="${GIT_INDEX_FILE:-}"
case "${idx##*/}" in
    next-index-*) exit 0 ;;
esac

git_dir="$(git rev-parse --git-dir 2>/dev/null)" || exit 0

# A sequencer stop — a conflicted cherry-pick or merge, or a rebase stopped mid-pick —
# makes this guard's OWN prescribed remedy impossible. git refuses `git commit -- <path>`
# there with "cannot do a partial commit during a cherry-pick", while the bare form
# refused below is the only one it will accept. Refusing here leaves no compliant route
# at all, which is what teaches `--no-verify`. See
# docs/issues/2026-09-02-foreign-index-prescribes-a-remedy-git-refuses.md.
#
# Keyed on the sequencer HEADs, NOT on "a rebase is running": measured 2026-09-02, a
# rebase stopped with rebase-merge/ present and CHERRY_PICK_HEAD absent commits by
# pathspec fine, so the wider test would stand the guard down while its remedy still
# works. Asked via `--git-path` rather than "$git_dir/..." so git decides per-worktree
# vs common itself — both resolve per-worktree today, and only one stays right if that
# ever changes.
#
# This is a stand-down, not a hole: the guard still refuses in ordinary work, which is
# the case the fix could most easily have broken. All four arms are covered by
# tests/hooks-discrimination.sh § 7, whose `no sequencer -> still refuses` case is the
# one that fails if this is ever widened to an unconditional exit.
if [ -e "$(git rev-parse --git-path CHERRY_PICK_HEAD)" ] ||
   [ -e "$(git rev-parse --git-path MERGE_HEAD)" ]; then
    exit 0
fi

log="$git_dir/session-stage-log"
[ -s "$log" ] || exit 0

# Resolve a session id to a LIVE session, printing "<pid>|<name>" or nothing.
#
# Without this the refusal can only DESCRIBE the dead-incarnation case while being
# unable to detect one, which is a gate keyed on an event it cannot observe — the very
# class this repo tracks as IC-2, shipped inside a guard against capture.
#
# The registry record carries `sessionId` alongside `pid` and `name`, and the file is
# named for the pid, so this is a direct lookup rather than a scan of anything. The
# `comm` check guards pid reuse the same way scripts/peer-sessions.sh does: a recycled
# pid running something else must not be reported as a live peer.
resolve_session() {
    _rs_id="$1"
    for _rs_f in "$HOME"/.claude*/sessions/*.json; do
        [ -f "$_rs_f" ] || continue
        grep -q "sessionId\"[[:space:]]*:[[:space:]]*\"$_rs_id\"" "$_rs_f" 2>/dev/null || continue
        _rs_pid="$(basename "$_rs_f" .json)"
        [ -r "/proc/$_rs_pid/comm" ] || continue
        [ "$(tr -d '\0' < "/proc/$_rs_pid/comm")" = "claude" ] || continue
        _rs_name="$(sed -n 's/.*"name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$_rs_f")"
        printf '%s|%s\n' "$_rs_pid" "${_rs_name:-?}"
        return
    done
}

mine=()
theirs=()
foreign_owners=()

while IFS=$'\t' read -r blob path; do
    [ -n "$path" ] || continue
    owner="$(awk -F'\t' -v b="$blob" -v p="$path" \
        '$2 == b && $3 == p { print $1; exit }' "$log")"
    if [ -n "$owner" ] && [ "$owner" != "$me" ]; then
        theirs+=("$path")
        case " ${foreign_owners[*]-} " in
            *" $owner "*) ;;
            *) foreign_owners+=("$owner") ;;
        esac
    else
        mine+=("$path")
    fi
done < <(git diff --cached --raw 2>/dev/null |
    awk -F'\t' '{ split($1, a, " "); print a[4] "\t" $2 }')

((${#theirs[@]})) || exit 0

{
    echo
    echo "Refusing a bare commit: the index holds paths staged by another session."
    echo
    echo "  theirs:"
    for path in "${theirs[@]}"; do
        echo "      $path"
    done
    if ((${#mine[@]})); then
        echo
        echo "  yours:"
        for path in "${mine[@]}"; do
            echo "      $path"
        done
    fi
    echo
    echo "\`git commit\` with no pathspec commits the WHOLE index, and this checkout"
    echo "shares one index across every session working it. Committing now would file"
    echo "their work under your message, where it is durable and no longer theirs to"
    echo "attribute."
    echo
    echo "Commit your own paths by pathspec — that form ignores the shared index:"
    echo
    if ((${#mine[@]})); then
        echo "    git commit -- ${mine[*]}"
    else
        echo "    git commit -- <your paths>   # <- none of the staged paths look like yours"
    fi
    echo
    echo "Leave theirs staged; it is not yours to unstage either. \`git reset\` here"
    echo "would take their work out of the index seconds before they commit it."
    echo
    echo "Staged by:"
    for owner in "${foreign_owners[@]-}"; do
        if [ "$owner" = "-" ]; then
            echo "      (unrecorded) — staged before this guard was installed, so no"
            echo "          session claimed it. Unknown is deliberate: it over-refuses"
            echo "          until the pair churns out, where claiming it would have gone"
            echo "          silent. Find the owner by asking, not by assuming it is yours."
            continue
        fi
        info="$(resolve_session "$owner")"
        if [ -n "$info" ]; then
            echo "      $owner"
            echo "          LIVE — ${info#*|} (pid ${info%%|*})"
            echo "          SendMessage(to: \"uds:/run/user/$(id -u)/cc-socks/${info%%|*}.sock\")"
        else
            echo "      $owner"
            echo "          NOT LIVE — a dead incarnation, not an abandoned file."
            echo "          Compaction and resume mint a NEW id for the same agent doing"
            echo "          the same work, so the owner is often alive under a different"
            echo "          one. Read \$HOME/.claude*/projects/<encoded>/$owner.jsonl"
            echo "          before concluding anything, and do not read an unreachable id"
            echo "          as permission to take the file."
        fi
    done
    echo
    echo "ASK before assuming. scripts/peer-sessions.sh lists every live session,"
    echo "including the ones ListAgents hides from you."
    echo
    echo "\`--no-verify\` also works and is the wrong habit."
    # Forward reach — see scripts/commit-sequence-tail.txt. This hook's rule is step 4's
    # consequence; the rest of the sequence is what stops the next collision rather than
    # this one. Single emitted copy shared by all three refusing hooks.
    _tail="$(dirname "$0")/commit-sequence-tail.txt"
    [ -r "$_tail" ] && { echo; cat "$_tail"; }
} >&2

exit 1
