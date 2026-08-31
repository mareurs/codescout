---
id: e613a2692b577b7a
kind: tracker
status: active
title: Session Log — Cluster Promotion (IC-N → OB-N)
owners:
- marius
tags:
- session-log
- clusters
- observer-blindness
- mechanism-design
topic: cluster promotion and mechanism design
entry_prefix:
- F
- W
entry_high_water_F: 1
---

# Session Log — Cluster Promotion (IC-N → OB-N)

> **Purpose:** Two-sided observation log for the work stream promoting `IC-N` defect
> classes (`docs/trackers/issue-clusters.md`) into `OB-N` rows with named mechanisms
> (`docs/trackers/observer-blindness.md`). Frictions are `F-N`, wins are `W-N`.
>
> **Append with one call** — the server allocates the id, writes `## F-N — <title>`,
> records the high-water mark, and stamps `**Valid:**`:
>
> ```
> artifact(action="append_entry", id="<this artifact id>", id_prefix="F",
>          anchor_heading="## Template for new entries",
>          title="<one-line title>", body="**Observed:** ...")
> ```
>
> **Then** add the Index / Wins Index row with the id the call returned. Never
> hand-allocate, never pre-write the row — a pre-written row consumes the id it names.
>
> Status vocabularies, category conventions and the full F-N / W-N entry templates are
> pinned in `docs/templates/session-log.md`; this file follows them rather than
> restating them, so a change there does not leave this copy stale.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-09-01 | med | architectural | open | `OB-7`'s mechanism cites an LSP-verified result to justify a text-search check |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|

---

## Scope note

This work stream is **adjudication and mechanism design**, not bug fixing — the unit of
work is a defect *class*, and its output is an `OB-N` row whose `Mechanism status` field
is honest about what has and has not been built. Bug-level findings belong in
`docs/issues/`; recon findings about the promotion process itself belong here.

Two standing hazards this stream has already produced, recorded so entries can cite them
rather than re-derive them:

- **Resemblance is not membership.** Twice on 2026-08-31 the answer to "does this belong
  in that class?" was no, on the remedy test rather than the description — `IC-8`
  declined for a decayed-conclusion pattern, `IC-10` declined for a verification-reflex
  pattern. Both descriptions fit; neither remedy did.
- **A count and the judgement that quotes it move independently.** The archive backfill
  updated every `Members:` line and left four `Promotes to:` fields reasoning from the
  superseded number. When a count moves, re-derive every judgement quoting it in the
  same pass.

---

## F-1 — OB-7's mechanism cites an LSP-verified result to justify a text-search check, and the substrate gap is a deletion-authorising false positive

**Observed:** 2026-09-01, post-commit reconnaissance of `a2aedd49` (IC-3 → OB-7). Scouting three checkable claims I had committed and reported to the user as verified.

**When:** After promoting `IC-3`, before starting `IC-1`. The seam is `OB-7`'s `Mechanism status` field — the part that makes the entry a worklist item rather than advice.

**Expected (what I wrote in `OB-7` and in the commit message):** *"`references(symbol)` **decides this today** — verified on `chunk_size_for_model` … The check is *non-test caller count == 0*, and the partition needs call-site granularity rather than file granularity, since test modules live inline — which the AST index already carries (`grep` annotates each source hit with its enclosing symbol)."*

**Got (scouted reality):** the partition half is **true and now evidenced**, and the decidability half is **overstated in two ways I did not name.**

1. **Confirmed, and better than asserted.** `grep` does annotate each hit with its enclosing symbol path, and test call sites carry a `tests/` prefix: `[tests/names_path_containing_generalises_and_normalises_separators]` versus `[names_tracker_path]`. The file-granularity trap is real rather than hypothetical — `references` grouped `src/librarian/adapter.rs:1451` under a production file, and `grep` shows it is `tests/…`. That single case is the whole argument for call-site granularity, and I had asserted it without an instance.

2. **The entry names two instruments with different substrates and treats them as one.** I *verified* with `references`, which is **LSP-backed and semantic**; I *described the mechanism* using `grep`'s annotation, which is **text**. That is the substrate distinction this skill's Phase 1 makes central, committed by a session that had the rule loaded.

