---
id: 3edba321dcc159c8
kind: bug
status: archived
title: 'BUG: the markdown citation scanner has no ignore marker, so a prose mention is exempted by a line coordinate any insertion above it falsifies'
tags:
- cluster/addressing-without-an-escape-hatch
---

# BUG: the markdown citation scanner has no ignore marker, so a prose mention is exempted by a line coordinate any insertion above it falsifies

## Summary

`tests/doc_tool_refs.rs` has two producers feeding one verdict function, and they suppress false positives at **different layers**. The sidecar-prompt producer has a producer-side ignore marker that travels with the text. The markdown producer has none, and is exempted instead by a `(file, line, tool)` coordinate table living in a third file. A one-line insertion elsewhere in `docs/PROBES.md` falsified one such coordinate, and the guard went red for every session on the shared checkout for **13h45m54s** — on a commit that touched no Rust and passed every pre-commit check.

The drifted entry does not go inert. It stays live at the line it names, so the same staleness has a second, silent direction.

## Symptom (Effect)

`cargo test` red in both lanes, failing `a_documented_call_names_a_live_tool` (`tests/doc_tool_refs.rs:904`) at `docs/PROBES.md:202`. The failure names a docs line, so it reads as *"a doc I am editing cites a dead tool"* to whoever runs the suite next, whatever their diff contains. Window: `ffeada30` 2026-09-20 20:34:56 +0300 → `70e6c1ad` 2026-09-21 10:20:50 +0300, from `git log -1 --format=%cI` on both; derived independently by two parties who agree.

## Reproduction

```
git show 737a29fe^:docs/PROBES.md | grep -n 'sorted(x, key=f)'   # 201
git show 737a29fe:docs/PROBES.md  | grep -n 'sorted(x, key=f)'   # 201
git show ffeada30^:docs/PROBES.md | grep -n 'sorted(x, key=f)'   # 201
git show ffeada30:docs/PROBES.md  | grep -n 'sorted(x, key=f)'   # 202
```

Against `ANCHOR_FALSE_POSITIVES`' then-current `("docs/PROBES.md", 201, "sorted")`, the fourth line is the red.

**The selection method is the more general defect.** Two defensible selectors for *"when did this break"* **both give the wrong answer**:

| commit | time | touched | mention at |
|---|---|---|---|
| `737a29fe` | 16:42:41 | `docs/PROBES.md` | 201 — unmoved |
| `e0ceff79` | 16:46:49 | `tests/doc_tool_refs.rs`, never `docs/PROBES.md` | n/a |
| `ffeada30` | 20:34:56 | `docs/PROBES.md` | **201 → 202** — the cause |

*What last changed the doc* and *what last changed the test* are both reasonable populations, and neither contains the cause, because **the coordinate that decides the answer — a line's position — is not the one either selector is keyed on.** The 16:46 figure was taken as the breakage time and published to four sessions; a session gating between 16:46 and 20:34 would have been told a valid green was suspect.

**Excluding `737a29fe` needs more than the position trace.** Showing it did not move line 201 rules out displacement, but a newly added row can introduce a fresh finding *on its own content*. The chain that actually excludes it, none of it self-reported:

1. the single table row `737a29fe` added is byte-identical at that commit and at HEAD — `sha256 308e8f0314ac99d7…`, reproduced here from both blobs;
2. `ANCHOR_FALSE_POSITIVES` holds exactly one `docs/PROBES.md` entry (`:853`), so nothing exempts that row;
3. the guard is green over that content now — `cargo test --test doc_tool_refs`, 22 passed / 0 failed, observed on the tree committed as `70e6c1ad`.

Identical bytes, no exemption covering them, green now ⇒ green then.

`git blame` misleads differently: line 202 is authored by `b0c40ec9` (2026-08-26). True of the **prose**, false of the **red**.

### The class reproduced inside its own report

Enumerated, never totalled — a count is the proxy this file is about. Names are omitted deliberately: a registry name is re-minted by compaction or resume while a sessionId is not, and no sessionIds were held here.

