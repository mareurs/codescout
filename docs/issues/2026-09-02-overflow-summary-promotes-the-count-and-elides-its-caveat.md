---
id: '4a00acf19728660f'
kind: bug
status: open
title: 'BUG: the overflow summary promotes a magnitude verbatim and reduces the qualifier that makes it interpretable to a bare key name'
tags:
- cluster/capped-result-presented-as-complete
closed: null
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-08-17-artifact-find-is-silent-about-files-the-catalog-has-never-seen.md
- docs/issues/archive/2026-08-16-content-free-overflow-envelope-costs-a-round-trip.md
severity: medium
---

# BUG: the overflow summary promotes a magnitude verbatim and reduces the qualifier that makes it interpretable to a bare key name

## Summary

When a tool's payload overflows into a `@tool_*` buffer, `describe_payload_shape` renders
top-level **scalars verbatim** but reduces every **object-valued** key to its bare name.
`count` is a top-level number and `hints` is an object — so **the magnitude reaches the
agent and the qualifier that makes it interpretable does not.**

That is the claim, and it is deliberately wider than "warnings get elided". Warnings are a
subset. Three independent instances (§ *Evidence*) elide three different things by one
rule: a completeness notice (`unindexed_files` — the catalog does not hold everything), a
**scope** statement (`hidden_archived` — this number is not of the population you think),
and the same completeness notice again in a live triage derivation. `hidden_archived` is
not a warning at all. **A count without its population is not a weaker answer; it is a
different one, and it arrives shaped exactly like the answer that was asked for.**

The canonical triage query (`artifact(action="find", kind="bug", filter={"status": {"in":
["open", "investigating", "zombie"]}})`) is the exact shape that overflows, so it answers
"what's open?" with a number and hides the field qualifying it.
## Symptom (Effect)

The triage query on a checkout with one unindexed bug file rendered:

```
47 matched:
    c38773037d73d7f9 [open] BUG: one ledger file serializes every class edit, so textually disjoint…
    …
    … +39 more — narrow the filter, or read them from the buffer
  4 keys: count, items, scope, hints
  arrays: items[47]
  count=47
```

The complete answer was **48**. The buffered JSON carried the warning all along:

```json
{
  "more_in_umbrella": 5,
  "more_in_workspace": 19,
  "unindexed_files": 1,
  "unindexed_hint": "1 file(s) under this scope are not in the catalog and cannot match any filter; run librarian(action=\"reindex\") to include them"
}
```

Every one of those four facts was elided into the four letters `hints` in the `keys:` line,
while `count=47` was promoted to its own row. No error, no marker, no `…` — the summary
reads as a complete answer to the question asked.

## Reproduction

At `a8f0befd9ad0cd12f2893a1f077705ae324a090d` on `experiments`:

1. Create a bug file outside the librarian (`create_file` / `Write` / `git`) so the catalog
   has no row for it — e.g. `docs/issues/2026-09-02-staging-is-not-a-state-you-can-hold.md`,
   which arrived this way and carried no `id:` in frontmatter.
2. Run the canonical triage query. The payload exceeds the summary budget
   (`buffered_bytes: 14695`) and is rendered by `describe_payload_shape`.
3. Read the rendered summary: `count=47`, `hints` named but empty of content.
4. Run the same query narrowed to one row. It returns **full JSON inline**, `hints` visible.

Step 4 is the load-dependence: the elision only happens on the overflow path, so **the
warning is hidden exactly when the result set is large** — i.e. when a backlog is big
enough that triage matters and a hand-count is least likely.

## Environment

Linux 7.1.9-zen1-2-zen, codescout MCP over stdio, project `codescout`, branch
`experiments`, `~/.claude-sdd` profile.

## Root cause

`describe_payload_shape` (`src/tools/format.rs:133`) walks **only the top level** of the
object. Three passes over `map`:

- `keys:` — every key's *name* (`src/tools/format.rs:144-149`).
- `arrays:` — `v.as_array()`, so array lengths (`src/tools/format.rs:151-158`).
- scalars — `Value::String` / `Number` / `Bool` rendered `k=v`; **everything else hits
  `_ => None`** (`src/tools/format.rs:159-166`).

`hints` is a `Value::Object`. It is not an array, and it is not a scalar, so it contributes
its name and nothing else. `count` is a `Value::Number` and is promoted verbatim. The
asymmetry is structural, not a budget cut: `MAX_SCALARS = 8` was nowhere near reached (one
scalar was emitted), so no marker fires and nothing is dropped for space.

Measured 2026-09-02: read at the bytes in `src/tools/format.rs:133-173`, and the
load-dependence confirmed by running the same filter at two result sizes (48 rows → summary,
1 row → inline JSON with `hints` present).

