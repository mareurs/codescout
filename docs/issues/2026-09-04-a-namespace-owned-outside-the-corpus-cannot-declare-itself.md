---
kind: bug
status: open
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
opened: 2026-09-04
owner: marius
related: []
severity: medium
---

# BUG: a namespace owned outside the corpus cannot say so, and both prescribed remedies make it worse

## Summary

`cited_prefix_with_no_definer` reports a prefix cited many times with no `## <ID> — <title>`
heading and no `entry_prefix` declaration. It names two remedies — define a heading per entry, or
declare the namespace — and there is no third. But a prefix can be in a **third state** the check
cannot distinguish: **alive and authoritative somewhere the resolver cannot read.** `TC-N` is owned
by `scripts/tc-suites/legacy-natural.json`, which defines `TC-01`…`TC-20` as executable benchmark
test cases. It is not abandoned and it was not never-created. Measured here, the check's own
preferred remedy converts **1 silent finding into 6 dangling citations** — a strictly worse,
strictly *reported* state — while looking like progress.

## Symptom (Effect)

```
"check": "cited_prefix_with_no_definer",
"detail": "`TC-N` is cited 107 times across 13 files, but no `## TC-N — <title>` heading exists
anywhere in the corpus and no artifact declares `entry_prefix: TC`. These citations are neither
resolved nor reported dangling — link_scan's resolver treats a wholly-unknown prefix as prose
noise (the same gate that keeps `UTF-8`/`SHA-256` silent), so this state is reported nowhere
else. […] Either define the namespace (a heading per entry) or declare it empty via
`entry_prefix` […]"
```

Every clause is true. The diagnosis it implies — *abandoned or never created* — is false.

## Reproduction

1. `git rev-parse HEAD` → `c9e9b77a` (branch `experiments`).
2. `librarian(action="doctor")` → `cited_prefix_with_no_definer` names `TC-N`.
3. Confirm the namespace is alive:
   `grep -oh '"TC-[0-9]*"' scripts/tc-suites/*.json | sort -u -V` → `TC-01` … `TC-20`.
4. Confirm it is documented, in a shape the resolver rejects:
   `grep -c "^#\+ TC-[0-9]" docs/research/2026-04-03-embedding-model-benchmark.md` → **20**, every
   one written `#### TC-01: Exact type name` — a **colon**, where the definition rule requires
   ` — `. Per `get_guide("tracker-conventions")` § *Entry headings*, a heading without the dash
   defines nothing.
5. Now cost the prescribed remedy before applying it (this is the step that inverts it):
   ```
   grep -oE "^#+ TC-[0-9]+" docs/research/2026-04-03-embedding-model-benchmark.md \
     | grep -oE "TC-[0-9]+" | sort -u > defined.txt      # 20
   grep -rhoE '\bTC-[0-9]+\b' . --include='*.md' | sort -u > cited.txt   # 26
   comm -13 defined.txt cited.txt
   ```
   → `TC-1 TC-21 TC-22 TC-23 TC-24 TC-25`

## Environment

codescout `experiments` @ `c9e9b77a`, main checkout, Linux, MCP over the release binary. Observed
2026-09-04 02:0x EEST.

## Root cause

**The check's silence conditions are its remedy set, and both are properties of the markdown
corpus.** Verified by reading every silencer rather than inferring: `scan_cited_prefix_with_no_definer`
(`src/librarian/tools/doctor.rs:3811`) is quiet in exactly two cases, each pinned by its own test —
`cited_prefix_with_no_definer_is_silent_when_prefix_is_defined_elsewhere` (a heading definer exists
somewhere in the corpus) and `cited_prefix_with_no_definer_is_silent_when_prefix_is_declared` (an
artifact declares `entry_prefix`). A grep across `doctor.rs` for an allowlist, an exemption or an
external-prefix concept returns nothing. **There is no way to say "this namespace's authority is a
JSON file / a test suite / a code constant."**

So three states collapse into one finding:

| state | truth | what the remedies do |
|---|---|---|
| abandoned | definers deleted | correct: restore or retire |
| never created | typo'd or aspirational prefix | correct: create or drop |
| **externally owned** | **authority is outside markdown** | **both remedies damage it** |

The check is not wrong about what it measured; its doc comment is careful, saying it fires when
citation *volume* "looks like a real, abandoned-or-never-created namespace rather than an incidental
mention." The volume test is doing its job — `TC-N` **is** a real namespace. The unrepresentable
case is the one where "real" and "has a markdown definer" come apart.

## Evidence

### The remedy is measurably harmful, and the measurement is one command

Adding ` — ` to the 20 headings makes `TC` a *known* prefix. The moment it is known, the resolver
stops treating unmatched `TC-NN` tokens as prose noise and starts reporting them **dangling**. Six
cited ids have no heading:

- **`TC-21`–`TC-25`** — cited in `docs/research/2026-05-06-retrieval-stack-benchmark.md`,
  `docs/trackers/retrieval-benchmark.md`, `docs/issues/archive/2026-07-28-reranker-costs-42x-latency-and-lowers-score.md`
  and two `tests/librarian/goal_eval/fixtures/goal_02_retrieval_p5/*.json` fixtures. They are in
  **neither** the suite (`legacy-natural.json` stops at `TC-20`) nor the research doc. **This is a
  separate, real gap** — later benchmarks reference five test cases that exist nowhere — and it is
  surfaced only as a side effect of costing this remedy.
- **`TC-1`** — not a citation at all. It is the literal text of a shell command inside
  `docs/superpowers/plans/2026-05-06-retrieval-stack-plan.md:1993`:
  `grep -rn "TC-1\|test_case_1" docs/research/ | head`. A *documentation example of a search
  pattern*, extracted as an entry token. It also cannot ever resolve to `TC-01`: the grammar is
  `\b[A-Z]{1,3}-\d+\b`, so `TC-1` and `TC-01` are different tokens, kept apart by zero-padding alone
  — the hazard already open as
  `docs/issues/2026-09-02-prefix-t-collides-again-with-the-zero-padding-protection-gone.md`.

Net effect of the prescribed fix: **−1 finding, +6 findings**, and the six are in a category that
*is* reported while the one was not. A reader who applied it would see the `cited_prefix` line
disappear and reasonably record the issue as closed.

### The other remedy hands allocation to the wrong party

Declaring `entry_prefix: TC` makes the librarian the allocator: the next `append_entry` issues
`TC-21` from the committed high-water mark while a suite author independently writes `TC-21` into
`legacy-natural.json`. That is the cross-writer collision the high-water mechanism exists to
prevent, manufactured by following the hint. It would also put a **dated 2026-04-03 research
snapshot** under the librarian write guard, off-limits to direct `edit_file`.

### Caveat on the citation counts

My `grep -rhoE '\bTC-[0-9]+\b'` is not fence-aware, so the 26 cited ids may overcount by the `TC-1`
case if the resolver skips fenced blocks. The direction of the finding is unaffected: `TC-21`–`TC-25`
are outside any code fence, so **at least 5** would dangle.

## Hypotheses tried

1. **Hypothesis:** `TC-N` is an abandoned namespace whose definers were deleted.
   **Test:** `git log -S 'TC-01' -- docs/` and read `scripts/tc-suites/`.
   **Verdict:** rejected — the suite defines `TC-01`…`TC-20` today and the research doc documents
   all 20. Nothing was deleted; nothing was ever in the required shape.

2. **Hypothesis:** an allowlist or external-prefix escape already exists and was simply not used.
   **Test:** grep `doctor.rs` for `KNOWN_NON_LEDGER|prefix_allowlist|external_prefix|IGNORED_PREFIX`,
   then read every test asserting silence.
   **Verdict:** rejected — exactly two silencers, both corpus-internal.

3. **Hypothesis:** adding ` — ` to the 20 headings is a safe, local fix.
   **Test:** the `comm -13` in *Reproduction* step 5, run **before** editing.
   **Verdict:** rejected — it creates six dangling citations. This is the whole finding.

## Fix

Not applied, and **the naive fix must not be applied** — see *Evidence*. Three candidate directions,
in increasing order of cost:

1. **An external-namespace declaration.** Let an artifact declare a prefix as owned elsewhere —
   e.g. frontmatter `external_prefix: TC` naming the authority (`scripts/tc-suites/*.json`). The
   check reads it as a third silence condition, `link_scan` keeps treating the tokens as prose, and
   nobody is invited to hand allocation to the librarian. Smallest change that makes the true state
   *representable*, which is the defect.
2. **Split the finding by evidence.** The check already knows the citation volume; it could also
   ask whether the prefix appears in non-markdown files under the repo and say so, turning one
   verdict into two distinguishable ones.
3. **Do nothing about `TC` specifically** and accept the finding as a permanent known-noise row.
   Cheapest, and it leaves the next reader to re-derive everything above — which is why this file
   exists.

Whichever lands, **fix the `TC-21`–`TC-25` gap separately**: it is a genuine content defect and is
independent of this mechanism.

## Tests added

None yet. A regression test for direction 1 is a fixture with a prefix cited above threshold, an
`external_prefix` declaration, and an assertion of silence — plus its negative control, the same
fixture without the declaration, asserting the finding still fires. Both are needed: a silence
assertion alone is monotone under the check being disabled entirely.

## Workarounds

Read the finding as *"this prefix has no markdown definer"*, which is what it measures, rather than
*"this namespace is abandoned"*, which is what it reads as. Before acting on it, run the
`comm -13` from *Reproduction* step 5 — if the cited set exceeds the definable set, the prescribed
remedy is net-negative.

## Resume

Decide between directions 1–3 in `## Fix`. If direction 1: the silence conditions are
`src/librarian/tools/doctor.rs:3811` (`scan_cited_prefix_with_no_definer`), and the two existing
silence tests at `:8933` and `:8963` are the pattern to copy for a third. Independently, hand the
`TC-21`–`TC-25` gap to whoever owns `docs/trackers/retrieval-benchmark.md` — five cited test cases
exist in neither `scripts/tc-suites/legacy-natural.json` nor
`docs/research/2026-04-03-embedding-model-benchmark.md`.

## References

- `src/librarian/tools/doctor.rs:3766-3811` — the check's doc comment and its two silence conditions.
- `scripts/tc-suites/legacy-natural.json` — the actual `TC-01`…`TC-20` authority.
- `docs/research/2026-04-03-embedding-model-benchmark.md` — 20 `#### TC-NN:` headings, colon-separated.
- `docs/issues/2026-09-02-prefix-t-collides-again-with-the-zero-padding-protection-gone.md` — the `TC-1` / `TC-01` half.
- `docs/issues/archive/2026-08-26-cited-prefix-with-no-definer-is-invisible.md` — why the check exists.
