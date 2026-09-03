---
id: af3a7ffe8626562c
kind: bug
status: open
title: 'BUG: a heading query without its `#` markers cannot reach the exact tiers, so an earlier heading merely containing the word wins'
owners:
- marius
tags:
- cluster/truncated-window-ordered-by-the-wrong-key
opened: 2026-09-03
severity: medium
---

# BUG: a heading query without its `#` markers cannot reach the exact tiers, so an earlier heading merely containing the word wins

## Summary

`resolve_section_range` compares the query against `HeadingInfo.text`, which is the **raw
heading line including its `##` markers**. So a bare query like `Index` can never satisfy
tier 1 or tier 2 (both exact), falls through to tier 4 (substring, case-insensitive), and
tier 4 returns `substring_matches[0]` — **document order**. Any earlier heading that merely
*contains* the word outranks the heading whose text *is* the word. On `doc(action="get")`
the substitution is invisible: `body_meta.heading` echoes the requested string.

## Symptom (Effect)

On `docs/trackers/issue-clusters.md` (`## Index` at line 244; `### One slug, two spellings
— a `cluster/`-prefixed pattern cannot see the Index` at line 185):

```
doc(action="get", id="1b5a080fe2efcb6b", heading="Index")
→ body starts "### One slug, two spellings — …"
→ body_meta: { "line_count": 58, "bytes": 3685, "heading": "Index" }
```

58 lines of the wrong section, and `body_meta.heading` reports `"Index"` — the string that
was *asked for*, not the one that was *resolved*. There is no resolved-heading field in the
response. `occurrence=1` does not help: it selects the first of the substring matches, which
is the same wrong section.

`read_file` over the same resolver **does** disclose it (`§ ### One slug, two spellings …
L7-L10`), so the two surfaces over one resolver differ in whether the caller can tell.

## Reproduction

`git rev-parse HEAD` → `d04f1e8a`, branch `experiments`. No build needed — live MCP.

```
cat > /tmp/headtest.md <<'EOF'
# Title

## Alpha

alpha body

### One slug, two spellings — a pattern cannot see the Index

subsection body here

## Index

INDEX-SECTION-BODY-MARKER

| a | b |
EOF
```

```
read_file(path="/tmp/headtest.md", heading="Index")
→ § ### One slug, two spellings — a pattern cannot see the Index  L7-L10   ← WRONG

read_file(path="/tmp/headtest.md", heading="## Index")
→ § ## Index  L11-L15                                                      ← right
```

## Environment

Linux, `experiments` @ `d04f1e8a`, codescout MCP over stdio, project `codescout`.
Reproduces on both `read_file` and `doc(action="get")` — one shared resolver.

## Root cause

`parse_all_headings` stores the **whole line**: `raw.push((line.to_string(), level, idx + 1))`
— `src/tools/file_summary/file_summary.rs:213`. So `HeadingInfo.text` for `## Index` is
`"## Index"`, not `"Index"`.

`resolve_section_range` (`src/tools/file_summary/file_summary.rs:282-451`) then runs its
cascade against that text, and **neither exact tier normalises the `#` prefix**:

- Tier 1 (`:366`) — `h.text == heading_query` → `"## Index" != "Index"` → miss.
- Tier 2 — `strip_inline_formatting(&h.text) == query_stripped`; `strip_inline_formatting`
  removes emphasis, not heading markers → still `"## Index" != "Index"` → miss.
- Tier 3 — `"## index".starts_with("index")` → false → miss.
- Tier 4 (`:406`) — `"### one slug … cannot see the index".contains("index")` → **true**, as
  does `"## index"`. Both match; `:419` returns `substring_matches[0]`, i.e. the
  document-order-first hit.

So the exact tiers are **structurally unreachable for the unprefixed query form**, and the
selection among the fuzzy hits is by document position — a key unrelated to why the call was
made. The doc comment at `:282` states *"the fuzzier tiers below never run once an exact tier
has matched"*, which is true of the code and misleading in effect: for a bare query no exact
tier can ever match, so the fuzzy tiers always run.

measured 2026-09-03: the two `read_file` calls above, on the fixture, against `d04f1e8a`.

## Evidence

### The resolver keeps the markers

`src/tools/file_summary/file_summary.rs:213`, inside `parse_all_headings`:

```rust
raw.push((line.to_string(), level, idx + 1));
```

### Tier 4 selects by document order

`src/tools/file_summary/file_summary.rs:419`:

```rust
None => Ok(make_range(&headings[substring_matches[0]])),
```

### The `##`-prefixed form is the one the tests use