1. **The coordinating session** — selected the population by *what touched the test* rather than *what moved the line*, and published the resulting timestamp. Caught by a peer who opened their own saved gate logs instead of accepting it. Did not catch it themselves. **Verifiable from the tree:** `ffeada30` at 20:34:56 against `e0ceff79` at 16:46:49, and `git log -- tests/doc_tool_refs.rs` returning a population that excludes the cause.
2. **A peer** — forwarded a subagent's self-reported `22/0` as an observed measurement. Caught by themselves, and retracted as evidence. **Verifiable from the tree:** the `sha256 308e8f0314ac99d7…` identity of the row added at `737a29fe` and at HEAD, the single `ANCHOR_FALSE_POSITIVES` `docs/PROBES.md` entry, and a green run over that content — the chain that replaced the hand-back.
3. **A second peer** — asserted a line-local marker is *"strictly more precise"* than the coordinate triple; the triple names the tool too, so only the drift axis held. Forwarded as settled, then retracted by them before this file hardened. **NOT verifiable from the tree.** The assertion and the retraction were made in a cross-session exchange that leaves no artifact in the repository; both rest on this file's author's account. The load-bearing half — *retracted unprompted, before the file hardened* — exists only in that exchange.

Deliberately excluded: a third party's explicitly-flagged *suspicion* about the source of the wrong timestamp, retracted unprompted. A guess marked as a guess is not a proxy asserted as the thing.

4. **The coordinating session again** — called all of the above *"re-checkable"*, a property that holds for the first two and had never been tested for the third. Caught by a peer. **Not verifiable from the tree** — exchange only.
5. **The coordinating session again** — wrote two sentences about this same set that cannot both stand: *"two of the three caught their own, and the one who did not was me"* and *"every one caught by a party who did not share the author's premise, and none by the author noticing."* Not a proxy but a flat self-contradiction, introduced in the act of stating the finding. Caught by a peer. **Not verifiable from the tree** — exchange only.
6. **The coordinating session again** — wrote *"none by the author"* as a summary line, two paragraphs below a body of the same message correctly stating that instance 2's author caught their own. Caught by a peer. **Not verifiable from the tree** — exchange only.

Deliberately excluded: a third party's explicitly-flagged *suspicion* about the source of the wrong timestamp, retracted unprompted. A guess marked as a guess is not a proxy asserted as the thing.

The first three are proxies — each one step removed from the thing, returning a plausible answer rather than an error. The last three are not: they are **summary lines contradicting the bodies they summarise**.

**Why this section carries no total, here or anywhere else in the file.** *"None by the author"* is false. **Instance 2 was caught by its own author** — triggered by the claim being consumed — and **instance 3 was retracted unprompted by its own author**. The summary erased the very mechanism this section names: the detectors-as-instances error inverted, deleting detections rather than counting them as instances. It was written in the same message as the body saying otherwise, by a party who demonstrably knew.

> **The gap was not between knowing and not knowing. It was between the body and the line that totals it — summary lines are where claims go to stop being checked.**

So there is no tally in this section, in the closing line, or anywhere below. Each instance carries its own party, act, catcher and provenance; a reader who wants a number must count them and own the counting rule.

### The closing observation, which changes the remedy

The tempting reading is *"authors cannot see their own."* It is wrong, and the accurate version is sharper. Instance 2's author **did** catch their own — but not by re-reading it. They caught it because the claim was **consumed**: their subagent's hand-back was written back to them inside someone else's argument, promoted from self-report to *"a positive observation of green at 16:42"*, and seeing that is what sent them to check what it rested on. Left in their own message it had already sat unexamined once.

> **Self-detection fires when a claim is CONSUMED — not when the author re-reads it, and not when the author knows the class.**

That rescues the asymmetry rather than losing it. Instance 1 went uncaught for hours because nothing downstream had to act on the window until one peer did; it surfaced immediately once someone did. (Formulation credited to the peer who made it, not to the session that had been summarising it wrongly.)

**Why this matters more than the wording.** *"Authors cannot see their own"* argues for review. *"Self-detection fires on consumption"* argues for getting claims consumed sooner — a different mechanism, and the one this thread actually ran on. It also means **a file nobody reads corrects nothing**, which is worth writing down in a bug file, since that is what this is.

Instance 6 is the demonstration: a summary line contradicting its own body, written by a party who had stated the correct version two paragraphs earlier. Knowing was never the missing ingredient.

## Recurrence 2026-09-22 — the second falsification, observed red at HEAD