3. **The substrate gap is a false-positive mode, and it is the dangerous direction.** A symbol reached only through `dyn Trait` dispatch, a function pointer, a macro expansion or a re-export alias has **zero textual non-test hits**, so the grep form flags it dead. `references` would catch trait dispatch; `grep` would not. This codebase is dispatch-heavy by construction — `impl Tool`, `Arc<dyn CodeEmbedder>` — so the blind spot lands exactly where the check would be run. And a false "dead in production" finding **authorises a deletion on a negative search result**, which is this skill's hardest rule.

4. **I ran only a positive control.** Seven symbols probed (`chunk_size_for_model`, `fences_balanced`, `names_path_containing`, `leading_run`, `strip_matching_quotes`, `clean_prefix`, `is_librarian_id`, `literal_continuation_mask`, `min_indent_outside_literals`); **every one confirmed the "has production callers" state.** I never exhibited a symbol with zero non-test callers, so the state the check exists to detect is **unexercised**. Per the skill: a single confirmatory probe cannot reveal a missing state.

**Probable cause:** I generalised from one probe run for a *different* purpose — establishing that a former member is live today — to a claim about an instrument's decision procedure. The two questions share an output and not a warrant. Compounded by writing the mechanism paragraph after the probe rather than before, so the probe never had to match the sentence it was cited for.

**Workaround:** none needed operationally — nothing consumes `OB-7`'s mechanism yet. The correction is to the text: name `references` (LSP) as the instrument and `grep`'s annotation as the partition helper, state the dispatch/macro blind spot, downgrade *"decides this today"* to *"decides for by-name call sites"*, and record that the zero-caller state is unexercised.

**Severity:** med — no failure today, but the next person to build the check builds it on the instrument the entry names, inherits a blind spot concentrated in this codebase's dominant dispatch idiom, and the failure mode's output is a deletion candidate. The cost is bounded only because the entry is four hours old and nobody has acted on it.

**Status:** open — correction not yet written to `OB-7`.

**Valid:** dated 2026-09-01

**Rests on:** the substrate law — *when two instruments disagree, the question is which world each one read* — and the positive-control law: an instrument must be exercised once per state you believe it can report.

**Fix idea / Pointer:** correct `OB-7`'s `Mechanism status` family-1 bullet and `IC-3`'s matching text in one edit; both carry the same sentence.

**Sharpened 2026-09-01, by the peer session, and it is worse than "no negative control".** The
nine probes were **monotone under the exact failure the check is for**. A symbol reached only
via `dyn Trait`, a function pointer, a macro or a re-export alias has zero textual non-test hits
— it presents as the **dead** state. Every one of the nine presented as *has-callers*, and a
symbol with visible textual callers is **by construction not the case that misclassifies**. So
the population was not merely missing a negative control; it was selected such that no member
could be one. Nine confirmations of a proposition none of them could have falsified — which is
`CLAUDE.md` § *Testing Discipline*'s monotone law applied to a probe population rather than to
an assertion, and the two are the same defect.

**What would close it:** seed the probe from the **dispatch** side rather than the symbol side
— enumerate `impl Tool` implementors and `Arc<dyn …>` construction sites, then ask the checker
whether it calls them dead. If it does, the false-positive mode is *demonstrated* rather than
reasoned. If it does not, that is the first genuine negative control.

**Severity is `med` only while nothing reads the mechanism, and should be re-read rather than
inherited when a consumer arrives.** The failure mode is not a wrong report — it is a deletion,
because a false *dead-in-production* finding authorises removal on a negative search result.

**Where the correction landed:** `IC-3`'s half is in codescout `77d4da06`, a **peer's** commit
about `IC-11` backfilling — the edits were staged in the shared index when it committed, and the
warning message was in flight while it did. `OB-7`'s half is in `8ceb9ea9`. Recorded because the
ledger prose cites this entry, so a reader following the correction arrives here and would
otherwise find no pointer back to the commit that carries it.
## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     artifact(action="append_entry", id="<this artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's own level,
     records the high-water mark and stamps `**Valid:** dated <today>` — one write.
     Then add the Index / Wins Index row with the id it returned. -->