`src/tools/file_summary/tests.rs:866` (`occurrence_selects_among_identical_headings`) queries
`HeadingQuery::new("## Fix", Some(1))` — prefixed. Every exact-tier test therefore exercises
the form that reaches tier 1, and none exercises the bare form that cannot.

### Second instance, on the WRITE path — and its diagnostic names the wrong cause

Reported 2026-09-03 by sessionId `63083c9e-cc56-4dbd-9852-820f34261eeb`, who hit it **hours
before this file existed** and recorded it in commit `4d4d804f`'s message, so it is
independently dated rather than reconstructed. Same file, same mis-binding, different surface:

```
doc(action="update", patch={body_edits:[{heading: "Index", ...}]})
  → body_edits[0]: old_string not found in section 'Index'. The text must match
    exactly (whitespace-sensitive). I looked, and nothing scored above 0.5 similarity.
    scoped_miss_tier: "no_similar_match"
```

Passing `## Index` resolved it immediately. **The read path and the write path share the
resolver**, so this is not a `body_edits` quirk — `doc(action="get", heading="Index")`
mis-binds identically.

**The part that makes this worse than a wrong section, and worth its own remedy: the error
reports the wrong cause, confidently, with a number attached.** It names an `old_string`
problem and volunteers a similarity score, when the `old_string` was present and correct in the
section the caller meant — what was wrong was the *section selection*. `scoped_miss_tier:
"no_similar_match"` is a true statement about the section the resolver **chose** and a false one
about the section the caller **named**, so the diagnostic points away from the defect. The
reporter re-read their `old_string` twice before questioning the heading. So the fix owes a
third thing beyond the two in `## Fix`: when a heading resolves through tier 3 or 4, say which
heading was bound, in the error as well as in the success path.

**And the section it wrongly bound is the note warning that a `cluster/`-prefixed pattern
cannot see the Index table** — an addressing-failure warning, mis-addressed. That is `IC-6`
firing inside the ledger that defines `IC-6`, twice in one day, on two different surfaces.

## Hypotheses tried

1. **Hypothesis** — a peer had moved the sections mid-session, so the "wrong" body was
   actually correct. **Test** — `grep -n '^#\{1,3\} '` on the live file after the observation.
   **Verdict** — rejected; `## Index` was at 244 and the returned 58-line body matched
   `### One slug` at 185-243.
2. **Hypothesis** — the librarian has its own resolver and `resolve_section_range` is
   innocent. **Test** — reproduced on a plain scratchpad file via `read_file`, which is
   outside the librarian path. **Verdict** — rejected; one shared resolver.
3. **Hypothesis** — `occurrence=` is the escape hatch. **Test** — `doc(get, heading="Index",
   occurrence=1)`. **Verdict** — rejected; it indexes into the substring-match list, so it
   selects the same wrong section.

## Fix

**Primary defect FIXED on `experiments`. The disclosure half is NOT, so this file stays open** —
see *Still owed* below. Archiving requires both.

- **SHA** `205a04d4818c4f1ab3a1979fa9783f920c1a05b8` (on `experiments`)
- **patch-id** `37615c490a973b822fa41eab49d56492904b4d04`

**What shipped — direction 1 of the two below, and it cost nothing.** The trade-off this section
originally described turned out not to exist. Normalising is confined to **tier 2**: a new
`strip_heading_marker` (`src/tools/file_summary/file_summary.rs`, beside `heading_level`) strips
the `#` prefix from both the stored heading text and the query before the tier-2 comparison, so
a bare `Index` reaches an *exact* tier. **Tier 1 is untouched**, and because it runs first, a
caller who writes `## Foo` still selects by level and `### Foo` cannot answer it — which is the
level-sensitivity this file worried about losing. Where both `## Foo` and `### Foo` exist, a
bare `Foo` now matches both at tier 2 and raises the **existing** duplicate error naming each
line, the same contract two byte-identical headings already get. That is strictly better than
the silent document-order pick it replaces.

Direction 2 (rank tiers 3/4 by match quality) was **not** taken and is not needed for this
defect. Left recorded because it remains the answer if level-sensitivity ever has to move.

### Still owed — the third remedy, and it survives this fix

When resolution goes through tier 3 or 4, **name the heading that was actually bound** — in the
error as well as on the success path. Tiers 3 and 4 remain for convenience, so a fuzzy bind can
still happen and is still silent on `doc(action="get")`, whose `body_meta.heading` echoes the
*request*. `read_file` already discloses it. This is what the second instance under `## Evidence`
argues for, and it is a second **remedy**, not a second claim — so it belongs here rather than in
a new file.