**This fired again, and this time it was sitting red at `HEAD` for every session in the
checkout.** Found by running `./scripts/gate.sh` on unrelated work; the lean lane failed
`tests/doc_tool_refs.rs` with two findings, neither in the diff being gated.

| what | detail |
|---|---|
| exemption | `tests/doc_tool_refs.rs` — `("docs/PROBES.md", 202, "sorted")` |
| mention actually at | `docs/PROBES.md:203` |
| displaced by | `50986419` (2026-09-21), which inserted the ledger-entry-loss probe row above it |
| prior occurrence | `201 -> 202`, by `ffeada30` — already recorded in the exemption's own comment |

The comment above that tuple **predicted this in writing** — *"ANY row inserted above it in
PROBES.md moves the mention and reds this guard for every session until the number here is
bumped"* — and the prediction did not prevent the second instance, because nothing executes a
comment. That is `CLAUDE.md` § *Observer Blindness*: knowing the class is not a mechanism.

**A second, independent finding came with it, and it is NOT this bug.** `docs/PROBES.md:189`
wrote `references(upsert_int_line)` — a symbol passed as a *value*, which the scanner bills as a
parameter name. That one has a real escape and took it: `references(symbol=upsert_int_line)`,
naming the slot, which is what the guard's own failure text recommends. Worth separating because
the two arrived in one test run and only one of them is a missing-ignore-marker instance.

**Both repaired 2026-09-22** — the exemption bumped `202 -> 203` with the recurrence recorded
on the tuple, and the `:189` citation given its parameter name. **Status stays `open`:** the
line-keyed exemption is re-armed, not removed. Any row inserted above `PROBES.md:203` falsifies
it a third time, which is precisely what this record exists to stop and what the § *Fix* ignore
marker would close.

**Cost datum for whoever prices that fix:** the red reads as a regression in whatever the
reader just committed — it names `docs/PROBES.md`, a file the reader has usually not touched —
and the discriminating check (is `PROBES.md` dirty? if clean, worktree == HEAD, so the red is
pre-existing) is not one the failure output suggests. `git stash` is the reflex here and is
**wrong on this checkout**: a peer's uncommitted work would go with it.

## Denominator 2026-09-24 — all four valid, and both falsifications hit the same one

**Measured at HEAD `0fd2e244`, all four cited files clean in the worktree: 4 of 4 exemptions still find their token at their named line.** Recorded as a denominator rather than dropped as a non-event — § *Testing Discipline* asks that a confirming re-derivation be published, because a population that reports only its catches looks self-correcting.

| exemption | named line | token found at | valid |
|---|---|---|---|
| `docs/TAXONOMY.md`, `for` | 41 | 41 | yes |
| `CONTRIBUTING.md`, `cfg` | 167 | 167 | yes |
| `docs/PROBES.md`, `sorted` | 203 | 203 | yes |
| `src/prompts/guides/tracker-conventions.md`, `find` | 89 | 89 | yes |

**The finding is the DISTRIBUTION, not the total.** § *Fix* says *"the other three carry the same fragility"* — true of the mechanism, and not yet true of the observations. `git log -S` on each exact tuple text returns **one** commit for each of the three non-`PROBES` entries: `89d2ca06`, the one that introduced them. None has been re-pointed since. `docs/PROBES.md` was re-pointed **twice in the same five days** (`70e6c1ad`, `ce9aa15f`).

**That concentration is predictable, which is what makes it useful.** `docs/PROBES.md` is by its own first line a one-page **index of every measurement instrument** — a registry, whose growth mode is inserting rows into a table. The other three citations sit in stable prose, where an edit above them is an ordinary rewrite rather than a scheduled event. So line-keying is fragile in proportion to how often lines are inserted **above** the cited one, and that rate is a property of the containing document's *purpose*, not of the exemption or of the care taken by whoever edits it. (Mechanism inferred from the documents' shape; the two counts above are measured.)

**What it buys whoever prices the § *Fix* remedies:** the observed exposure is concentrated in one entry, so a remedy scoped to registry-shaped documents reaches **both** recorded instances at a fraction of a general ignore-marker mechanism. It is **not** an argument against the general remedy — scope does nothing for the silent direction, and any prose entry can still drift without warning.

