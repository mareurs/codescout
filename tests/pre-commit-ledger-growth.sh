#!/usr/bin/env bash
# tests/pre-commit-ledger-growth.sh — what CHECK 3 of scripts/pre-commit-ledger-counts.py ACCEPTS.
#
# WHAT THIS GUARDS
#
# CHECK 3 refuses a commit in which a bug file JOINS a defect class (a new file, or a retag)
# unless that bug file carries a `## Class fit` section with at least 40 letters or digits. Until
# 2026-10-16 the same check asked for an entry on the class's shared `**Members:**` line, and it
# tested that with a substring (`docs/issues/2026-10-16-the-growth-gate-accepts-a-bare-stem-while-its-refusal-demands-the-plus-one-shape.md`).
# The ADR that moved the reasoning into the member's own file:
# `docs/adrs/2026-10-16-the-reasoning-for-a-class-member-lives-in-the-members-own-bug-file.md`.
#
# EACH CASE NAMES THE PRODUCTION CHANGE THAT WOULD FAIL IT. They run in two directions on purpose:
# the refuse cases prove the floor is real, and the accept cases prove it is not over-tight. An
# over-tight rule is the quieter failure, because it refuses the filings that were right.
#
# The cases that carry a fixture trap, so a reader does not "simplify" them away:
#   - The hook reads the INDEX, never the working tree. Cases 15 and 16 stage one body and leave
#     another on disk. A hook that read the disk would pass the first and fail the second.
#   - "Prospective" is a rule, not a convenience. Cases 11 and 12 pin that an old bug file with no
#     `## Class fit`, moved or edited, is not refused. Without that the first archive sweep reds.
#
# Fixtures are throwaway git repos under `mktemp -d` — never this checkout, which several sessions
# share. The corpus is the shared synthetic one in `tests/lib/ledger-corpus.sh`, whose three
# traps (ELEVEN classes, LETTERED slugs, roster and class files kept apart) are not re-derived here.
set -uo pipefail

