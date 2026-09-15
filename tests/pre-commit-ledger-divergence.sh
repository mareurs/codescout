#!/usr/bin/env bash
# tests/pre-commit-ledger-divergence.sh — the worktree/index divergence note.
#
# WHAT THIS GUARDS
#
# `scripts/pre-commit-ledger-counts.py` judges the INDEX, while the hook that runs it
# executes the copy in the WORKING TREE. So a corpus file edited on disk and not staged
# puts the reader in a state where the refusal cites a file, and opening that file shows
# the OPPOSITE of what the refusal says
# (`docs/issues/2026-09-13-a-gate-script-edited-in-the-worktree-has-already-shipped.md`).
#
# Measured 2026-09-15 in an isolated repo, the same grep against both copies of the
# ledger: worktree `1`, index `0`. The reader's rational conclusion from that pair is
# "the gate is broken", and the action it licenses is `--no-verify`. The note exists to
# make the contradiction legible WITHOUT softening the verdict, which is correct.
#
# THE DISCRIMINATION SET IS CASES 2-4, and they are the point. A note printed on every
# refusal would satisfy case 1 alone and be strictly worse than none: it would claim a
# divergence that is not there, on the majority of refusals, and get tuned out. Each of
# 2-4 removes exactly ONE of the three conditions the note requires.
#
# Fixtures are throwaway git repos under `mktemp -d` — never this checkout, which several
# sessions share. The corpus is SYNTHETIC and minimal rather than copied from the live
# tree: a copy would red whenever the real ledger changed, which is a test that reports
# the corpus instead of the code.
set -uo pipefail

SELF_ROOT=$(cd "$(dirname "$0")/.." && pwd)
SCRIPT="$SELF_ROOT/scripts/pre-commit-ledger-counts.py"
PASS=0
FAIL=0

has() { # has <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        PASS=$((PASS + 1))
    else
        FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "  expected to find: $3"
        printf '  in: %s\n' "$2" | head -12
    fi
}
hasnt() { # hasnt <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "  expected NOT to find: $3"
        printf '  in: %s\n' "$2" | head -12
    else
        PASS=$((PASS + 1))
    fi
}
eq() { # eq <label> <actual> <expected>
    if [ "$2" = "$3" ]; then
        PASS=$((PASS + 1))
    else
        FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "  actual:   $2"; echo "  expected: $3"
    fi
}

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

MARKER="the worktree and the index DISAGREE"

# ELEVEN classes, and the number is load-bearing rather than arbitrary. The hook carries
# its own vacuity guard — `len(index_rows) <= 10` refuses, on the grounds that CHECK 1
# passing over a table nobody could parse is exactly how a format change goes unnoticed.
# A one-class fixture therefore cannot reach ANY of the rules under test here: it reds on
# the row-count guard instead, which is a different refusal wearing the same exit code.
# Drop this to ten and every case below starts testing that guard by accident.
CLASSES=11

# Slugs are LETTERED (`demo-a` … `demo-k`), never numbered. `valid_slugs` admits a slug
# only when every character is lowercase ASCII or `-`, so `demo-1` is silently not a slug:
# `valid` comes back empty, no Index row resolves, and the hook reds on its own row-count
# guard — measured 2026-09-15, "only 0 Index row(s) parsed", from a fixture that looked
# entirely reasonable. The live corpus has no digits in a slug, so nothing else showed it.
SLUGS=(demo-a demo-b demo-c demo-d demo-e demo-f demo-g demo-h demo-i demo-j demo-k)

# The corpus is TWO surfaces, and collapsing them into one is the mistake this comment
# exists to stop the next reader repeating. `docs/trackers/issue-clusters.md` is the
# ROSTER: a header and the Index table, nothing else — a `## IC-N` section left in it is
# its own refusal ("this commit leaves class section(s) in it"). The `**Slug:**` and
# `**Members:**` declarations live one per file under `docs/trackers/issue-clusters/`, and
# `read_ledger` concatenates the two before parsing. A fixture that puts the declarations
# in the roster reds on the section rule instead of whatever it meant to test.
_write_roster() { # _write_roster <repo> <clean|counted>
    local p="$1" kind="$2" f="$1/docs/trackers/issue-clusters.md"
    {
        echo "# Issue clusters"
        echo
        echo "| id | class | slug | promotes to |"
        echo "|---|---|---|---|"
        # The post-slug cell is what `parse_index_counts` reads. Prose there is no stored
        # count and the hook is silent; an integer is CHECK 1 (`no_index_row_stores_a_count`),
        # the cheapest deterministic refusal here and the one chosen because it needs no
        # second file — so a case meant to vary only the divergence varies only that.
        if [ "$kind" = counted ]; then
            printf '| IC-1 | a demo class | `%s` | 3 |\n' "${SLUGS[0]}"
        else
            printf '| IC-1 | a demo class | `%s` | not yet |\n' "${SLUGS[0]}"
        fi
        for i in $(seq 2 "$CLASSES"); do
            printf '| IC-%s | a demo class | `%s` | not yet |\n' "$i" "${SLUGS[$((i - 1))]}"
        done
    } > "$f"
}

