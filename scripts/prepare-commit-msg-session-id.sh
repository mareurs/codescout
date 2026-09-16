#!/usr/bin/env bash
#
# Stamp the Claude Code session id into every commit made by a session.
#
# WHY THIS EXISTS
# ---------------
# Several Claude Code sessions work this checkout at once. Every one of them commits
# as the SAME git author, from the SAME working tree, seconds apart. So `%an`, commit
# adjacency and the dirty-file list are constant across sessions BY CONSTRUCTION —
# they carry zero ownership signal, and no amount of care extracts one from them.
#
# Measured 2026-09-01: five misattributions across three sessions in one evening.
# Every one was resolved by ASKING a peer; not one by inference. Two of them were made
# inside a message correcting the previous one. That is
# docs/trackers/issue-clusters.md IC-10 (`authorship-unrecoverable-after-the-fact`),
# which until this hook had `Mechanism status: none yet`.
#
# One line in the commit makes the question answerable from the artifact itself,
# forever, with no registry and no live session to ask.
#
# MECHANISM, measured 2026-09-01
# ------------------------------
# `CLAUDE_CODE_SESSION_ID` reaches a git hook. Verified through git itself:
#
#     git -c 'alias.x=!echo $CLAUDE_CODE_SESSION_ID' x   ->  c0ab9bc4-...
#
# It is INJECTED per child spawn rather than inherited: it is absent from the claude
# process's own /proc/<pid>/environ and present in every child. That is the stronger
# property — a hook spawned at commit time gets the CURRENT conversation id, so it
# does not suffer the `/clear` staleness that forced codescout to build the
# rendezvous mechanism in src/server.rs for its long-lived MCP subprocess.
#
# WHAT IS DELIBERATELY NOT RECORDED
# ---------------------------------
# The profile (`$CLAUDE_CONFIG_DIR`), the pid, and the messaging socket are all
# available here and all omitted on purpose.
#
#   - The profile is a personal home path. tests/committed_paths.rs fails the build
#     for one in a script; a commit message is the same leak into a surface that test
#     does not scan. CLAUDE.md already holds that per-machine paths must not be
#     committed, because they read as FALSE to anyone on another machine.
#   - The pid is recyclable, and — measured 2026-09-01 — it is not even present on
#     half the commit paths in this repo: `CLAUDE_PID` is set in native Bash and
#     UNSET in codescout's `run_command`, while `CLAUDE_CODE_SESSION_ID` is identical
#     in both. A hook keyed on the pid would have stamped an empty field for every
#     codescout-side commit while working perfectly wherever it was tested from Bash.
#     The socket dies with the session. Both are rotting pointers in a permanent
#     record; one of them is also intermittently absent.
#
# The uuid alone answers the question that actually failed — "is this hunk from my
# session or another?" — and it answers it without any lookup at all. Resolving an id
# to a human name stays a local scan of ~/.claude*/sessions/*.json for the matching
# `sessionId` field, needed only while a session is live, which is exactly when you
# can just ask it.
#
# SCOPE LIMIT, stated rather than glossed
# ---------------------------------------
# A subagent inherits its parent window's CLAUDE_CODE_SESSION_ID (measured: the only
# distinguishing marker is CLAUDE_CODE_CHILD_SESSION=1). So a commit made from a
# subagent carries the PARENT window's id. That is the intended granularity here —
# the unit of ownership is the window, because that is the unit that shares a working
# tree — but it means this hook does not distinguish parent from subagent.
#
# A commit from a plain terminal has no such variable and gets NO trailer, rather than
# a guessed one. Absence is honest; a default would be a fabricated owner.

set -uo pipefail

msg_file="${1:-}"
[ -n "$msg_file" ] || exit 0
[ -f "$msg_file" ] || exit 0

session_id="${CLAUDE_CODE_SESSION_ID:-}"
[ -n "$session_id" ] || exit 0