**What this check does NOT establish, stated because the gap is the interesting half.** It asks only whether each entry still finds its *own* token, so it is blind to the silent direction named in § *Root cause*: an exemption standing ready to exempt a *future* genuine stale-tool citation that comes to occupy its coordinate. That failure emits nothing, is invisible to this check and to the gate, and nobody has measured it. A clean sweep here is evidence about false positives only.

*(Measured by sessionId `3aa55c01-9663-44ca-82d2-48b6b8d76d66`, who did not author this file, is not holding this bug, and changed no code for it.)*

## Environment

Shared codescout checkout, branch `experiments`. Observed at HEAD `a832ae89` during a gate run; cause at `ffeada30`; instance fixed at `70e6c1ad`.

## Root cause

Two producers converge on `stale_tool_call_findings` (`:866`) and suppress at different layers:

| producer | suppression | where it lives | invariant under insertion? |
|---|---|---|---|
| `anchored_cites()` `:629` (markdown docs) | `ANCHOR_FALSE_POSITIVES` read at `:881`, keyed `(file, line, tool)` | a **third file**, neither the text nor the scanner | **no** |
| `prompt_cites_from()` `:1611` (sidecar prompts) | `prompt_ignore(line)` at `:1630`, dropping the cite **before** the verdict function | **the line itself** | **yes, by construction** |

`prompt_ignore` has exactly one call site. `anchored_cites` consults no marker at all. `ffeada30` could not have broken the sidecar half.

**This is not an unbuilt escape.** `PROMPT_IGNORE_TOKEN` (`:1484`) is designed, built, tested and documented. Its doc at `:1488` opens *"**This scanner owes an escape** (`CLAUDE.md` § *Parsers Over a Namespace*, `IC-6`)"*, and at `:1491` names the alternatives — *"deleting correct history — which is `IC-6` again — or an allowlist entry, **which silences that site for every FUTURE defect as well**."* It borrows its grammar from `parse_ignore_marker` (`src/librarian/tools/audit_doc_refs/parser.rs:670`) including degrade-to-coarse, takes a deliberately distinct token (`:1500`), and is pinned by `the_prompt_ignore_marker_is_the_documented_escape` (`:1893`). It was wired to the **newer, smaller** producer; the older markdown one kept the coordinate table.

**The escape is already in the file that broke.** `docs/PROBES.md:163` carries `<!-- audit-doc-refs:ignore-refs … -->` for the sibling scanner — 39 lines above the citation that had none. Its prose explains the choice: *"Scoped by token, not by section, because this section carries 27 real refs."* `parser.rs:614-620` argues the same case naming this file.

### Both failure directions, and only one is loud

The exemption is a `continue` in the finding loop, matching `*file == c.file && *line == c.line && *tool == c.tool` (`:881-883`). A drifted entry therefore fails **twice**:

- **False positive, loud.** It stops covering the mention it was written for — today's red.
- **False negative, silent.** It stays live at the line it names, standing ready to exempt a genuine stale-tool citation that later occupies `docs/PROBES.md:201`. A guard declining to fire, which this corpus rates worse than a red — and **no commit-time docs guard sees it**, because the edit creating the exposure may be in another file entirely.

## Evidence

- `tests/doc_tool_refs.rs:629` / `:1611` — the two producers.
- `:866`, `:881-883` — the shared verdict function and the exact-triple `continue`.
- `:1630` — the single `prompt_ignore` call site, producer-side.
- `:1484`, `:1488`, `:1491`, `:1500`, `:1893` — the built escape, its `IC-6` debt, its argument against allowlists, its distinct token, its test.
- `:467`/`:477` — `Cite` already carries `text: String`, held so the failure message can quote the author's form.
- `:853` — the one `docs/PROBES.md` entry in the allowlist.
- `src/librarian/tools/audit_doc_refs/parser.rs:614-620`, `:670` — coarse-vs-scoped, argued over this exact file.
- `docs/PROBES.md:163` — the scoped marker already present, for the sibling scanner.

## Hypotheses tried

*The red belongs to `b0c40ec9`, which authored the prose.* Rejected — `blame` attributes text, not position.

*The red belongs to any commit touching `docs/PROBES.md`.* Rejected by the byte-identity chain above, not by the position trace alone.

