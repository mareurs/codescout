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
    echo "Do this instead — it costs one command and makes the content visible:"
    echo
    echo "    git add ${unreviewed[*]}"
    echo "    git diff --cached          # <- read this; it is the whole point"
    echo "    git commit"
    echo
    echo "If you have read the diff and the content is genuinely yours, staging it"
    echo "satisfies this check. \`--no-verify\` also works and is the wrong habit."
} >&2

exit 1
