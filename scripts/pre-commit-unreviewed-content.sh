#!/usr/bin/env bash
#
# Refuse a commit that carries content this session never staged.
#
# WHY THIS EXISTS
# ---------------
# `git commit -- <paths>` does not commit the index. It commits the WORKING TREE at
# those paths. On a shared checkout that means it also commits whatever a concurrent
# session wrote to the same file between your edit and your commit — silently, under
# your commit message.
#
# That is not hypothetical. It has happened four times in this repo, documented in
# docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md. In
# instance 4 the capturing session was actively guarding against the mechanism, used
# path-scoped committing (the remedy that file recommended), and captured a peer's
# complete tracker entry anyway. Detection was luck: `--stat` reported 32 changed
# lines in a file where one cell had been edited, and the author happened to read it.
#
# WHAT IT CHECKS
# --------------
# Only pathspec commits are examined — an ordinary `git add` + `git commit` commits
# the index, which is by definition what you staged and could review. A pathspec
# commit is identified by git handing the hook a temporary index named
# `next-index-<pid>.lock` rather than `.git/index` (verified 2026-08-31, and the
# pre-commit framework preserves the variable).
#
# For each path in such a commit, the blob being committed is compared against the
# blob in the real index. They match only if what is about to be committed is exactly
# what you staged. Any difference means unreviewed content — either your own unstaged
# edit, or a peer's.
#
# WHAT IT DOES NOT CATCH
# ----------------------
# A peer editing a file you staged AND committing it themselves; anything outside the
# paths named in the commit; and any capture in an ordinary index commit, where the
# content was staged and is presumed reviewed. It narrows the window, it does not
# close it — only a per-session worktree does that.
#
# THE INDEX IS SHARED TOO — the remedy below is not complete on its own.
# `git add <paths>` then a bare `git commit` commits the WHOLE index, and on this
# checkout a peer may have staged their own files into it. Measured 2026-08-31: a peer
# found this hook's own two files already staged by another session while they were
# about to commit, and a bare `git commit` would have taken both under their message.
# So confirm the index is yours alone before committing — the check below prints the
# command. This hook cannot enforce that part: at pre-commit time the commit content
# is already decided, and refusing every commit whose index holds foreign paths would
# fire on ordinary sequential work by one session.
#
# READ-SIDE COST, so nobody diagnoses it as data loss
# --------------------------------------------------
# The pre-commit framework stashes unstaged changes while hooks run, and that stash
# covers EVERY session's in-flight work on this checkout, not just the committing
# one's. For the sub-second duration of someone else's commit you may observe your own
# edited file revert to its HEAD content, `git status` report it clean, and `grep` for
# text you just wrote return nothing. `git stash list` is EMPTY throughout, because
# pre-commit writes a patch under ~/.cache/pre-commit rather than using `git stash` —
# so the obvious way to detect a stash says there is not one.
#
# It is not data loss and it clears itself. The danger is REACTING inside the window:
# rewriting a section from memory races the restore and can genuinely lose or duplicate
# work while "recovering" from a problem that has already fixed itself. Do not trust
# `git status` here. For a librarian artifact the oracle is
# `artifact_event(action="list")`, whose field_patch byte counts no git operation
# touches; for any other file it is `wc -c <path>` against `git show HEAD:<path> | wc -c`.
# Reported by a peer session within a minute of these hooks going live, and written up
# as "The read-side twin" in the bug file cited above.

set -uo pipefail

idx="${GIT_INDEX_FILE:-}"
case "${idx##*/}" in
    next-index-*) ;;
    *) exit 0 ;;
esac

gitdir="$(git rev-parse --git-dir)"
real_index="$gitdir/index"
unreviewed=()

while IFS= read -r path; do
    [ -n "$path" ] || continue
    committing="$(GIT_INDEX_FILE="$idx" git rev-parse ":$path" 2>/dev/null || true)"
    staged="$(GIT_INDEX_FILE="$real_index" git rev-parse ":$path" 2>/dev/null || true)"
    [ "$committing" = "$staged" ] || unreviewed+=("$path")
done < <(GIT_INDEX_FILE="$idx" git diff-index --cached --name-only HEAD)

((${#unreviewed[@]})) || exit 0

{
    echo
    echo "Refusing a pathspec commit that carries content you never staged:"
    echo
    for path in "${unreviewed[@]}"; do
        echo "    $path"
    done
    echo
    echo "\`git commit -- <paths>\` commits the WORKING TREE at those paths, not the index."
    echo "On this shared checkout that includes anything a concurrent session wrote to the"
    echo "same file since you last looked. Four such captures are recorded in"
    echo "docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md."
    echo
    echo "Do this instead — it costs two commands and makes the content visible:"
    echo
    echo "    git add ${unreviewed[*]}"
    echo "    git diff --cached --name-only   # <- the index is SHARED: confirm these are all yours"
    echo "    git diff --cached               # <- read the content; that is the whole point"
    echo "    git commit"
    echo
    echo "A bare \`git commit\` commits the WHOLE index, and a peer may have staged into it."
    echo "If it holds paths you did not stage, wait or commit with an explicit pathspec"
    echo "AFTER staging — staging is what satisfies this check, not the pathspec."
    echo
    echo "If you have read the diff and the content is genuinely yours, staging it"
    echo "satisfies this check. \`--no-verify\` also works and is the wrong habit."
    # Forward reach: this hook teaches ITS rule, and every hook here teaches only its
    # own, so the sequence was being learned one collision at a time — nine cross-session
    # messages and ~2h across two sessions for one two-author commit, measured 2026-09-01.
    # Single emitted copy, shared by all three refusing hooks, so the three cannot drift.
    _tail="$(dirname "$0")/commit-sequence-tail.txt"
    [ -r "$_tail" ] && { echo; cat "$_tail"; }
} >&2

exit 1
