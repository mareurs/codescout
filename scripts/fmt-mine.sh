#!/usr/bin/env bash
# fmt-mine.sh — format only the Rust this session wrote, and refuse the rest.
#
# WHY THIS EXISTS
#
# `CLAUDE.md` § *Development Commands* opens the four-command gate with bare
# `cargo fmt`. `cargo fmt` takes no pathspec, no `--staged` and no "only my files"
# mode: its blast radius is every `.rs` in the workspace, which on a shared
# checkout is strictly wider than the set of files the running session authored.
# So the documented gate, FOLLOWED EXACTLY, rewrites other sessions' uncommitted
# work every time anyone runs it, and there is nothing in the documented form to
# defeat.
#
# Measured 2026-09-09: 7 hunks rewritten in a peer's UNTRACKED
# `src/agent/build_check.rs`, where `git checkout` could not have restored it and
# the only copy of the pre-write bytes was the `--check` diff the same command
# happened to print. Cost that time was an interruption, not lost work. That is a
# property of the instance — the same write against a file with an editor buffer
# open is last-writer-wins with no git copy behind it.
# `docs/issues/2026-09-09-the-documented-gates-first-command-rewrites-every-peers-uncommitted-rust.md`
#
# WHY THIS SHAPE RATHER THAN A CHECK YOU REMEMBER TO RUN
#
# The cheap fix is `cargo fmt -- --check && cargo fmt`, and it is genuinely
# better than nothing. It is also a policy: it works only when the person typing
# the gate remembers to type it that way, and the incident above happened to a
# session that HAD read the warning, HAD run `--check` first, and then chained
# the whole gate with `;` so the check gated nothing. `CLAUDE.md` § *Observer
# Blindness* position 3 asks for the correct path to end in a safe state instead.
# This is that: run it as gate step 1 and there is no wrong way to type it.
#
# THE TWO-STAGE COST, WHICH IS THE REASON IT IS AFFORDABLE
#
# Stage 1 is `cargo fmt -- --check`, which the gate pays for anyway. On a tree
# where nothing needs reformatting — the overwhelmingly common case — this script
# exits there, having spent nothing extra. Only when something WOULD be rewritten
# does it pay for stage 2, the transcript scan, which costs several seconds and
# does not cache. Same cheap-question-first design as `scripts/attribute-red.py`,
# for the same reason.
#
# WHAT IT REFUSES TO GUESS
#
# `UNKNOWN` from the provenance scan is never read as "safe to format". Absence
# is a statement about coverage — a Bash write the heuristics miss is
# indistinguishable from no write at all — and reading it as "nobody owns this"
# is the precise substitution the tool's own docstring forbids in the other
# direction. `SHARED` is refused too: a file this session AND a peer have written
# still carries the peer's bytes.
#
# Usage:
#   scripts/fmt-mine.sh            format my files, refuse on anyone else's
#   scripts/fmt-mine.sh --check    report only, write nothing
#
# Exit: 0 nothing to do, or my files formatted.
#       1 refused — files needing format belong to a peer, or are unattributable.
#       2 the scan itself could not run (no session id, `cargo fmt` failed).

set -euo pipefail

CHECK_ONLY=0
[ "${1:-}" = "--check" ] && CHECK_ONLY=1

ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
    echo "fmt-mine: not inside a git repository" >&2
    exit 2
}
cd "$ROOT"

# Overridable so the test suite can stub attribution without a live transcript
# corpus. Same escape `file-provenance.py` gives its own tests via
# FILE_PROVENANCE_*_ROOTS, and for the same reason: a suite that can only run
# against this machine's real profiles is a suite that runs nowhere else.
PROVENANCE="${FMT_MINE_PROVENANCE:-$ROOT/scripts/file-provenance.py}"

# ---------------------------------------------------------------- stage 1
# `cargo fmt -- --check` exits 0 when nothing would change and 1 when something
# would. Any other code is a real failure (unparseable file, missing toolchain)
# and must not be read as "clean" — that would hand back a green with no scan.
set +e
CHECK_OUT=$(cargo fmt -- --check 2>&1)
CHECK_RC=$?
set -e

if [ "$CHECK_RC" -eq 0 ]; then
    exit 0
fi
if [ "$CHECK_RC" -ne 1 ]; then
    echo "fmt-mine: \`cargo fmt -- --check\` failed (exit $CHECK_RC), not a formatting diff:" >&2
    printf '%s\n' "$CHECK_OUT" >&2
    exit 2
fi

# rustfmt prints `Diff in <absolute path>:<line>:` for every file it would touch.
WOULD_CHANGE=$(printf '%s\n' "$CHECK_OUT" \
    | sed -n 's|^Diff in \(.*\):[0-9][0-9]*:$|\1|p' \
    | sed "s|^$ROOT/||" \
    | sort -u)

if [ -z "$WOULD_CHANGE" ]; then
    echo "fmt-mine: \`--check\` reported diffs but named no files — refusing to guess." >&2
    printf '%s\n' "$CHECK_OUT" >&2
    exit 2
fi

