# tests/lib/ledger-corpus.sh — the synthetic issue-cluster corpus that the hook suites build.
#
# SOURCED, never run. Used by `tests/pre-commit-ledger-divergence.sh` and `tests/commit-mine.sh`.
# It holds ONE copy of the corpus writer and ONE copy of the three traps below, which each suite
# used to carry and re-explain. A third suite that needs the corpus sources this file; it does not
# paste the writers again (`docs/conventions/changing-an-enforcement-gate.md`).
#
# The corpus is SYNTHETIC and minimal, never a copy of the live ledger: a copy would red whenever
# the real ledger changed, which is a test that reports the corpus instead of the code. Fixtures are
# throwaway repos under `mktemp -d` — never this checkout, which several sessions share.
#
# THE THREE TRAPS. Each one produces a refusal that looks like the rule under test.
#
# 1. ELEVEN classes, and the number is load-bearing rather than arbitrary. The hook carries its own
#    vacuity guard — `len(index_rows) <= 10` refuses, on the grounds that CHECK 1 passing over a
#    table nobody could parse is exactly how a format change goes unnoticed. A one-class fixture
#    therefore cannot reach ANY of the rules under test: it reds on the row-count guard instead,
#    which is a different refusal wearing the same exit code. Drop this to ten and every case starts
#    testing that guard by accident.
#
# 2. Slugs are LETTERED (`demo-a` … `demo-k`), never numbered. `valid_slugs` admits a slug only when
#    every character is lowercase ASCII or `-`, so `demo-1` is silently not a slug: `valid` comes
#    back empty, no Index row resolves, and the hook reds on its own row-count guard — measured
#    2026-09-15, "only 0 Index row(s) parsed", from a fixture that looked entirely reasonable. The
#    live corpus has no digits in a slug, so nothing else showed it.
#
# 3. The corpus is TWO surfaces, and collapsing them into one is the mistake this comment exists to
#    stop. `docs/trackers/issue-clusters.md` is the ROSTER: a header and the Index table, nothing
#    else — a `## IC-N` section left in it is its own refusal ("this commit leaves class section(s)
#    in it"). The `**Slug:**` and `**Members:**` declarations live one per file under
#    `docs/trackers/issue-clusters/`, and `read_ledger` concatenates the two before parsing. A
#    fixture that puts the declarations in the roster reds on the section rule instead of whatever
#    it meant to test.

LEDGER_CLASSES=11
LEDGER_SLUGS=(demo-a demo-b demo-c demo-d demo-e demo-f demo-g demo-h demo-i demo-j demo-k)

# ledger_write_roster <repo> [clean|counted]
#
# FOUR cells per row, never five. A fifth trips `index_rows_with_extra_cells` (the mechanism-column
# rule) instead, and because CHECK 1 runs first the suite would still look green while asserting
# against a refusal it never triggered.
#
# The slug cell carries the BARE slug (`demo-a`), never `cluster/demo-a`. Measured against the
# script 2026-09-15: with the prefixed form `parse_index_counts` matches nothing, the declared map
# comes back empty, and a different check fires. The live ledger is bare.
#
# `counted` puts an integer in IC-1's post-slug cell. That is CHECK 1 (`no_index_row_stores_a_count`),
# the cheapest deterministic refusal available and the one chosen because it needs no second file,
# so a case meant to vary only one thing varies only that. Prose there is no stored count and the
# hook is silent.
ledger_write_roster() {
    local kind="${2:-clean}" f="$1/docs/trackers/issue-clusters.md" i
    {
        echo "# Issue clusters"
        echo
        echo "| id | class | slug | promotes to |"
        echo "|---|---|---|---|"
        if [ "$kind" = counted ]; then
            printf '| IC-1 | a demo class | `%s` | 3 |\n' "${LEDGER_SLUGS[0]}"
        else
            printf '| IC-1 | a demo class | `%s` | not yet |\n' "${LEDGER_SLUGS[0]}"
        fi
        for i in $(seq 2 "$LEDGER_CLASSES"); do
            printf '| IC-%s | a demo class | `%s` | not yet |\n' "$i" "${LEDGER_SLUGS[$((i - 1))]}"
        done
    } > "$f"
}

# ledger_write_classes <repo>: one file per class, each with its `**Slug:**` and a bare Members line.
ledger_write_classes() {
    local p="$1" i s
    for i in $(seq 1 "$LEDGER_CLASSES"); do
        s="${LEDGER_SLUGS[$((i - 1))]}"
        printf '# IC-%s — demo class %s\n\n**Slug:** `cluster/%s`\n\n**Members:** none yet.\n' \
            "$i" "$i" "$s" > "$p/docs/trackers/issue-clusters/IC-$i-$s.md"
    done
}
