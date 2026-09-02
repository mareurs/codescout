---
kind: tracker
status: active
title: 'a truncated window is ordered by a key unrelated to why it was requested'
owners:
- marius
tags:
- defect-classes
- clusters
- truncated-window-ordered-by-the-wrong-key
topic: issue clusters and rule promotion
---

## IC-19 — a truncated window is ordered by a key unrelated to why it was requested

**Slug:** `cluster/truncated-window-ordered-by-the-wrong-key`
**Claim:** A view that must drop items chooses which to keep by an ordering independent of — or exactly opposite to — the criterion that made the call worth making, so the shown sample **systematically** excludes the item that motivated it. The cap is honestly announced, and announcing it harder changes nothing: the remedy is the **selection**, never the marker.
**Members:** `filter={"tags": {"contains": "cluster/truncated-window-ordered-by-the-wrong-key"}}` — **`n=4`, 2026-09-02, by query.** Opened with its membership rather than before it: the first three were `IC-13` members until the 2026-09-01 claim measurement found none of them satisfied that class, and two independent readers then agreed no existing class did (`cluster-promotion-session-log:F-6`). The fourth, `closest-match-search-scores-anchors-against-whole-lines`, was filed 2026-09-02 directly into this class and **judged against `IC-13` explicitly rather than by default** — `IC-13`'s widened clause deliberately excludes a marker the caller can see, and that diagnostic names its own 0.5 threshold in the message, so it fails `IC-13`'s admission and meets this one. Single-party at filing; not re-read by a second reader.
**Blind party:** the author of the window, who holds the **fill order** and not the **criterion** — the two live at different layers, and scan order, line order or insertion order are all locally reasonable defaults. Recorded as a candidate; **not adjudicated for `OB`**, because it is arguable that any caller who inspects the sample can see the mismatch.
**Promotes to:** `not yet` — but note it **clears the count bar** and now clears it wider: **4 instances across 4 subsystems** (`audit_doc_refs` findings, `grep`'s narrowing hint, `preview::headings`/`resolve_section_range`, and `edit_markdown`'s miss diagnostic). Spread and `OB`-routing remain unadjudicated, so this is still a count that clears rather than a promotion earned — the fourth member widens the count and settles neither open question.
**Mechanism status:** none yet — **not checked against the code as of 2026-09-02**, so read this as an open question rather than an established absence.
**Valid:** dated 2026-09-02

Four fill orders, one shape. `audit-doc-refs-gate-hides-its-own-cause` fills by **scan order** while the exit code turns on **severity**, so `exit 1` was returned with zero `high` findings visible in a 50-of-46572 window that honestly reported both numbers. `grep-narrowing-hint-ranks-by-capped-display-count` ranks candidates by **post-cap** counts, so it recommends 3-match files and never names the 20-match one. `append-entry-anchor-is-undiscoverable-through-the-surface-its-error-names` fills **head-first** while the anchor a caller needs is by convention the **last** heading — and that file states the discriminator this class turns on better than any restatement: *"Disclosure and discoverability are different properties… So the remedy is not 'disclose the truncation'."*

The fourth, `closest-match-search-scores-anchors-against-whole-lines`, orders by **whole-line Levenshtein similarity** while the caller's predicate is **substring containment**. It is the class's cleanest instance so far, because the mismatch is a closed-form function rather than a tendency: for an anchor `p` inside a line `L` the score is exactly `|p| / |L|`, so against a 0.5 threshold the diagnostic is silent for **every** anchor shorter than half its line — and short unique anchors are the recommended practice. It also satisfies this class's falsifier in the strong direction: the exclusion is systematic and derivable, never chance. Its one departure from the other three is that the window is not *truncated* so much as *scored and discarded*; the ordering-by-the-wrong-key claim is what it instantiates, and whether this class's title should say "selected" rather than "truncated" is left open rather than decided here.

The class exists because that remedy is genuinely unavailable to `IC-13`. Its members already ship `headings_truncated: true`, `"shown": 50, "total": 46572`, and a sound overflow signal respectively — the marker is present, correct and useless, because it describes *that* something was dropped and never *that the dropped part is the part you asked about*.

**A fourth member is plausible and deliberately not claimed:** `docs/issues/archive/2026-08-08-doctor-outside-roots-sample-is-unranked-and-unreachable.md` (an unranked `SELECT` with no `ORDER BY`, so which 10 rows appear can change after a `VACUUM`) currently sits in `IC-15` on a dropped-`limit` argument, moved there by the 2026-09-01 blind second read. It is a *different* half of the same file, and under this ledger's own rule that a finding satisfying a second class's claim is a second bug file, it wants splitting rather than re-tagging. Left alone pending that.

**Falsified by** a member whose shown window excludes the wanted item by chance rather than by an ordering *systematically* unrelated to the request — that is an ordinary sampling limitation, and this class claims a structural mismatch.