# `interpret-trailers` rather than an append: this repo's commits already carry a
# Co-Authored-By trailer, and appending after a blank line would open a SECOND
# trailer block, which `git log --format='%(trailers:key=...)'` does not read as one.
# `--if-exists doNothing` is what makes re-runs idempotent, so --amend, rebase and
# squash never accumulate duplicates.
git interpret-trailers \
    --in-place \
    --if-exists doNothing \
    --trailer "Session-Id: $session_id" \
    "$msg_file"

# THE OTHER AUTHORS, WRITTEN RATHER THAN PASTED.
#
# `scripts/pre-commit-foreign-index.sh` admits a commit carrying another session's staged
# paths when CODESCOUT_INDEX_ACK names every one of their sids, and it PRINTS the matching
# `Co-Authored-Session-Id:` trailers for the committer to paste. Pasting is where it broke:
# git parses ONLY the message's final paragraph as trailers, so a line placed above an
# existing `Co-Authored-By` block is readable prose and invisible to every query. Three
# instances are filed in
# docs/issues/2026-09-06-a-hand-written-trailer-above-the-final-block-defeats-if-exists-donothing.md,
# and in one of them (`7a986ac3`) a third party's content was committed with no attribution
# surface at all -- `git blame` hands an auditor one session, confidently, and nothing in
# the commit contradicts it.
#
# So the hook writes them. The paste step is removed rather than documented better.
#
# ORDER IS WHAT MAKES THIS SOUND, and it is measured: git fires pre-commit BEFORE
# prepare-commit-msg. By the time this runs, the guard has already refused unless the ack
# named EVERY foreign owner -- so the list below is one the guard validated as complete, on
# a commit it let through. This hook is not deciding anything; it is recording a decision
# already made and already checked.
#
# A SEPARATE INVOCATION, and the flag is the reason rather than tidiness. `--if-exists` is
# global to one `interpret-trailers` call, and the two keys need opposite policies:
#
#   Session-Id            doNothing       single-valued, and on a rebase replaying a peer's
#                                         commit `addIfDifferent` would ADD ours beside
#                                         theirs -- inventing a second author.
#   Co-Authored-Session-Id addIfDifferent MULTI-valued. Measured: with `doNothing`, two
#                                         distinct sids write ONE trailer -- every co-author
#                                         after the first is silently dropped, which is the
#                                         exact failure this block exists to prevent.
#                                         `addIfDifferent` writes both and stays idempotent
#                                         across --amend, rebase and squash (verified: a
#                                         second pass still yields two, not four).
#
# Absent or empty ack -> nothing written, not an empty trailer. A commit with no other
# authors must look like one.
index_ack="$(printf '%s' "${CODESCOUT_INDEX_ACK:-}" | tr -d '[:space:]')"
if [ -n "$index_ack" ]; then
    ack_trailers=()
    while IFS= read -r _sid; do
        [ -n "$_sid" ] || continue
        # `-` is the stage log's UNATTRIBUTED sentinel, never a session. The guard refuses
        # an ack naming it, so reaching here with one means the ack was set by hand on a
        # commit the guard did not gate. Recording `Co-Authored-Session-Id: -` would put a
        # co-author that does not exist into a permanent record.
        [ "$_sid" = "-" ] && continue
        ack_trailers+=(--trailer "Co-Authored-Session-Id: $_sid")
        # `printf '%s\n'`, NOT `printf '%s'`. Without the trailing newline `read` returns
        # non-zero on the final element and the loop body never runs for it, so the LAST
        # sid is silently dropped -- and with one sid that is the only sid. Measured: it
        # made a two-element ack look like the sentinel filter working, because the element
        # being dropped happened to be the one the filter was meant to drop.
    done < <(printf '%s\n' "$index_ack" | tr ',' '\n')

    if ((${#ack_trailers[@]})); then
        git interpret-trailers \
            --in-place \
            --if-exists addIfDifferent \
            "${ack_trailers[@]}" \
            "$msg_file"
    fi
fi