# ---------------------------------------------------------------- stage 2
if [ -z "${CLAUDE_CODE_SESSION_ID:-}" ]; then
    # Fail CLOSED and say why. With no session id the provenance scan cannot
    # answer "is this mine", and its verdict for every attributable file
    # degrades to PEER — which would look like a confident refusal rather than
    # an unanswerable question.
    echo "fmt-mine: CLAUDE_CODE_SESSION_ID is not set, so 'mine' has no referent." >&2
    echo "  Files that need formatting:" >&2
    printf '    %s\n' $WOULD_CHANGE >&2
    echo "  If this checkout is yours alone, \`cargo fmt\` is safe and is what you want." >&2
    echo "  If it is shared, ask each owner — scripts/peer-sessions.sh lists them." >&2
    exit 2
fi

# shellcheck disable=SC2086
set +e
PROV=$("$PROVENANCE" $WOULD_CHANGE 2>&1)
PROV_RC=$?
set -e
# `file-provenance.py` returns `1 if unknown == len(paths) else 0` — exit 1 means the
# scan RAN and attributed nothing, which is a verdict, not a failure. Only >1 (a usage
# error, a missing interpreter) is the scan itself failing. Treating any non-zero as
# failure was this script's third remedy-text defect: it refused correctly and told the
# reader "attribution failed", sending them to debug a scan that had worked perfectly and
# produced a careful UNKNOWN explanation the guard then threw away. Stage 1 already reads
# `cargo fmt --check`'s 0/1/other exactly this way; the discipline was applied to cargo
# and not to the tool right beside it.
if [ "$PROV_RC" -gt 1 ]; then
    echo "fmt-mine: attribution could not RUN (exit $PROV_RC); refusing rather than guess." >&2
    printf '%s\n' "$PROV" >&2
    exit 2
fi

# Partitioned with awk on the verdict column, NOT with a sed `s|...|` — the first
# draft used `s|^\(SHARED\|PEER\|UNKNOWN\)...|` where `|` was both the s/// delimiter
# and the intended alternation. sed reads an escaped delimiter as a LITERAL, so the
# pattern matched the string "SHARED|PEER|UNKNOWN" and never fired: NOT_MINE was
# always empty and every refusal fell through to the wrong branch, reporting "nothing
# attributable to this session" instead of naming the owner. The guard still failed
# CLOSED — nothing was written — which is exactly why it survived a passing test of
# the outcome. Caught by asserting on the MESSAGE, not on the exit code.
MINE=$(printf '%s\n' "$PROV" | awk '$1=="MINE"{ $1=""; sub(/^ +/,""); print }')
NOT_MINE=$(printf '%s\n' "$PROV" \
    | awk '$1=="SHARED"||$1=="PEER"||$1=="UNKNOWN"{ $1=""; sub(/^ +/,""); print }')

