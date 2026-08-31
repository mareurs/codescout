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
#   - The pid is recyclable and the socket dies with the session, so both are rotting
#     pointers written into a permanent record.
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