- **Normalise the query and the heading text** — strip leading `#`+space from both sides
  before the tier-1/2 comparisons. This makes `Index` an *exact* match for `## Index` and the
  bug disappears at the root. Risk: it changes what "exact" means for callers who currently
  rely on level-sensitivity (`## Fix` vs `### Fix`); those would need `level` as a separate
  selector.
- **Rank tier 3/4 hits by match quality** rather than document order. Cheaper, but leaves
  `Index` a fuzzy query — a later heading that is a *better* substring match still loses to
  an earlier exact-modulo-prefix one unless quality is defined to include that case.

The first is the real repair; the second is what you take if level-sensitivity must stay.

## Tests added

Three, in `src/tools/file_summary/tests.rs`, and **each has an observed RED from mutating the
production path** — not merely an assertion that exists:

| test | mutation | observed failure |
|---|---|---|
| `resolve_section_range_bare_query_reaches_the_exact_tier` | revert tier-2 normalisation | bound `### One slug …` instead of `## Index` |
| `resolve_section_range_bare_query_matching_two_levels_is_ambiguous_not_silent` | same | returned `Ok(SectionRange { heading_text: "## Fix" })` — the silent pick, verbatim |
| `resolve_section_range_prefixed_query_keeps_exact_level_semantics` | disable tier 1's early return | `## Fix` became ambiguous across `## Fix` / `### Fix` |

The third mutation established a fact previously only reasoned about: **tier 1 is the sole
level-aware step in the cascade.** Nothing else distinguishes `##` from `###`.

**Both fixtures annotate the detail that makes them discriminate**, on the fixture, because a
tidy-up that removes it leaves the test passing and no longer testing anything — a change no
assertion can catch. For the first: the `###` must both PRECEDE `## Index` and contain "Index" as
a substring.

**And the pre-existing bare-form assertion did not cover this.**
`resolve_section_range_finds_last_heading` already asserted that a bare `Resume` resolves to
`## Resume`, and was green throughout — because its fixture holds no earlier heading containing
"Resume", so tier 4's first-match-wins happens to land correctly. It documented the contract and
was structurally unable to detect its violation: green for the wrong reason, and monotone under
exactly this defect. That is the reason the bug survived to be found by hand twice in one day.

The ambiguity test asserts on `err.extra["heading_ambiguous"]` — the named discriminant a caller
one frame up actually reads — rather than on message text, which would still pass if the extra
were dropped and the error collapsed into a plain miss.

## Workarounds

Always pass the `#` markers: `heading="## Index"`, not `heading="Index"`. That reaches tier 1
and is exact. The MCP schemas already model this (`read_file`'s `heading` is documented
`e.g. "## Auth"`), but nothing refuses the bare form or warns that it took a fuzzy path.

## Resume

Decide between the two fix directions above. If normalising: `src/tools/file_summary/file_summary.rs:366`
(tier 1) and the tier-2 comparison immediately below, plus a `level` selector on `HeadingQuery`
to preserve level-sensitivity. Then add the disclosure half — `doc(action="get")` must report
the **resolved** heading in `body_meta`, not echo the request, which is what made this
invisible for the whole session that found it.

## References

- Resolver: `src/tools/file_summary/file_summary.rs:187-232` (`parse_all_headings`),
  `:282-451` (`resolve_section_range`).
- `docs/issues/archive/2026-07-09-artifact-get-heading-exact-match-only.md` — the fix that
  introduced the cascade. It solved the opposite complaint (exact-only, no fuzzy) and this is
  the tier-ordering cost it left behind.
- `docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md`
  — added `occurrence`, which does not reach this case.
- `docs/issues/2026-09-02-closest-match-search-scores-anchors-against-whole-lines.md` — the
  sibling `IC-19` member in the same subsystem: a match score computed against the wrong unit.
- **Also satisfies `IC-6` (`addressing-without-an-escape-hatch`)** on its *two-spellings*
  half: `Index` and `## Index` are two spellings of one heading and only one reaches the
  exact tier. Under this corpus's rule that a finding satisfying a second class's claim is a
  second bug file, that half wants its own file rather than a second tag. Filed here under
  `IC-19` because the observable defect is the **selection by document order**, which is
  `IC-19`'s claim and whose remedy (`the selection, never the marker`) is the one that fixes it.
- Found while auditing the `IC-N` ledger — on `issue-clusters.md`, whose `### One slug, two
  spellings` section warns about exactly this class of two-spelling trap, one heading above the
  section the query failed to reach.