*Bare-word keying would fix it.* Rejected — it is the regression `the_anchor_false_positive_list_is_keyed_on_the_citation_site_not_the_word` (`:1003`) exists to prevent. **Rejecting it does not vindicate the coordinate**, which was the first reading here and is too generous.

*A line-local marker is strictly more precise than the triple.* **Rejected** — the triple names the tool as well. The marker wins on drift-invariance alone: one axis, the one that broke.

## Fix

Fixed on `experiments`.

- **SHA:** `d2aaf0bd534ba52e6d6eff69eef41890687b44e2`
- **patch-id:** `0b49bdc2ab990b79cf8b7044a1ba09b7b961041c`

Content-anchoring plus the self-check, the two remedies this section had named as complementary. `ANCHOR_FALSE_POSITIVES` is now a list of `AnchorFalsePositive { file, tool, snippet }`, with the snippet matched against the WHOLE citing line (`Cite::source`; `Cite::text` is cut at 110 characters and the `docs/PROBES.md` mention sits far past it). Inserting a row above a mention no longer touches its exemption, and a different line landing on an exempted coordinate is no longer waved through.

`exemption_defects` keeps each entry honest and is run over the real corpus by `every_anchor_exemption_still_names_exactly_one_citation_site`. It reds an entry whose snippet matches no line (the text was edited, which was formerly a silent dead entry), more than one line (refused rather than first-matched, the disambiguator half of `IC-6` that a snippet key would otherwise reproduce), or is empty (the bare-word key the list must not have).

Not adopted. A pre-commit doc scan costs every markdown commit, and the red it would prevent no longer arises from this cause. A line-local ignore marker distributes suppression into the docs, where no guard maintainer sees it. The earlier instance fix `70e6c1ad` (patch-id `e922d74466b4a5e71f1fbdc9b5c73371b319700f`) re-pointed one coordinate and is superseded by this.

## Tests added

Eight, in `tests/doc_tool_refs.rs`, all run in both gate lanes:

- `an_exemption_follows_its_text_when_the_line_moves` — the defect itself: one text at three line numbers, all exempt. Its snippet sits past the 110-character cut, so it also separates `source` from `text`.
- `a_different_line_at_the_same_coordinate_is_not_exempt` — the silent direction.
- `an_exemption_requires_the_file_and_the_tool_to_agree_as_well` — each case agrees with the fixture on every other field.
- `a_healthy_exemption_names_exactly_one_site_and_is_not_a_defect`, `an_exemption_whose_text_is_gone_is_named_not_silently_dead`, `an_exemption_matching_two_lines_is_refused_rather_than_first_matched`, `an_empty_snippet_is_refused_even_when_it_would_match_one_site` — the detector is tested to FIRE, since the corpus assertion is an `is_empty()`, which is monotone under a detector that returns nothing.
- `every_anchor_exemption_still_names_exactly_one_citation_site` — the standing check on the real corpus.

Mutation sweep, one mutation per guarded site against the committed bytes: 13 of 14 killed. The survivor is the sidecar scan's `Cite::source`, which is semantically inert (`covers` tests `file` first, and a sidecar's file is `<yaml>#prompt`, which no exemption names); it is annotated as inert at the site.

## Workarounds

None needed. An edit to an exempted mention now reds `every_anchor_exemption_still_names_exactly_one_citation_site`, which names the entry and says whether its text is gone or has a twin.

## Resume

None; fixed. Adding an exemption is now a `snippet` long enough to name one line; the corpus check refuses one that matches none or several.

## References

- `docs/issues/2026-09-20-the-fix-anchor-check-accepts-a-sha-with-no-patch-id.md` (`5394e9b7bdd83069`, open, `cluster/guard-narrower-than-its-name`) — a guard named for a rebase-durable anchor discharges on the half that dies at rebase. Different mechanism and class, same family: *a check keyed to something that moves, with nothing watching the thing that moves it.*
- `docs/issues/archive/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md` (`2e0135b426faa7dd`, archived, `cluster/authorship-unrecoverable-after-the-fact`) — same reader experience, a shared-build red nobody can attribute.
- CLAUDE.md § *Development Commands* — the `cli_doc` case: a failure that reads as a regression in whatever you just committed.