Both production callers take this path — `src/librarian/adapter.rs:505` (every librarian
tool's overflow envelope) and `src/tools/core/types.rs:1172` (the core-tool one) — so the
elision is not specific to `find`.

## Evidence

### The alarm exists because a prior bug installed it

`docs/issues/archive/2026-08-17-artifact-find-is-silent-about-files-the-catalog-has-never-seen.md`
filed precisely this failure one layer down, and its Summary names the stakes:

> The project's own conventions make one `artifact(find)` call *the* way to answer "what's
> open?" (`CLAUDE.md`, `get_guide("tracker-conventions")`). That query is therefore
> structurally capable of omitting a just-filed bug and reading as complete.

The fix added `unindexed_files` + `unindexed_hint` to `build_hints`. It works: the field was
computed correctly and was in the payload. What is broken is the path from the field to a
reader.

### The regression test asserts one layer below the read surface

That bug's regression test is
`unindexed_disk_files_surface_a_staleness_hint_then_clear_after_reindex`
(`src/librarian/tools/find.rs`), asserting `hints.unindexed_files == 1` on the **returned
JSON**. The rendering layer is not in its scope, so the test is green while the signal is
unreachable — `CLAUDE.md` § *Testing Discipline*: *"Loudness is a property of a PATH, not of
a failure. An alarm nothing reaches is exactly as informative as no alarm."* The observer
this alarm was written for is an agent reading a rendered summary, and no test covers that
surface.

### Why this is IC-13's second shape, not its headline claim

`docs/trackers/issue-clusters.md` § IC-13 splits the class by the 2026-09-01 measurement:
four members where a cap is genuinely unannounced, and **five where the signal is computed
correctly and unreachable** — *"dropped at a buffering boundary, buried in a nested key the
envelope never names."* This is the second shape with one wrinkle worth recording: the
envelope **does** name the key (`hints` appears in `keys:`). It names the container and
withholds the content, which is weaker than an unnamed key — a reader is told a field exists
and given no reason to think it says anything.

### Three independent instances, three different qualifiers, one rule

All three are `artifact(action="find", kind="bug", …)` calls that overflowed, from **three
sessions across two profiles**. None was found by looking for it.

| session | profile | elided `hints` | kind of qualifier |
|---|---|---|---|
| codescout-e4 (this file) | `.claude-sdd` | `unindexed_files: 1` + reindex pointer | completeness |
| codescout-05 (101396) | `.claude` | `hidden_archived: 1` + `include_archived=true` pointer | **scope** |
| codescout-20 (3857171) | `.claude` | `unindexed_files: 1` + reindex pointer | completeness |

codescout-20's is the costliest and the least ambiguous:

> `count=74` promoted verbatim, `hints` reduced to the bare word `hints`. I reasoned over 74
> for about two minutes. … Had I reported off it I would have told my operator 47 when the
> answer was 48.

codescout-05's is the one that **breaks the original framing**, and is why § *Summary* was
rewritten. In 05's own words:

> What the summariser reliably promotes is the *magnitude* and what it reliably drops is
> *the qualifier that makes the magnitude interpretable*. A count without its population is
> not a weaker answer, it is a different one — and it arrives shaped exactly like the answer
> you asked for.

codescout-20, characterising 05's instance rather than its own, supplied the citation:

> `hidden_archived` is not a warning at all; it is a scope statement, and a count without
> its scope is the failure this repo's own ADR on negative results was written against.

That ADR is `docs/adrs/2026-08-27-negative-results-name-their-scope.md`, and it is the right
one: it requires a negative or bounded result to name the scope it examined.

> **Attribution corrected 2026-09-02, and the error is worth recording rather than just
> fixing.** The `hidden_archived`/ADR sentence was first published here as a quotation from
> **05**. It is 20's. 05 read the file, did not recognise the sentence, and asked for it
> either quoted accurately or restated in this file's own voice — noting that "a quotation is
> a claim about what someone said rather than about what is true", and that a
> plausible-but-wrong quote is the hardest error of the night's several to catch **because
> the person best placed to notice never reads the file**. Two peers' messages arrived four
> minutes apart and one sentence moved between them; nothing in the artifact could have
> flagged it. That is `observer-blindness` shaped, and the only instrument that caught it was
> showing the writeup to the party quoted.

Both sessions volunteered these unprompted and both declined to file anywhere, explicitly
leaving the framing to this file. Neither is a paraphrase — 05 routed 20's instance back to
20 to send first-hand rather than relaying it.

### The rule is per-KEY, not per-size — and that was predicted before it was checked

codescout-05 inferred from the shape alone that *"the rule appears to be per-key rather
than per-size, which should be cheap to confirm against the summariser"* — reasoning that
`count=NN` survives while a sibling key **at the same nesting level** is reduced to its
name, so size cannot be the discriminator.

Confirmed at the bytes, independently and **before that message arrived**
(`src/tools/format.rs:159-166`): the scalar pass matches `Value::String` / `Number` / `Bool`
and sends **everything else** to `_ => None`. An object is dropped regardless of how small
it is. `MAX_SCALARS = 8` was nowhere near reached in any of the three instances — one scalar
was emitted — so no budget was involved and no marker could fire.

The ordering matters for what this is worth: the function was read here before 05's message
landed, so this is independent confirmation of their inference rather than agreement with
it. Two instruments, opposite directions — 05 reasoned from three response shapes to a
per-key rule; this read the rule and predicted the shapes.

And it changes the **kind** of claim the file makes. Three observations support "this has
happened three times"; the function supports **"no payload shape gets a `hints` object
through this path"**. 05's phrasing for the upgrade: from *"observed three times"* to
*"cannot work"*.

So the defect is **mechanically statable rather than empirically observed**: *a non-scalar
value contributes its key name and nothing else, whatever it contains.* Every qualifier the
librarian computes lives in `hints`, which is an object. There is no payload shape in which
a `hints` field reaches a reader through this path.

### The elision is anti-correlated with the reader's ability to catch it

From codescout-20, and it sharpens the severity argument this file makes from
load-dependence:

> The overflow path is exactly where the caveat is most load-bearing: a small result returns
> full JSON and needs no warning, a large one gets summarised and is the case where a reader
> is least able to eyeball the corpus. The elision is therefore not uniform noise — it is
> anti-correlated with the reader's ability to catch it unaided.

Independently reproduced here by running one filter at two result sizes: 48 rows →
summarised with `hints` elided; 1 row → full JSON inline with `hints` present.
## Hypotheses tried

1. **Hypothesis** — the hint was never computed, i.e. the 2026-08-17 fix regressed.
   **Test** — `read_file("@tool_60e6787e", json_path="$.hints")` on the buffered payload of
   the very call that under-reported.
   **Verdict** — rejected. All four hint fields were present and correct.
   **Evidence** — the JSON block in § *Symptom*.

2. **Hypothesis** — the summary dropped `hints` for budget reasons, and a marker would have
   fired at a larger size.
   **Test** — read `describe_payload_shape`'s three passes; check `MAX_KEYS = 24`,
   `MAX_SCALARS = 8`, `MAX_SCALAR_LEN = 60` against the actual payload.
   **Verdict** — rejected. Four keys and one scalar; no bound was approached. The omission
   is the `_ => None` arm, which has no marker by construction.

3. **Hypothesis** — this affects any result size.
   **Test** — same filter narrowed to one row.
   **Verdict** — rejected. Small results return inline with `hints` intact. The defect is
   confined to the overflow path, which inverts the severity: it hides when the answer is
   biggest.

## Fix

Not yet implemented. The narrow fix follows from the root cause: in the scalar pass
(`src/tools/format.rs:159-166`), descend one level into object-valued keys whose contents
are scalars, instead of terminating at `_ => None`.

Prefer the narrow version over a general recursive walk, which re-opens the budget question
`MAX_KEYS` / `MAX_SCALARS` exist to close. `hints` is a flat map of short scalars by
construction at both call sites.

**Assert on the rendered summary string, not on the returned JSON.** Asserting on the JSON
is exactly what let this through — see § *Evidence*. A useful case is the one that makes the
claim mechanical: a payload whose `hints` object is *tiny* must still surface, since size is
not the discriminator.

**Do not scope the fix to `unindexed_files`.** Two of the three instances carry that key and
the third does not; fixing the field rather than the rule would leave the scope case
(`hidden_archived`) silently broken and look green.
## Tests added

None yet. The regression test this bug needs does not exist at any layer: see § *Evidence*
for why the closest existing one (`unindexed_disk_files_surface_a_staleness_hint_then_clear_after_reindex`)
passes throughout.

## Workarounds

When a librarian call returns a `@tool_*` envelope, read the hints explicitly rather than
trusting the summary:

```
read_file("@tool_<id>", json_path="$.hints")
```

For a "what's open?" report specifically, run `librarian(action="reindex")` first — it makes
the undercount impossible rather than merely visible, which is the better order regardless of
this bug.

## Resume

Read `src/tools/format.rs:159-166` (the scalar filter's `_ => None` arm) and add a nested-object
branch that renders scalar leaves for objects under a small key-count bound. Then write the
regression test against the **string** `describe_payload_shape` returns — seed a payload shaped
like a real `find` response (`count`, `items`, `scope`, `hints` with `unindexed_files`) and
assert the output contains `unindexed_files`. Confirm both callers benefit:
`src/librarian/adapter.rs:505` and `src/tools/core/types.rs:1172`.

## References

- `docs/issues/archive/2026-08-17-artifact-find-is-silent-about-files-the-catalog-has-never-seen.md`
  — installed the hint this bug makes unreachable.
- `docs/issues/archive/2026-08-16-content-free-overflow-envelope-costs-a-round-trip.md`
  — why `describe_payload_shape` exists at all; its doc comment cites this file.
- `docs/trackers/issue-clusters.md` § IC-13 — the class, and the 2026-09-01 split between
  "cap unannounced" and "signal computed correctly and unreachable".
- `src/tools/format.rs:133-185` — the function.