# The not-mine rows WITH their continuation lines, verdict column intact — unlike
# $NOT_MINE above, which is a bare path list for feeding a command. A continuation line
# never matches a verdict in $1, so it inherits the disposition of the row above it.
NOT_MINE_ROWS=$(printf '%s\n' "$PROV" | awk '
    $1=="MINE" { drop=1; next }
    $1=="SHARED"||$1=="PEER"||$1=="UNKNOWN" { drop=0 }
    !drop')

# Called AFTER this session's own files have been dealt with, never instead of them.
# CLAUDE.md states this script's contract as "formats what scripts/file-provenance.py
# attributes to you AND refuses the rest" — two actions in one run, not a choice between
# them. Refusing wholesale on a mixed scan meant one peer holding one unformatted file
# blocked the gate's mandated step 1 for every other session in the checkout.
refuse_not_mine() {
    echo "fmt-mine: REFUSED — these need formatting and are not this session's to write:" >&2
    # The not-mine rows verbatim: they carry the `uds:` socket and the [LIVE] / not-live
    # marker, which are what make the remedy below performable. MINE rows are filtered OUT
    # rather than printed and then contradicted — listing a file as `MINE` directly beneath
    # a header calling it "not this session's to write" was the second half of this
    # script's own bug file.
    printf '%s\n' "$NOT_MINE_ROWS" >&2
    cat >&2 <<'EOF'

  `cargo fmt` would rewrite them. On a shared checkout that is another session's
  uncommitted work, and for an UNTRACKED file git holds no copy to restore.

  What to do, and which applies depends on what the scan said above:

    [LIVE] peer      ask them to format their own file — the scan printed the
                     `uds:` socket to reach them. They can perform it; you cannot
                     perform it for them without writing their bytes.
    not live         nobody can be asked, and their uncommitted work is abandoned.
                     `cargo fmt` is then a judgement call a human should make, not
                     one this script should make for them.
    UNKNOWN          absence is a statement about COVERAGE, not ownership. A Bash
                     write this tool's heuristics miss looks identical to no write
                     at all, so this is "cannot tell", never "nobody owns it".
    SHARED           this session wrote it AND so did a peer. Their bytes are still
                     in there.

  There is deliberately no --force. A flag that reformats a peer's file on your
  say-so re-admits the exact defect this script exists to prevent, under a
  spelling that reads as deliberate. If you have decided it is safe, run
  `cargo fmt` yourself — that is the same act, minus the false assurance that a
  guard sanctioned it.
EOF
    # Interpolated after the heredoc — which is quoted, so it cannot carry a variable — so
    # the reader gets a command to paste rather than a shape to derive. `cargo fmt` stays
    # named above because it is what CLAUDE.md prescribes and it remains available; but it
    # takes no pathspec and rewrites EVERY `.rs` in the workspace, including files no scan
    # here has looked at. This form touches only the rows printed above, and the script
    # already uses exactly it thirty lines below — offering the wide command while the
    # narrow one was in the file was the third half of this script's own bug file.
    # shellcheck disable=SC2086
    printf '\n  The narrow form, which touches only the files listed above:\n\n    rustfmt --edition 2021 %s\n' \
        "$(printf '%s ' $NOT_MINE)" >&2
}

if [ -z "$MINE" ]; then
    if [ -n "$NOT_MINE" ]; then
        refuse_not_mine
    else
        # Both partitions empty against a non-empty $WOULD_CHANGE means the scan returned a
        # verdict this partition does not name. That is a coverage gap, not a clean tree,
        # and it gets its own message rather than falling through into the formatter.
        echo "fmt-mine: nothing attributable to this session needs formatting." >&2
    fi
    exit 1
fi

# `rustfmt <file>` is NOT per-file. It parses the file and descends into every `mod` the
# file declares, so formatting a module root rewrites its children too. Verified at the
# bytes on rustfmt 1.9.0-stable: `rustfmt --edition 2021 src/lib.rs` rewrote `src/peer.rs`
# through a `pub mod peer;` line, and `--skip-children` is not available outside nightly.
#
# This was invisible while the guard refused every mixed scan, and the reason is worth
# stating because it makes the old wholesale refusal look arbitrary when it was not: a
# peer's child module can only be DAMAGED if it also needs formatting, and a file needing
# formatting is by construction a row in this same scan — so refusing every scan that had
# any not-mine row was accidentally sufficient. Doing only this session's half is safe only
# after asking what this session's half would REACH.
if [ -n "$NOT_MINE" ]; then
    set +e
    # shellcheck disable=SC2086
    REACH_OUT=$(rustfmt --check --edition 2021 $MINE 2>&1)
    REACH_RC=$?
    set -e
    if [ "$REACH_RC" -gt 1 ]; then
        # Same discipline as the provenance scan above: 0 and 1 are verdicts, >1 is the
        # tool failing. Cannot tell what it would reach means cannot write.
        echo "fmt-mine: could not determine what formatting this session's files would reach" >&2
        echo "  (\`rustfmt --check\` exited $REACH_RC). Refusing rather than guess; nothing written." >&2
        printf '%s\n' "$REACH_OUT" >&2
        exit 2
    fi
    REACH=$(printf '%s\n' "$REACH_OUT" \
        | sed -n 's@^Diff in \(.*\):[0-9][0-9]*:$@\1@p' \
        | sed "s@^$ROOT/@@" \
        | sort -u)
    # `-F -x` so a path is matched whole and literally — a substring or regex match here
    # would silently pair `src/a.rs` with `src/ab.rs`.
    COLLATERAL=$(printf '%s\n' "$REACH" \
        | grep -Fx -f <(printf '%s\n' "$NOT_MINE") || true)
    if [ -n "$COLLATERAL" ]; then
        echo "fmt-mine: REFUSED — formatting this session's own files would rewrite files it" >&2
        echo "  does not own, because \`rustfmt\` descends into every \`mod\` a file declares:" >&2
        printf '    %s\n' $COLLATERAL >&2
        echo >&2
        echo "  Nothing was written, this session's files included — there is no way to format" >&2
        echo "  a module root without its children. Ask the owner below to format theirs first," >&2
        echo "  then re-run; this session's half will go through on its own." >&2
        echo >&2
        refuse_not_mine
        exit 1
    fi
fi

if [ "$CHECK_ONLY" -eq 1 ]; then
    # No longer "all attributed to this session" — on a mixed scan it is some of them.
    echo "fmt-mine: would format (attributed to this session):"
    printf '  %s\n' $MINE
    [ -n "$NOT_MINE" ] || exit 0
    refuse_not_mine
    exit 1
fi

# `rustfmt` directly rather than `cargo fmt`, because cargo has no per-file mode
# and formatting the whole workspace is the thing being avoided. `--edition` is
# required: rustfmt defaults to 2015 and would silently mangle 2021 syntax.
# shellcheck disable=SC2086
rustfmt --edition 2021 $MINE
echo "fmt-mine: formatted $(printf '%s\n' "$MINE" | wc -l) file(s) written by this session."

# The refusal comes AFTER the write, not instead of it. Exit stays 1: this session's half
# is done, the gate is still blocked, and the reader's remaining action is the one named
# above — ask the owner. Reporting 0 here would let a caller chaining on success proceed
# past unformatted files it does not own.
if [ -n "$NOT_MINE" ]; then
    refuse_not_mine
    exit 1
fi