# FOUR cells per row, never five. A fifth trips `index_rows_with_extra_cells` (the
# mechanism-column rule) instead, and because CHECK 1 runs first the suite would still
# look green while asserting against a refusal it never triggered.
#
# The slug cell carries the BARE slug (`demo-1`), never `cluster/demo-1`. Measured against
# the script 2026-09-15: with the prefixed form `parse_index_counts` matches nothing, the
# declared map comes back empty, and a different check fires. The live ledger is bare.
_write_classes() { # _write_classes <repo>
    local p="$1" i s
    for i in $(seq 1 "$CLASSES"); do
        s="${SLUGS[$((i - 1))]}"
        printf '# IC-%s — demo class %s\n\n**Slug:** `cluster/%s`\n\n**Members:** none yet.\n' \
            "$i" "$i" "$s" > "$p/docs/trackers/issue-clusters/IC-$i-$s.md"
    done
}

newrepo() { # newrepo <clean|counted> -> echoes the repo path, corpus COMMITTED
    local p="$WORK/repo$RANDOM$RANDOM"
    mkdir -p "$p/docs/trackers/issue-clusters" "$p/docs/issues" "$p/scripts"
    git -C "$p" init -q
    git -C "$p" config user.email t@t; git -C "$p" config user.name t
    _write_roster "$p" "$1"
    _write_classes "$p"
    cp "$SCRIPT" "$p/scripts/"
    [ -f "$SELF_ROOT/scripts/commit-sequence-tail.txt" ] \
        && cp "$SELF_ROOT/scripts/commit-sequence-tail.txt" "$p/scripts/"
    git -C "$p" add -A >/dev/null
    git -C "$p" commit -qm corpus
    echo "$p"
}

# Dirty a corpus file ON DISK ONLY. Appending a comment line keeps the parse valid, so
# the divergence is the only variable — a broken-on-disk file would change the verdict
# under `--source=worktree` and confound case 4.
dirty_on_disk() { printf '\n<!-- edited on disk, not staged -->\n' >> "$1/docs/trackers/issue-clusters.md"; }

run() { # run <repo> <args...> -> sets OUT and RC
    local repo="$1"; shift
    OUT=$(cd "$repo" && python3 scripts/pre-commit-ledger-counts.py "$@" 2>&1)
    RC=$?
}

# --- 1. Divergence + refusal: the note fires, names the file, verdict UNCHANGED --------
# The hook passes NO arguments, so this invocation is the one that runs on a real commit.
# Asserting the bare form is deliberate: `_SOURCE` was briefly assigned inside the
# `--source=` branch of the arg loop, which is green under every explicit-flag test and
# dead on exactly this path.
R=$(newrepo counted)
dirty_on_disk "$R"
run "$R"
eq   "1 still refuses — the verdict is right about the index" "$RC" "1"
has  "1 the note fires" "$OUT" "$MARKER"
has  "1 it names the diverged path" "$OUT" "docs/trackers/issue-clusters.md"
has  "1 it says which side the verdict came from" "$OUT" "computed from the INDEX"
has  "1 and warns the file will read otherwise" "$OUT" "the OPPOSITE of what it says"
has  "1 the rule's own refusal survives alongside it" "$OUT" "stores a DERIVED count"

# --- 2. DISCRIMINATION: a refusal with NO divergence prints no note -------------------
# Same refusal as case 1, clean tree. Without this, a note printed unconditionally on
# every refusal passes case 1 while asserting a divergence that is not there.
R=$(newrepo counted)
run "$R"
eq   "2 refuses" "$RC" "1"
has  "2 the rule still refuses" "$OUT" "stores a DERIVED count"
hasnt "2 and the note does NOT fire on a clean tree" "$OUT" "$MARKER"

