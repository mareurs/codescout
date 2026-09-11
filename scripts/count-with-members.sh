#!/usr/bin/env bash
# count-with-members.sh — break a commit range down by author trailer, in which
# every count arrives together with the commits it counts.
#
# WHY THIS EXISTS
#
# Intervention I-10 in docs/trackers/test-escape-hardening.md, routed from
# bug-fix-session-log:F-133. Twice in one session an author derived a per-commit
# aggregate with the right instrument and then composed the matching membership
# list BY HAND. The second time, the per-member instrument had already been run
# — on one of three columns — and the other two were freehanded. Two commits
# ended up in each other's columns.
#
# WHAT MAKES THAT SHAPE SURVIVE A CAREFUL READER, and the reason this is a tool
# rather than a rule: the COUNT STAYED RIGHT. The two membership errors
# compensated — one commit out of a column, one in — so every count was correct,
# self-consistent, and correct again when re-derived immediately before use. The
# aggregate did not merely fail to catch the wrong list; it actively corroborated
# it. CLAUDE.md § Testing Discipline's "count the LIST, never the corpus" and
# § Reaching a Peer Session's "never route by adjacency" both describe failures
# where number and list are BOTH suspect, and neither predicts one half vouching
# for the other.
#
# So this script has NO count-only mode, and adding one would defeat it. The
# count and the members come from a single pass over a single command; there is
# no supported way to obtain the reassuring number on its own.
#
# WHY GIT'S TRAILER PARSER AND NOT A GREP. `git log --format='%b' | grep Session-Id`
# counts MENTIONS as well as trailers. Measured 2026-09-11 on this repo: that grep
# returned 21 for one session over a range holding 20 of its commits, because
# commit aeab4ee2 — authored by b80a27d4 — discusses another session's id in its
# body. `%(trailers:key=...)` reads the trailer block only, so a body that talks
# about a session does not acquire its authorship. The mention-vs-use distinction
# is the single most common way this repo's attribution answers go wrong.
#
# A commit carrying no such trailer is reported under (none) and NEVER dropped:
# a silent omission is the same defect class this tool exists to remove.

set -u

usage() {
    cat <<'USAGE'
usage: count-with-members.sh [--by <TrailerKey>] <commit-range>

  <commit-range>   anything git log accepts, e.g. origin/experiments..HEAD
  --by <key>       trailer to group by (default: Session-Id)

Prints one line per group:   <count> <value>  <sha> <sha> ...
followed by a total that is checked against the range.

There is deliberately no count-only mode -- see the header.
USAGE
}

BY="Session-Id"
RANGE=""

while [ $# -gt 0 ]; do
    case "$1" in
        --by)
            [ $# -ge 2 ] || { echo "error: --by needs a trailer key" >&2; exit 2; }
            BY="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        --count-only|--count|-c)
            # Named explicitly so the refusal states the reason. A silent
            # "unknown option" would read as a missing feature rather than a
            # deliberate absence, and the next person would add it.
            echo "error: there is no count-only mode, by design." >&2
            echo "  This tool exists because a correct count corroborated a wrong" >&2
            echo "  membership list (I-10 / F-133). Emitting the count alone would" >&2
            echo "  reproduce exactly the artifact that misleads." >&2
            exit 2 ;;
        -*) echo "error: unknown option: $1" >&2; usage >&2; exit 2 ;;
        *)
            [ -z "$RANGE" ] || { echo "error: more than one range given" >&2; exit 2; }
            RANGE="$1"; shift ;;
    esac
done

[ -n "$RANGE" ] || { usage >&2; exit 2; }

git rev-parse --git-dir >/dev/null 2>&1 || { echo "error: not a git repository" >&2; exit 2; }

if ! git rev-list --quiet "$RANGE" -- 2>/dev/null; then
    echo "error: not a valid commit range: $RANGE" >&2
    exit 2
fi

TOTAL=$(git rev-list --count "$RANGE")

if [ "$TOTAL" -eq 0 ]; then
    echo "0 commits in $RANGE"
    exit 0
fi

# One pass. %H and the trailer value come out of the same command, so the
# membership list cannot drift from the count that summarises it.
RAW=$(git log --reverse --format="%H%x09%(trailers:key=${BY},valueonly,separator=%x2C)" "$RANGE")

EMITTED=0
KEYS=$(printf '%s\n' "$RAW" | awk -F'\t' '{ v = $2; if (v == "") v = "(none)"; sub(/,.*/, "", v); print v }' | sort -u)

while IFS= read -r value; do
    [ -n "$value" ] || continue
    members=$(printf '%s\n' "$RAW" | awk -F'\t' -v want="$value" '
        { v = $2; if (v == "") v = "(none)"; sub(/,.*/, "", v);
          if (v == want) printf "%s ", substr($1, 1, 8) }')
    n=$(printf '%s\n' "$members" | wc -w | tr -d ' ')
    EMITTED=$((EMITTED + n))
    printf '%s %s  %s\n' "$n" "$value" "${members% }"
done <<< "$KEYS"

echo "total: $TOTAL commit(s) in $RANGE"

# The tool's own guard. If the members printed do not account for every commit
# in the range, the breakdown above is exactly the artifact this script exists
# to prevent -- so it must fail loudly rather than print a plausible table.
if [ "$EMITTED" -ne "$TOTAL" ]; then
    echo "error: printed $EMITTED member(s) for a range of $TOTAL -- the breakdown above is incomplete and must not be used." >&2
    exit 1
fi
