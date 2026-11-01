# scripts/lost-stage.sh — sourced, not run. The one predicate behind the LOST-STAGE note.
#
# A session's staged pair (blob, path) can leave the shared index two ways: it was committed,
# or a git command took it out (`git restore --staged`, `git reset`). Only the second loses
# work, and only silently from a bare commit or scripts/commit-mine.sh
# (docs/issues/2026-09-02-staging-is-not-a-state-you-can-hold.md, 8941001da3c21636). A
# peer's `git add -A` over your blob, or a `git rm --cached`, also removes your pair, but
# leaves a pair of THEIRS at the path, which the foreign-index guard already reports.
#
# The recorder (post-index-change-stage-log.sh) writes a note when an owned pair leaves and
# this predicate says it is NOT settled; the guard (pre-commit-foreign-index.sh) asks it
# again before refusing, because a note can outlive its truth: a pathspec commit puts the
# blob in HEAD while the recorder, seeing only its temporary index, writes nothing. One
# predicate for both, so the two readers cannot drift.
#
# WHY CONTENT AND NOT THE COMMAND. Measured 2026-11-01: `git commit -- a` fires
# post-index-change twice with `a` still staged, and `a` leaves the staged set only at the
# next index write, under whatever command that is. So the command line at observation
# time cannot tell a commit from an unstage. Whether HEAD now holds the blob can.

# lost_stage_settled <blob> <path>
#   1 (a SILENT loss) only when the path is not staged at all any more — its shared-index
#     entry equals HEAD's, both absent included — and that entry is not the owner's blob.
#   0 otherwise, one line each:
#     - the index entry differs from HEAD's: the path is still staged with other content. A
#       pair someone else staged sits there (a peer's `git add -A` over your blob, or a
#       `git rm --cached`), and pre-commit-foreign-index.sh already refuses the bare commit
#       over it, and commit-mine.sh prints it as "not yours". Measured 2026-11-01: noting it
#       here too gave a refusal that said "would land without it" of a path that was in the
#       index, and that re-staging could not clear. This line also covers your own blob still
#       staged.
#     - a staged deletion (an all-zero blob) whose path is in neither: the deletion was committed.
#     - the common entry IS the blob: it was committed.
# `<blob>` may be abbreviated, as `git diff --raw` prints it, so it is matched as a prefix.
lost_stage_settled() {
    local b="$1" p="$2" idx head
    [ -n "$b" ] && [ -n "$p" ] || return 0
    # The SHARED index, whatever the caller exported: commit-mine commits from a private one,
    # and the pair went missing from the shared one.
    idx="$(env -u GIT_INDEX_FILE git --literal-pathspecs ls-files -s -- "$p" 2>/dev/null)"
    idx="${idx#* }"
    idx="${idx%% *}"
    head="$(git rev-parse -q --verify "HEAD:$p" 2>/dev/null)"
    [ "$idx" != "$head" ] && return 0
    case "$b" in
        *[!0]*) ;;
        *) [ -z "$idx" ] && return 0 ;;
    esac
    [ -n "$idx" ] && [ "${idx#"$b"}" != "$idx" ] && return 0
    return 1
}