# --- 3. DISCRIMINATION: divergence with NO refusal stays silent -----------------------
# A hook that fires on every commit to say nothing is how `--no-verify` gets learned, so
# the note must be tied to the refusal and not to the dirty state.
R=$(newrepo clean)
dirty_on_disk "$R"
run "$R"
eq   "3 passes" "$RC" "0"
hasnt "3 and says nothing at all" "$OUT" "$MARKER"
eq   "3 output is empty" "$(printf '%s' "$OUT" | wc -c)" "0"

# --- 4. DISCRIMINATION: under --source=worktree the note would be FALSE ---------------
# There the verdict IS computed from the bytes on disk, so there is no second copy to
# disagree with. Same dirty tree as case 1; only the source differs.
R=$(newrepo counted)
dirty_on_disk "$R"
run "$R" --source=worktree
eq   "4 still refuses from the worktree" "$RC" "1"
hasnt "4 and claims no divergence" "$OUT" "$MARKER"

# --- 5. The REMEDY TEXT, by shape rather than by wording ------------------------------
# A suite that tests only a guard's predicate leaves its remedy untested by construction.
# Pinning sentences would red on every rewording; these two assertions red on exactly the
# regressions that matter — losing the second addressee, or losing the answerable form of
# the question put to them (`observer-blindness:OB-20`: naming who to ask buys ARRIVAL,
# never ANSWERABILITY).
R=$(newrepo counted)
dirty_on_disk "$R"
run "$R"
has  "5 names an instrument for finding the other party" "$OUT" "file-provenance.py"
has  "5 and asks them something they can answer" "$OUT" "ready to STAGE"
has  "5 enumerating the answers it can take" "$OUT" "yes, not yet, or I will revert it"
has  "5 and says waiting is not one of them" "$OUT" "index moves only when somebody stages"

# --- 6. MUST-SURVIVE: outside a git repo the hook stays silent and does not crash ------
# `_corpus_paths_diverged` shells out to `git diff` on every refusal, so the question is
# whether a git failure can take down a hook whose verdict is already on stderr. It cannot
# be asked the way this case first asked it: OUTSIDE A REPO THE HOOK NEVER REACHES A
# REFUSAL AT ALL. `read_ledger("index")` returns None with no index to read, and `main`
# returns 0 on that path by design — "a commit that touches neither the ledger nor a bug
# file must not be blocked". The first draft asserted exit 1 here and failed against
# correct code, which is the whole reason this comment names the mechanism.
#
# So what is pinned is the reachable claim: silent, zero, and NO TRACEBACK. The traceback
# assertion is the one carrying weight — `_corpus_paths_diverged` catches `OSError` and a
# non-zero status by hand precisely so `git` being absent cannot become a crash, and a
# bare `_git` call there would red this line.
P="$WORK/nogit$RANDOM"
mkdir -p "$P/docs/trackers/issue-clusters" "$P/scripts"
_write_roster "$P" counted
_write_classes "$P"
cp "$SCRIPT" "$P/scripts/"
OUT=$(cd "$P" && python3 scripts/pre-commit-ledger-counts.py 2>&1); RC=$?
eq   "6 outside a repo the hook exits clean" "$RC" "0"
hasnt "6 with no traceback" "$OUT" "Traceback"
hasnt "6 and claims no divergence" "$OUT" "$MARKER"

# --- 7. The note survives a SECOND diverged file, and counts what it names -------------
# The count and the list are derived separately in the message, which is the shape
# `CLAUDE.md` warns about — a headline from one derivation beside an enumeration from
# another reconciles nowhere. Asserted together so they cannot drift.
R=$(newrepo counted)
dirty_on_disk "$R"
printf -- '---\nstatus: open\nkind: bug\ntags: []\n---\n\n# staged\n' > "$R/docs/issues/b.md"
git -C "$R" add docs/issues/b.md >/dev/null
printf 'edited after staging\n' >> "$R/docs/issues/b.md"
run "$R"
has  "7 names both diverged paths" "$OUT" "docs/issues/b.md"
has  "7 and the ledger too" "$OUT" "docs/trackers/issue-clusters.md"
has  "7 with a count that matches the list" "$OUT" "DISAGREE about 2 of this hook"

echo
echo "ledger-divergence: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
