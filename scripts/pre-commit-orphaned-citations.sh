#!/usr/bin/env bash
#
# pre-commit-orphaned-citations.sh — WARN when this commit moves a path that something
# it is NOT committing still cites.
#
# WHY THIS EXISTS
# ---------------
# Archiving a doc here is a git RENAME (docs/issues/foo.md -> docs/issues/archive/foo.md).
# Other markdown cites the moved file by path, usually RELATIVE (`../issues/foo.md`), and
# `id = sha256(abs_path)` means the move also re-keys its 16-hex artifact id. A commit that
# moves the file without also committing the citer leaves HEAD holding citations that
# resolve to nothing.
#
# Measured 2026-09-16 at `215a5cad`: six archive renames landed while the tracker citing
# them stayed uncommitted, so HEAD carried 7 lines of live `../issues/2026-09-13-…`
# citations against paths `git cat-file -e` confirms absent from that tree, plus all six
# PRE-move ids. Repaired minutes later by `53ff4aa0`; nothing consumed the window.
#
# WHY NOTHING ELSE CATCHES IT, verified rather than assumed
# --------------------------------------------------------
#   * `scripts/pre-commit-dead-artifact-ids.sh` scans `git diff --cached --name-only`, i.e.
#     the COMMIT, never its complement — and filters to `ref_kind == "artifact_id"`, so a
#     dead PATH citation is outside it twice over.
#   * CI's `audit-doc-refs` does see the whole tree, but `DEFAULT_HISTORICAL_DIRS` includes
#     `trackers`, so a dead path citation living in a tracker is dropped High->Med and
#     cannot red the job. The measured population above was a tracker.
#   * `doc(action="move")`'s `inbound_path_citations` DOES look at the complement, but it
#     fires at MOVE time, reads the WORKTREE, and has no enforcement leg: read the list,
#     commit only the moved files, and nothing re-asks the question.
#
# WHY A SEPARATE SCRIPT RATHER THAN A BRANCH OF THE FOREIGN-INDEX GUARD
# --------------------------------------------------------------------
# That was the first design and it could not have worked. That guard has FIVE early
# `exit 0` paths — no session id, no git dir, a sequencer stand-down, no stage log, and
# `theirs` empty — and it builds its rename maps AFTER all of them. `215a5cad` was a
# pathspec commit naming only its author's own paths, so `theirs` was empty and the guard
# exited before any rename was looked at; it is recorded as Passed on that commit. A
# warning hosted there would have been unable to fire on the one case it was built for.
# The concern is also simply a different one: this asks whether a MOVE strands a citation,
# which has nothing to do with who staged what.
#
# WARNING, NEVER A REFUSAL, and that is a decision rather than caution.
# The citer may legitimately be a peer's in-flight file, or a deliberate historical mention
# in an archived doc, or a path this scan over-reports (see SCOPE). Refusing would block a
# commit on a judgement this script cannot make. It prints what it found and exits 0.
#
# SCOPE, and it over-reports on purpose
# -------------------------------------
# Keyed on the file STEM, not the full path, mirroring `src/librarian/tools/mv.rs`'s
# `files_mentioning`. Citations here are commonly relative (`../issues/<stem>.md`), so a
# path-anchored grep misses the majority. Over-reporting is the safe direction for a
# warning: a false line costs a glance, a miss costs a broken HEAD nobody is told about.
set -uo pipefail

# Respect a partial commit's temporary index, exactly as the sibling guards do: a pathspec
# commit runs hooks with GIT_INDEX_FILE pointing at `next-index-<pid>.lock`, holding only
# the named paths. Reading the shared index instead would compute the committed set from
# content this commit is not taking.
committed="$(git diff --cached --name-only --diff-filter=ACMRD 2>/dev/null)" || exit 0
[ -n "$committed" ] || exit 0

# Rename SOURCES only. A destination is a path this commit creates; nothing at HEAD can
# cite it yet. `-M` is what makes a rename one row with two paths; without it git reports
# an unrelated add and delete and this script has nothing to read.
sources="$(git diff --cached --name-status -M 2>/dev/null |
    awk -F'\t' '$1 ~ /^R/ { print $2 }')"
[ -n "$sources" ] || exit 0

found=0
{
    while IFS= read -r src; do
        [ -n "$src" ] || continue
        stem="$(basename -- "$src")"
        stem="${stem%.md}"
        [ -n "$stem" ] || continue

        # `git grep ... HEAD` reads the COMMITTED tree, which is the population this check
        # is about: what a reader of the repo sees after this commit lands. The worktree is
        # the wrong subject — a peer's uncommitted edit is not a broken citation.
        citers="$(git grep -l -F -e "$stem" HEAD -- '*.md' 2>/dev/null |
            sed 's/^HEAD://')" || continue

        while IFS= read -r citer; do
            [ -n "$citer" ] || continue
            # The moved file itself is not an orphaned citer — it is the subject.
            [ "$citer" = "$src" ] && continue
            # A citer this commit is ALSO committing is being repointed in the same commit,
            # which is the correct shape and must stay silent. This is the whole
            # discrimination: the check is about the COMPLEMENT of the commit.
            printf '%s\n' "$committed" | grep -qxF "$citer" && continue

            if [ "$found" = "0" ]; then
                found=1
                echo
                echo "note: this commit MOVES a path that files it is not committing still cite."
                echo
            fi
            printf '  %s\n      still cited by: %s\n' "$src" "$citer"
        done <<EOF
$citers
EOF
    done <<EOF
$sources
EOF

    if ((found)); then
        echo
        echo "Not a refusal. The citer may be a peer's in-flight file, a deliberate mention in"
        echo "an archived doc, or a stem this scan over-reports — none of which this check can"
        echo "tell apart, and refusing would block your commit on a judgement it cannot make."
        echo
        echo "What it does mean: if you commit only the moved paths, HEAD carries those"
        echo "citations pointing at a file that is no longer there, and \`id = sha256(abs_path)\`"
        echo "means the artifact id moved too. Both resolve to nothing until someone repoints"
        echo "them. Neither CI nor the other pre-commit checks will say so — audit-doc-refs"
        echo "drops a dead path citation in docs/trackers/** to med, and the dead-id check"
        echo "reads only files you ARE committing."
        echo
        echo "Repoint them in THIS commit, or say in the message who is repointing them next."
        echo
    fi
} >&2

exit 0