SELF_ROOT=$(cd "$(dirname "$0")/.." && pwd)
# HOOK_UNDER_TEST lets a caller point the suite at another copy of the hook: the pre-switch one, to
# watch every refuse case go red, or a mutant. Unset, it is this checkout's own hook.
SCRIPT="${HOOK_UNDER_TEST:-$SELF_ROOT/scripts/pre-commit-ledger-counts.py}"
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); }
no() { FAIL=$((FAIL + 1)); echo "FAIL: $1"; [ -n "${2:-}" ] && printf '  %s\n' "$2"; }
eq() { [ "$2" = "$3" ] && ok || no "$1" "actual:   $2
  expected: $3"; }
has() { printf '%s' "$2" | grep -qF -- "$3" && ok || no "$1" "missing: $3
  in: $(printf '%s' "$2" | head -6)"; }
hasnt() { printf '%s' "$2" | grep -qF -- "$3" && no "$1" "unexpected: $3" || ok; }

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# shellcheck source=lib/ledger-corpus.sh
source "$SELF_ROOT/tests/lib/ledger-corpus.sh"
CLASS_A="docs/trackers/issue-clusters/IC-1-demo-a.md"

# Fit texts. The counts are checked below with `tr`, not by the hook, so a miscounted literal
# fails the suite at its own door and not inside a case that blames the hook.
FIT40='the selector covers one of the two forms the has x'
FIT39='the selector covers one of the two forms the has'
FIT8='filed by A.'
FIT_LONG='the selector covers one of the two forms the population has, and this bug shows only the second'

alnum() { printf '%s' "$1" | tr -cd '[:alnum:]' | wc -c | tr -d ' '; }
eq "fixture: FIT40 holds exactly 40 letters or digits" "$(alnum "$FIT40")" "40"
eq "fixture: FIT39 holds exactly 39" "$(alnum "$FIT39")" "39"
eq "fixture: FIT8 holds exactly 8" "$(alnum "$FIT8")" "8"
[ "$(alnum "$FIT_LONG")" -ge 40 ] && ok || no "fixture: FIT_LONG is above the floor"

# newrepo [legacy] -> echoes the repo path, corpus COMMITTED. With `legacy`, HEAD also holds an old
# bug file in demo-a that has NO `## Class fit`, which is what every existing bug file looks like.
newrepo() {
    local p="$WORK/repo$RANDOM$RANDOM"
    mkdir -p "$p/docs/trackers/issue-clusters" "$p/docs/issues/archive" "$p/scripts"
    git -C "$p" init -q
    git -C "$p" config user.email t@t
    git -C "$p" config user.name t
    # A user-level `core.autocrlf` would rewrite case 17's CRLF to LF at `git add`, and the case would
    # then test an LF file while its label claims CRLF. Off here; case 17 also checks the blob.
    git -C "$p" config core.autocrlf false
    ledger_write_roster "$p"
    ledger_write_classes "$p"
    cp "$SCRIPT" "$p/scripts/pre-commit-ledger-counts.py"
    [ -f "$SELF_ROOT/scripts/commit-sequence-tail.txt" ] && cp "$SELF_ROOT/scripts/commit-sequence-tail.txt" "$p/scripts/"
    if [ "${1:-}" = legacy ]; then
        write_bug "$p/docs/issues/2026-09-01-old-bug.md" demo-a ""
    fi
    git -C "$p" add -A >/dev/null
    git -C "$p" commit -qm corpus
    echo "$p"
}

# write_bug <path> <class> <fit-block>: an open bug tagged cluster/<class>. An EMPTY <fit-block>
# writes no `## Class fit` heading at all; anything else is written verbatim under that heading
# (so a caller can pass a block that carries its own headings or fences).
write_bug() {
    {
        printf -- "---\nid: 'aaaaaaaaaaaaaaaa'\nkind: bug\nstatus: open\ntitle: x\ntags:\n- cluster/%s\nseverity: low\nopened: 2026-10-16\n---\n\n# BUG: x\n\n## Summary\nx\n" "$2"
        if [ -n "$3" ]; then printf '\n## Class fit\n%s\n' "$3"; fi
    } > "$1"
}

# stage_bug <repo> <stem> <class> <fit-block>: a NEW bug file `2026-10-16-<stem>.md`, staged.
stage_bug() {
    write_bug "$1/docs/issues/2026-10-16-$2.md" "$3" "$4"
    git -C "$1" add "docs/issues/2026-10-16-$2.md"
}

run() { # run <repo> -> sets OUT and RC
    OUT=$(cd "$1" && python3 scripts/pre-commit-ledger-counts.py 2>&1)
    RC=$?
}

# ------------------------------------------------------------ 1: the reasoning in the bug file is enough
# Fails if the rule still asks for an edit to the class's `**Members:**` line.
R=$(newrepo)
stage_bug "$R" new-bug demo-a "$FIT_LONG"
run "$R"
eq "1 a new bug with a Class fit and NO ledger edit is accepted" "$RC" "0"
[ "$RC" -eq 0 ] || printf '  output: %s\n' "$OUT"

# ------------------------------------------------------------ 2: no section is refused, and routed
# Fails if the section is not required at all.
R=$(newrepo)
stage_bug "$R" new-bug demo-a ""
run "$R"
eq "2 a new bug with no Class fit is refused" "$RC" "1"
has "2 the refusal says the section is missing" "$OUT" 'has no `## Class fit` section'
has "2 the refusal names the bug file" "$OUT" "docs/issues/2026-10-16-new-bug.md"
has "2 the refusal names the file that DEFINES the class" "$OUT" "$CLASS_A"
has "2 the refusal says the Members line is no longer the route" "$OUT" "NOT PART OF THIS ANY MORE"
has "2 the refusal still opens with the phrase agents learned" "$OUT" "a class gained a member"

# ------------------------------------------------------------ 3: too little text, and the count shown
# Fails if the floor is a presence test, or if the refusal drops the number the author needs.
R=$(newrepo)
stage_bug "$R" new-bug demo-a "$FIT8"
run "$R"
eq "3 a section of 8 letters or digits is refused" "$RC" "1"
has "3 the refusal reports what it found" "$OUT" "has 8 letters or digits"

# ------------------------------------------------------------ 4: the boundary, both sides
# Fails if `>=` becomes `>` (40 refused) or `>=` becomes `>= 39`-style off-by-one (39 accepted).
R=$(newrepo); stage_bug "$R" new-bug demo-a "$FIT40"; run "$R"
eq "4 exactly 40 is accepted" "$RC" "0"
R=$(newrepo); stage_bug "$R" new-bug demo-a "$FIT39"; run "$R"
eq "4 exactly 39 is refused" "$RC" "1"

# ------------------------------------------------------------ 5: a heading inside a fence is not a section
# Fails if the scan ignores code fences. A template pasted in a fence would satisfy the rule.
R=$(newrepo)
write_bug "$R/docs/issues/2026-10-16-new-bug.md" demo-a ""
printf '\n```\n## Class fit\n%s\n```\n' "$FIT_LONG" >> "$R/docs/issues/2026-10-16-new-bug.md"
git -C "$R" add docs/issues/2026-10-16-new-bug.md
run "$R"
eq "5 a Class fit heading inside a code fence does not count" "$RC" "1"
# The tilde fence is its own branch in the scan, and a mutation that drops it survives case 5 alone.
R=$(newrepo)
write_bug "$R/docs/issues/2026-10-16-new-bug.md" demo-a ""
printf '\n~~~\n## Class fit\n%s\n~~~\n' "$FIT_LONG" >> "$R/docs/issues/2026-10-16-new-bug.md"
git -C "$R" add docs/issues/2026-10-16-new-bug.md
run "$R"
eq "5 a Class fit heading inside a tilde fence does not count either" "$RC" "1"

# ------------------------------------------------------------ 6: the heading level is exact
# Fails if any heading depth is accepted. `###` and `#` are different documents.
R=$(newrepo)
write_bug "$R/docs/issues/2026-10-16-new-bug.md" demo-a ""
printf '\n### Class fit\n%s\n' "$FIT_LONG" >> "$R/docs/issues/2026-10-16-new-bug.md"
git -C "$R" add docs/issues/2026-10-16-new-bug.md
run "$R"
eq "6 a level-3 heading is not the section" "$RC" "1"
R=$(newrepo)
write_bug "$R/docs/issues/2026-10-16-new-bug.md" demo-a ""
printf '\n# Class fit\n%s\n' "$FIT_LONG" >> "$R/docs/issues/2026-10-16-new-bug.md"
git -C "$R" add docs/issues/2026-10-16-new-bug.md
run "$R"
eq "6 a level-1 heading is not the section" "$RC" "1"

# ------------------------------------------------------------ 7: text under the NEXT heading is not the section's
# Fails if the section runs to the end of the file instead of the next H2.
R=$(newrepo)
write_bug "$R/docs/issues/2026-10-16-new-bug.md" demo-a "$FIT8"
printf '\n## Root cause\n%s\n' "$FIT_LONG" >> "$R/docs/issues/2026-10-16-new-bug.md"
git -C "$R" add docs/issues/2026-10-16-new-bug.md
run "$R"
eq "7 text under a later heading does not count for Class fit" "$RC" "1"

# ------------------------------------------------------------ 8: the OLD route no longer satisfies
# Fails if a Members entry is still accepted in place of the section. This is the case that says
# the rule MOVED: a `+1:` entry with a long derivation, and still a refusal.
R=$(newrepo)
stage_bug "$R" new-bug demo-a ""
sed -i "s/^\*\*Members:\*\* none yet\.\$/**Members:** none yet. +1: \`new-bug\` — $FIT_LONG./" "$R/$CLASS_A"
git -C "$R" add "$CLASS_A"
run "$R"
eq "8 a long Members entry does not stand in for a Class fit" "$RC" "1"

# ------------------------------------------------------------ 9: both routes together are fine
# Fails if a Members edit is itself refused. Legacy lines are FROZEN, not forbidden, and a session
# that has not read the new rule yet will still write one.
R=$(newrepo)
stage_bug "$R" new-bug demo-a "$FIT_LONG"
sed -i "s/^\*\*Members:\*\* none yet\.\$/**Members:** none yet. +1: \`new-bug\` — $FIT_LONG./" "$R/$CLASS_A"
git -C "$R" add "$CLASS_A"
run "$R"
eq "9 a Class fit plus a Members edit is accepted" "$RC" "0"
[ "$RC" -eq 0 ] || printf '  output: %s\n' "$OUT"

# ------------------------------------------------------------ 10: a retag joins a class too
# Fails if only a NEW FILE is held to the rule. A retag never passes through `doc(create)`, so this
# hook is the only place it can be checked.
R=$(newrepo legacy)
sed -i 's#^- cluster/demo-a#- cluster/demo-b#' "$R/docs/issues/2026-09-01-old-bug.md"
git -C "$R" add docs/issues/2026-09-01-old-bug.md
run "$R"
eq "10 retagging an old bug into a class, with no Class fit, is refused" "$RC" "1"
has "10 the refusal names the class it joined" "$OUT" "cluster/demo-b"
printf '\n## Class fit\n%s\n' "$FIT_LONG" >> "$R/docs/issues/2026-09-01-old-bug.md"
git -C "$R" add docs/issues/2026-09-01-old-bug.md
run "$R"
eq "10 ...and accepted once the retagged file says why" "$RC" "0"

# ------------------------------------------------------------ 11: a move is not joining
# Fails if the check keys on the PATH. An archive move changes the path and not the stem.
R=$(newrepo legacy)
git -C "$R" mv docs/issues/2026-09-01-old-bug.md docs/issues/archive/2026-09-01-old-bug.md
run "$R"
eq "11 archiving an old bug with no Class fit is accepted" "$RC" "0"

# ------------------------------------------------------------ 12: an edit that keeps the tag is not joining
# Fails if the rule fires on any touch of a bug file instead of on joining a class.
R=$(newrepo legacy)
printf '\nA later note.\n' >> "$R/docs/issues/2026-09-01-old-bug.md"
git -C "$R" add docs/issues/2026-09-01-old-bug.md
run "$R"
eq "12 editing an old bug without changing its class is accepted" "$RC" "0"

# ------------------------------------------------------------ 13: an unstaged file is invisible
# Fails if the hook reads the working tree for the population. This is the legal escape the refusal
# names, so it has to be true.
R=$(newrepo)
write_bug "$R/docs/issues/2026-10-16-new-bug.md" demo-a ""
run "$R"
eq "13 an untracked bug file with no Class fit is not refused" "$RC" "0"

# ------------------------------------------------------------ 14: one bad file among good ones
# Fails if the loop stops at the first file, or names every file instead of the failing one.
R=$(newrepo)
stage_bug "$R" good-bug demo-a "$FIT_LONG"
stage_bug "$R" bad-bug demo-b ""
run "$R"
eq "14 one bug without a Class fit refuses the whole commit" "$RC" "1"
has "14 the refusal names the bad one" "$OUT" "2026-10-16-bad-bug.md"
hasnt "14 ...and does not name the good one" "$OUT" "2026-10-16-good-bug.md"

# ------------------------------------------------------------ 15, 16: the INDEX is what is read
# Fails if the hook reads the working tree. A pathspec or partial commit publishes the index.
R=$(newrepo)
stage_bug "$R" new-bug demo-a ""
printf '\n## Class fit\n%s\n' "$FIT_LONG" >> "$R/docs/issues/2026-10-16-new-bug.md"
run "$R"
eq "15 a Class fit written on disk but not staged does not satisfy the commit" "$RC" "1"
R=$(newrepo)
stage_bug "$R" new-bug demo-a "$FIT_LONG"
write_bug "$R/docs/issues/2026-10-16-new-bug.md" demo-a ""
run "$R"
eq "16 a staged Class fit is judged even when the disk copy lost it" "$RC" "0"

# ------------------------------------------------------------ 17: CRLF does not hide the section
# Fails if a trailing carriage return changes the heading match or the count. Only the BODY is
# CRLF here, on purpose: a whole-file CRLF bug is refused earlier by the one-tag rule, because
# `frontmatter` wants a literal `---\n` (measured 2026-10-16: "no cluster/ tag"). That is an older
# rule with its own behavior, and mixing it in would make this case test it instead of the scan.
# HONEST LIMIT: the mutant that stops stripping the `\r` SURVIVES this case, and it is equivalent,
# not missed: the heading is compared after `.strip()` and the count skips non-alphanumerics, so
# a retained `\r` changes nothing today. The case guards a future change that compares exactly.
R=$(newrepo)
write_bug "$R/docs/issues/2026-10-16-new-bug.md" demo-a "$FIT_LONG"
awk '/^---$/ && n < 2 { n++; print; next } n >= 2 { printf "%s\r\n", $0; next } { print }' \
    "$R/docs/issues/2026-10-16-new-bug.md" > "$R/crlf.tmp" && mv "$R/crlf.tmp" "$R/docs/issues/2026-10-16-new-bug.md"
git -C "$R" add docs/issues/2026-10-16-new-bug.md
# The premise: the INDEX blob really holds carriage returns on the heading. Without it this case
# proves nothing about CRLF.
git -C "$R" cat-file -p :docs/issues/2026-10-16-new-bug.md | grep -q $'^## Class fit\r$' \
    && ok || no "17 fixture: the staged blob keeps its carriage returns"
run "$R"
eq "17 a CRLF bug file with a Class fit is accepted" "$RC" "0"
[ "$RC" -eq 0 ] || printf '  output: %s\n' "$OUT"

# ------------------------------------------------------------ 18: the largest section counts
# Fails if the FIRST section with the title is judged, or if a later section's size is dropped when
# an earlier one closes. The order empty, real, other-heading matters: a mutant that keeps the first
# recorded size instead of the largest survived the two-section form of this case (measured
# 2026-10-16), because the final `max` rescued it. A real bug file always has a heading after Class
# fit, and the server judges the same way.
R=$(newrepo)
write_bug "$R/docs/issues/2026-10-16-new-bug.md" demo-a ""
printf '\n## Class fit\n\n## Class fit\n%s\n\n## Root cause\nx\n' "$FIT_LONG" >> "$R/docs/issues/2026-10-16-new-bug.md"
git -C "$R" add docs/issues/2026-10-16-new-bug.md
run "$R"
eq "18 an empty Class fit above a real one does not mask it" "$RC" "0"

# ------------------------------------------------------------ 19, 20: the REAL template
# Fails if an untouched copy of docs/issues/_TEMPLATE.md passes. The placeholder under Class fit
# must stay below the floor, or the template teaches the author that the section is decoration.
# Case 20 is the other direction: the same copy with the placeholder replaced is accepted, so the
# template's own structure (its HTML comment, its headings) is not what trips the scan.
TEMPLATE="$SELF_ROOT/docs/issues/_TEMPLATE.md"
R=$(newrepo)
sed 's#^- cluster/<slug>.*#- cluster/demo-a#' "$TEMPLATE" > "$R/docs/issues/2026-10-16-new-bug.md"
git -C "$R" add docs/issues/2026-10-16-new-bug.md
run "$R"
eq "19 an untouched copy of the template is refused" "$RC" "1"
has "19 ...for the Class fit, not for the tag it was given" "$OUT" "letters or digits"
R=$(newrepo)
sed -e 's#^- cluster/<slug>.*#- cluster/demo-a#' -e "s#^\*Which class trait does this bug show?\*\$#$FIT_LONG#" \
    "$TEMPLATE" > "$R/docs/issues/2026-10-16-new-bug.md"
git -C "$R" add docs/issues/2026-10-16-new-bug.md
run "$R"
eq "20 the template with only the placeholder replaced is accepted" "$RC" "0"
[ "$RC" -eq 0 ] || printf '  output: %s\n' "$OUT"

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" -eq 0 ]
