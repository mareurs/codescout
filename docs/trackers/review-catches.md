---
id: '3fc46942aafb886e'
kind: tracker
status: active
title: Review catches — defects an external reviewer caught that the author missed (RC-N)
tags:
- review
- lessons
- promotion-queue
topic: review catches and lesson distribution
entry_prefix: RC
expects_augmentation: docs/augmentations/docs-trackers-review-catches.yaml
snapshot_anchor: '| ID | Date | Reviewer | Author | Outcome | Fix | Bug | Cluster | Lesson home | Promotion | Promoted to | Source |'
---

# Review catches — defects an external reviewer caught that the author missed (RC-N)

## Scope and method

**What this holds:** one entry per defect or wrong claim that a reviewer **other than the author** caught. The reviewer might be Codex, an Opus review subagent or a human. For each: what the author had built or claimed, how the reviewer found it, what happened, and the lesson in a form that can be moved to where future work will read it.

**Why it exists:** a correction recorded only in a review document, a bug file or a commit message is read once and then forgotten. Authors repeat defects their own context hides (`observer-blindness`, and the 2026-09-24 model-vs-context experiment, `docs/evals/review-model-vs-context-2026-09-24.md`). Here the lessons live in one queryable place, each with a named home and a promotion status. Distributing them later is then a filter, not a re-reading of the review documents.

**Boundaries:**
- A catch the author made **themselves** is not a row here. That belongs in a session log (F-N or W-N).
- A defect **class** goes to `observer-blindness` (OB-N) or `issue-clusters` (IC-N).
- A **mechanism** built after a lesson goes to `test-escape-hardening` (I-N).
- A row here cites those ids; it does not replace them.

**Seeded 2026-09-27** from the rule-tell local-classifier campaign (2026-09-24 to 2026-09-27): the Codex reviews and their bug files, and the Opus review of `phase1b/step5_gate.py`.

## How to distribute a lesson

1. Filter for `promotion` = `pending`, and group the rows by `lesson_home`.
2. For each group, open the home and grep for the lesson. It may already be there, in other words.
3. Write the lesson in the home's own conventions. For `CLAUDE.md` § *Testing Discipline*, that means a measured instance beside the law, not a new law, unless two or more rows share a mechanism no law states.
4. Set the row's `promoted_to` to `<path>@<sha>` and its `promotion` to `promoted` with `update_entry`, which re-renders the row's line in § *Index* itself. Use `duplicate` or `not-promotable` where those fit.

## Index

**The durable copy of every row's structured fields.** The rows in the catalog's params are
machine-local and not in git; the committed sidecar restores the ledger's shape, never its rows.
So this table is what a fresh clone has, and the promotion queue survives only through it.
**It is kept in step by the tool, not by hand:** the ledger declares a `snapshot_anchor` on the
header below, so `update_entry` re-renders a row's line from params in the same transaction, and
`append_entry` lands a new line when passed `index_row`. A title is in its `### RC-N` heading,
not repeated here.

| ID | Date | Reviewer | Author | Outcome | Fix | Bug | Cluster | Lesson home | Promotion | Promoted to | Source |
|---|---|---|---|---|---|---|---|---|---|---|---|
| RC-1 | 2026-09-24 | Codex | 571eb3d6 | fixed | 0fef5562 (patch-id 0668562a7669dcf3dac496c0d9137a5519328f0e) | `ad8aa199f2f5cfa5` | cluster/capped-result-presented-as-complete | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A missing judgment is not a NO | docs/evals/rule-tell-scoring-2026-09-23.md |
| RC-2 | 2026-09-24 | Codex | 571eb3d6 | fixed | 0fef5562 (patch-id 0668562a7669dcf3dac496c0d9137a5519328f0e) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A claim covers exactly the population and procedure that produced it | docs/evals/rule-tell-scoring-2026-09-23.md |
| RC-3 | 2026-09-24 | Codex | 571eb3d6 | claim-corrected | 0fef5562 (patch-id 0668562a7669dcf3dac496c0d9137a5519328f0e) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A claim covers exactly the population and procedure that produced it | docs/evals/rule-tell-scoring-2026-09-23.md |
| RC-4 | 2026-09-24 | Codex gpt-6-astra medium | 571eb3d6 | fixed | a63adc78 (patch-id 2c568203f402597d7f6958b8dd616225a1646772) | `75fa59bbda9c1ce1` | cluster/value-correct-in-a-frame-its-name-does-not-state | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/label-unit-matches-model-input.md (2026-09-28, not in git) | docs/research/2026-09-24-codex-rule-tell-followup-review.md |
| RC-5 | 2026-09-24 | Codex gpt-6-astra medium | 571eb3d6 | fixed | a63adc78 (patch-id 2c568203f402597d7f6958b8dd616225a1646772) | `6e17aec199b30604` | cluster/value-correct-in-a-frame-its-name-does-not-state | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/new-vs-new-overlap.md (extended 2026-09-28, not in git) | docs/research/2026-09-24-codex-rule-tell-followup-review.md |
| RC-6 | 2026-09-24 | Codex gpt-6-astra medium | 571eb3d6 | claim-corrected | de4c63c1 (patch-id cd76437a8a8bcce82e306f8c6f604bcd8a27cee7) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A claim covers exactly the population and procedure that produced it | docs/research/2026-09-24-codex-rule-tell-followup-review.md |
| RC-7 | 2026-09-24 | Codex gpt-6-astra medium | 571eb3d6 | fixed | c061be8b (patch-id aeb4cc2d4b059afd53ade49b8c13a18553cba66a) | `e5d326668615bbf7` | — | buddy:data-leakage-snow-pheasant | duplicate | buddy data-leakage-snow-pheasant SKILL.md Operating Principle 2 (test set is touched once); cross-referenced from ~/.buddy/memory/data-leakage-snow-pheasant/held-out-protected-by-name.md | docs/research/2026-09-24-codex-rule-tell-review.md |
| RC-8 | 2026-09-24 | Codex gpt-6-astra medium | 571eb3d6 | fixed | c061be8b (patch-id aeb4cc2d4b059afd53ade49b8c13a18553cba66a) | — | — | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/label-unit-matches-model-input.md (2026-09-28, not in git) | docs/research/2026-09-24-codex-rule-tell-review.md |
| RC-9 | 2026-09-24 | Codex gpt-6-astra medium | 571eb3d6 | fixed | 138bdb60 (patch-id a9a38c62218952de6ad091a08a9fb2ff318af434) | `745a98bc8d36c770` | cluster/capped-result-presented-as-complete | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A missing judgment is not a NO | docs/research/2026-09-24-codex-rule-tell-review.md |
| RC-10 | 2026-09-24 | Codex gpt-6-astra medium | 571eb3d6 | fixed | 36ad723d (patch-id 08ded7243c59e9b0c92c78b67463e78de30eb6bd) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A one-shot evaluation owes a run contract | docs/research/2026-09-24-codex-rule-tell-review.md |
| RC-11 | 2026-09-24 | Codex gpt-6-astra medium | 571eb3d6 | claim-corrected | 9c0d2505 (patch-id bea0d8a8d3b6c99b32c922948f380347f751a69c) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A threshold decision is only as sound as the numbers it compares | docs/research/2026-09-24-codex-rule-tell-review.md |
| RC-12 | 2026-09-24 | Codex gpt-6-astra medium | 571eb3d6 | claim-corrected | 9c0d2505 (patch-id bea0d8a8d3b6c99b32c922948f380347f751a69c) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A claim covers exactly the population and procedure that produced it | docs/research/2026-09-24-codex-rule-tell-review.md |
| RC-13 | 2026-09-24 | Codex gpt-6-astra medium | 571eb3d6 | claim-corrected | 9c0d2505 (patch-id bea0d8a8d3b6c99b32c922948f380347f751a69c) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § Freeze the whole decision rule, per comparator, before any data | docs/research/2026-09-24-codex-rule-tell-review.md |
| RC-14 | 2026-09-24 | Codex gpt-6-astra medium | 571eb3d6 | claim-corrected | 9c0d2505 (patch-id bea0d8a8d3b6c99b32c922948f380347f751a69c) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A claim covers exactly the population and procedure that produced it | docs/research/2026-09-24-codex-rule-tell-review.md |
| RC-15 | 2026-09-25 | Codex | 571eb3d6 | fixed | 02511d99 (patch-id ab577faceaf8c6f9aad80ba35a6bcadeb2ef6d92) | — | — | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/label-unit-matches-model-input.md (2026-09-28, not in git) | docs/research/2026-09-25-codex-phase1b-draft-review.md |
| RC-16 | 2026-09-25 | Codex | 571eb3d6 | fixed | 02511d99 (patch-id ab577faceaf8c6f9aad80ba35a6bcadeb2ef6d92) | — | — | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/label-unit-matches-model-input.md (2026-09-28, not in git) | docs/research/2026-09-25-codex-phase1b-draft-review.md |
| RC-17 | 2026-09-25 | Codex | 571eb3d6 | fixed | 02511d99 (patch-id ab577faceaf8c6f9aad80ba35a6bcadeb2ef6d92) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § Compare arms only on the surface they share | docs/research/2026-09-25-codex-phase1b-draft-review.md |
| RC-18 | 2026-09-25 | Codex | 571eb3d6 | claim-corrected | 8254d636 (patch-id da9a0d7d52a01ec03b5c936c885b714306ffba7b) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A control isolates exactly what it varies | docs/research/2026-09-25-codex-phase1b-draft-review.md |
| RC-19 | 2026-09-25 | Codex | 571eb3d6 | fixed | da67db02 (patch-id 99a0f1838c3548255280b22d3ed134fb018fe7e6) | `fe4baee34fb48c15` | cluster/value-correct-in-a-frame-its-name-does-not-state | buddy:testing-snow-leopard | promoted | ~/.buddy/memory/testing-snow-leopard/assert-on-what-was-written.md (2026-09-28, not in git) | docs/research/2026-09-25-codex-stage2-freeze-review.md |
| RC-20 | 2026-09-25 | Codex | 571eb3d6 | accepted-no-change | — | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § Freeze the whole decision rule, per comparator, before any data | docs/research/2026-09-25-codex-stage2-freeze-review.md |
| RC-21 | 2026-09-25 | Codex | 571eb3d6 | fixed | f0125e0e (patch-id f57b7cf16581abab25e3ae878ee25dc8cd7fe146) | `5d4e9ab75d686fed` | cluster/declared-not-wired | buddy:testing-snow-leopard | promoted | ~/.buddy/memory/testing-snow-leopard/assert-on-what-was-written.md (2026-09-28, not in git) | docs/research/2026-09-25-codex-stage3-stage4-review.md |
| RC-22 | 2026-09-25 | Codex | 571eb3d6 | claim-corrected | f0125e0e (patch-id f57b7cf16581abab25e3ae878ee25dc8cd7fe146) | — | — | buddy:ml-training-takin | promoted | ~/.buddy/memory/ml-training-takin/diagnostic-clears-only-its-named-cause.md (2026-09-28, not in git) | docs/research/2026-09-25-codex-stage3-stage4-review.md |
| RC-23 | 2026-09-25 | Codex | 571eb3d6 | claim-corrected | f0125e0e (patch-id f57b7cf16581abab25e3ae878ee25dc8cd7fe146) | — | — | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/masked-labels-over-fire.md (file last modified 2026-09-26, not in git) | docs/research/2026-09-25-codex-stage3-stage4-review.md |
| RC-24 | 2026-09-25 | Codex gpt-6-astra medium | 571eb3d6 | fixed | 8db4aece (patch-id 22594c83b082b211e6a380f7857fd20065f5eae4) | — | — | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/held-out-protected-by-name.md (2026-09-28, not in git) | docs/research/2026-09-25-codex-synthetic-registration-review.md |
| RC-25 | 2026-09-25 | Codex gpt-6-astra medium | 571eb3d6 | fixed | 8db4aece (patch-id 22594c83b082b211e6a380f7857fd20065f5eae4) | — | — | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/held-out-protected-by-name.md (2026-09-28, not in git) | docs/research/2026-09-25-codex-synthetic-registration-review.md |
| RC-26 | 2026-09-25 | Codex gpt-6-astra medium | 571eb3d6 | fixed | 8db4aece (patch-id 22594c83b082b211e6a380f7857fd20065f5eae4) | — | — | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/new-vs-new-overlap.md (extended 2026-09-28, not in git) | docs/research/2026-09-25-codex-synthetic-registration-review.md |
| RC-27 | 2026-09-25 | Codex gpt-6-astra medium | 571eb3d6 | fixed | 8db4aece (patch-id 22594c83b082b211e6a380f7857fd20065f5eae4) | — | — | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/new-vs-new-overlap.md (extended 2026-09-28, not in git) | docs/research/2026-09-25-codex-synthetic-registration-review.md |
| RC-28 | 2026-09-25 | Codex gpt-6-astra medium | 571eb3d6 | fixed | 8db4aece (patch-id 22594c83b082b211e6a380f7857fd20065f5eae4) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A one-shot evaluation owes a run contract | docs/research/2026-09-25-codex-synthetic-registration-review.md |
| RC-29 | 2026-09-25 | Codex gpt-6-astra medium | 571eb3d6 | fixed | 8db4aece (patch-id 22594c83b082b211e6a380f7857fd20065f5eae4) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § Freeze the whole decision rule, per comparator, before any data | docs/research/2026-09-25-codex-synthetic-registration-review.md |
| RC-30 | 2026-09-25 | Codex gpt-6-astra medium | 571eb3d6 | fixed | 8db4aece (patch-id 22594c83b082b211e6a380f7857fd20065f5eae4) | — | — | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/a-pass-bounds-only-what-could-fail-it.md (2026-09-28, not in git) | docs/research/2026-09-25-codex-synthetic-registration-review.md |
| RC-31 | 2026-09-25 | Codex gpt-6-astra medium | 571eb3d6 | fixed | 8db4aece (patch-id 22594c83b082b211e6a380f7857fd20065f5eae4) | — | — | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/label-unit-matches-model-input.md (2026-09-28, not in git) | docs/research/2026-09-25-codex-synthetic-registration-review.md |
| RC-32 | 2026-09-25 | Codex gpt-6-astra medium | 571eb3d6 | fixed | 8db4aece (patch-id 22594c83b082b211e6a380f7857fd20065f5eae4) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § Compare arms only on the surface they share | docs/research/2026-09-25-codex-synthetic-registration-review.md |
| RC-33 | 2026-09-25 | Codex gpt-6-astra medium | 571eb3d6 | fixed | 8db4aece (patch-id 22594c83b082b211e6a380f7857fd20065f5eae4) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § Freeze the whole decision rule, per comparator, before any data | docs/research/2026-09-25-codex-synthetic-registration-review.md |
| RC-34 | 2026-09-25 | Codex | 571eb3d6 | fixed | 1249d6c5 (patch-id be40fa5fba4dc8676a3d22b622d8b260d500d4ca) | — | — | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/label-unit-matches-model-input.md (2026-09-28, not in git) | docs/research/2026-09-25-codex-synthetic-results-review.md |
| RC-35 | 2026-09-25 | Codex | 571eb3d6 | fixed | 1249d6c5 (patch-id be40fa5fba4dc8676a3d22b622d8b260d500d4ca) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § Compare arms only on the surface they share | docs/research/2026-09-25-codex-synthetic-results-review.md |
| RC-36 | 2026-09-26 | Codex | 571eb3d6 | fixed | 14346eb4 (patch-id c453deb0228aff223e47e6346ee778cdcab66d6f) | `5099ff75b5574f28` | cluster/guard-narrower-than-its-name | buddy:testing-snow-leopard | promoted | ~/.buddy/memory/testing-snow-leopard/failure-queued-behind-slow-work.md (2026-09-28, not in git) | docs/research/2026-09-26-codex-phase1b-labelling-preflight-review.md |
| RC-37 | 2026-09-26 | Codex | 571eb3d6 | fixed | 14346eb4 (patch-id c453deb0228aff223e47e6346ee778cdcab66d6f) | `e26fe2d011bb2d0f` | cluster/declared-not-wired | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A one-shot evaluation owes a run contract | docs/research/2026-09-26-codex-phase1b-labelling-preflight-review.md |
| RC-38 | 2026-09-26 | Codex | 571eb3d6 | fixed | 14346eb4 (patch-id c453deb0228aff223e47e6346ee778cdcab66d6f) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A one-shot evaluation owes a run contract | docs/research/2026-09-26-codex-phase1b-labelling-preflight-review.md |
| RC-39 | 2026-09-26 | Codex | 571eb3d6 | claim-corrected | 90e0965d (patch-id b8b24b99bf25fca1c21248b02772c064a7a71ce5) | `1fb2cb7b4614e8b5` | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A control isolates exactly what it varies | docs/research/2026-09-26-codex-phase1b-labelling-preflight-review.md |
| RC-40 | 2026-09-26 | Codex | 571eb3d6 | fixed | 96b52f0c (patch-id ee65c6ff9f755e52e96d05b51b2db8efada7aef8) | `4fc5455c19f81132` | cluster/guard-narrower-than-its-name | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/new-vs-new-overlap.md (2026-09-26, extended 2026-09-28, not in git) | docs/research/2026-09-26-codex-phase1b-stage1-review.md |
| RC-41 | 2026-09-26 | Codex | 571eb3d6 | claim-corrected | 96b52f0c (patch-id ee65c6ff9f755e52e96d05b51b2db8efada7aef8) | — | — | buddy:data-leakage-snow-pheasant | promoted | ~/.buddy/memory/data-leakage-snow-pheasant/a-pass-bounds-only-what-could-fail-it.md (2026-09-28, not in git) | docs/research/2026-09-26-codex-phase1b-stage1-review.md |
| RC-42 | 2026-09-26 | Codex | 571eb3d6 | claim-corrected | 96b52f0c (patch-id ee65c6ff9f755e52e96d05b51b2db8efada7aef8) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A claim covers exactly the population and procedure that produced it | docs/research/2026-09-26-codex-phase1b-stage1-review.md |
| RC-43 | 2026-09-26 | Codex | 571eb3d6 | claim-corrected | 8254d636 (patch-id da9a0d7d52a01ec03b5c936c885b714306ffba7b) | — | — | buddy:ml-training-takin | promoted | ~/.buddy/memory/ml-training-takin/diagnostic-clears-only-its-named-cause.md (2026-09-28, not in git) | docs/research/2026-09-26-codex-phase1b-stage1-review.md |
| RC-44 | 2026-09-27 | Codex | 82cff72e | fixed | c991f226 (patch-id f0bd8c00dafbb5ece2e7e98a445af9f9d7f9559a) | `e76b043bbdd97622` | cluster/record-asserts-an-unchecked-completion | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A one-shot evaluation owes a run contract | docs/research/2026-09-27-codex-phase1b-stop-review.md |
| RC-45 | 2026-09-27 | Opus review subagent (claude-opus-5-5) | 82cff72e | fixed | 5a477c51 (patch-id 0f92d94f10cf7ef443b9abbf584834cd83901c24) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A threshold decision is only as sound as the numbers it compares | docs/evals/phase1b-local-classifier-preregistration.md (Step 5's script) |
| RC-46 | 2026-09-27 | Opus review subagent (claude-opus-5-5) | 82cff72e | fixed | 5a477c51 (patch-id 0f92d94f10cf7ef443b9abbf584834cd83901c24) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A one-shot evaluation owes a run contract | docs/evals/phase1b-local-classifier-preregistration.md (Step 5's script) |
| RC-47 | 2026-09-27 | Opus review subagent (claude-opus-5-5) | 82cff72e | fixed | 5a477c51 (patch-id 0f92d94f10cf7ef443b9abbf584834cd83901c24) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A one-shot evaluation owes a run contract | docs/evals/phase1b-local-classifier-preregistration.md (Step 5's script) |
| RC-48 | 2026-09-27 | Opus review subagent (claude-opus-5-5) | 82cff72e | fixed | 5a477c51 (patch-id 0f92d94f10cf7ef443b9abbf584834cd83901c24) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A one-shot evaluation owes a run contract | docs/evals/phase1b-local-classifier-preregistration.md (Step 5's script) |
| RC-49 | 2026-09-27 | Opus review subagent (claude-opus-5-5) | 82cff72e | fixed | 5a477c51 (patch-id 0f92d94f10cf7ef443b9abbf584834cd83901c24) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § Compare arms only on the surface they share | docs/evals/phase1b-local-classifier-preregistration.md (Step 5's script) |
| RC-50 | 2026-09-27 | Opus review subagent (claude-opus-5-5) | 82cff72e | fixed | 5a477c51 (patch-id 0f92d94f10cf7ef443b9abbf584834cd83901c24) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A missing judgment is not a NO | docs/evals/phase1b-local-classifier-preregistration.md (Step 5's script) |
| RC-51 | 2026-09-27 | Opus review subagent (claude-opus-5-5) | 82cff72e | fixed | 5a477c51 (patch-id 0f92d94f10cf7ef443b9abbf584834cd83901c24) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A threshold decision is only as sound as the numbers it compares | docs/evals/phase1b-local-classifier-preregistration.md (Step 5's script) |
| RC-52 | 2026-09-27 | Opus review subagent (claude-opus-5-5) | 82cff72e | fixed | 5a477c51 (patch-id 0f92d94f10cf7ef443b9abbf584834cd83901c24) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A threshold decision is only as sound as the numbers it compares | docs/evals/phase1b-local-classifier-preregistration.md (Step 5's script) |
| RC-53 | 2026-09-27 | Opus review subagent (claude-opus-5-5) | 82cff72e | fixed | 5a477c51 (patch-id 0f92d94f10cf7ef443b9abbf584834cd83901c24) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@6e416ff5 § A one-shot evaluation owes a run contract | docs/evals/phase1b-local-classifier-preregistration.md (Step 5's script) |
| RC-54 | 2026-09-27 | Opus review subagent (claude-opus-5-5) | 82cff72e | fixed | 5a477c51 (patch-id 0f92d94f10cf7ef443b9abbf584834cd83901c24) | — | — | CLAUDE.md § Testing Discipline | duplicate | CLAUDE.md § Testing Discipline (the monotone law, and its twin: a case exercises only the guard it names); concrete shape in ~/.buddy/memory/testing-snow-leopard/assert-on-what-was-written.md | docs/evals/phase1b-local-classifier-preregistration.md (Step 5's script) |
| RC-55 | 2026-09-28 | peer session 3c5b02df (Claude, codescout-38) | 82cff72e | fixed | a0b265cf (patch-id 2013104cfec03940d4ebfb144b0535420c7b0645) | — | — | memory:eval-design | promoted | .codescout/memories/eval-design.md@add2718d § Freeze the whole decision rule, per comparator, before any data (validity-gate bullet) | docs/superpowers/specs/2026-09-26-system1-base-rate-measurement-design.md § Amendment 7 (b)2 (R109) |
| RC-56 | 2026-09-28 | peer session 3c5b02df (Claude, codescout-38) | 82cff72e | fixed | a0b265cf (patch-id 2013104cfec03940d4ebfb144b0535420c7b0645) | — | — | memory:eval-design | duplicate | .codescout/memories/eval-design.md@6e416ff5 § A claim covers exactly the population and procedure that produced it | docs/superpowers/specs/2026-09-26-system1-base-rate-measurement-design.md § Amendment 7 (b)2 (R109) |

## Catches

Each entry below is condensed from its source document, which remains authoritative. Structured fields (reviewer, fix, bug, cluster, lesson home, promotion) are in the row, and mirrored in § *Index*, which is the copy that survives a fresh clone.

### RC-1 — Score A accepted incomplete rule sweeps and scored a partial group as a clean text

**Missed:** `report_corpus` in `scripts/phase1-span-selector.py` never checked that each (case, side) group held every registered rule exactly once. So one NO row scored as a clean negative text, and the report exited 0.
**Found by:** an offline probe of the extracted functions, fed a single synthetic NO row.
**Lesson:** validate membership and uniqueness before reporting a whole-corpus metric. A missing row is not a NO.

### RC-2 — A verbatim quote was treated as locating the claim, though it proves only that the sentence exists

**Missed:** a YES counted only when it came with a verbatim quote, but nothing checked that the quote landed on the violating sentence.
**Found by:** the review, recorded in the scoring document's *Registered for any next run* section.
**Lesson:** a quote proves the sentence exists, not that it is the violation. Measure claim correctness separately from rule correctness.

### RC-3 — RTD-3's 0/10 was read as the reminder stopping the violation, though 3 of 10 forks were scored before the decision point

**Missed:** the scoring document said binding the rule to the claim "stops the violation". But three of the ten 3-1b forks were scored on a first turn that comes before the decision point.
**Found by:** reading which turn each fork was scored on.
**Lesson:** a score taken before the decision point describes that turn, not what the agent writes after its tool call returns.

### RC-4 — The Stage 2 miner stored the corrected text as the positive's context, a label leak if used as input

**Missed:** `change_blocks` returned only the new side of each hunk, and it was written as the positive sentence's `paragraph`. The corrected twin appeared in 605 of 946 rows.
**Found by:** a whitespace-normalised substring test over the 946 mined rows.
**Lesson:** keep the before- and after-correction contexts apart, and check a target against the side it came from, because the new side carries the answer. The bug file records two open residuals.

### RC-5 — The miner's overlap census counted 20 document pairs sharing a shingle; there are 25

**Missed:** keeping one owner per shingle records star edges (A-B and A-C, never B-C), and that number was published as a count of document pairs.
**Found by:** an inverted index listing every unordered pair of owners.
**Lesson:** a star-edge map preserves connected components, not pairs. Publish a count under the frame it actually measures.

### RC-6 — The verdict that mined pairs 'would bring no rule' to 50 positives went beyond its ten-row sample

**Missed:** the conclusion extrapolated a pooled ten-row manual sample through keyword hints, which are not labels. 615 rows had no hint at all.
**Found by:** reading the derivation against what its instrument can show.
**Lesson:** say what was measured: no rule has yet been shown to reach 50 adjudicated positives.

### RC-7 — The local-classifier plan chose C1 on held-out T while promising T is never read during selection

**Missed:** Stage 4 picked C1 by its T any-fire rate, spending T on selection before presenting it as the held-out evaluation.
**Found by:** reading the two conflicting sections side by side.
**Lesson:** a held-out set used to choose an arm becomes a selection set. Choose on validation, freeze the choice, then read T.

### RC-8 — Stage 2 labelled every other sentence a negative for every rule, and did not keep incident families in one fold

**Missed:** a correction diff is evidence only for its own rule's positive and twin. Every other (sentence, rule) cell is unknown. Incident families could also straddle folds.
**Found by:** reading the labelling contract against what a correction diff can establish.
**Lesson:** a construction labels only what it is evidence for. Mask the rest, and keep families within one fold.

### RC-9 — Score A accepted rows files missing whole case/side texts and reported zero incomplete groups

**Missed:** after RC-1's fix, `report_corpus` still checked only the groups present, never the corpus's expected list of texts. An empty file returned 0.
**Found by:** running the extracted function on an empty input and on a single complete text.
**Lesson:** check the expected set of groups, not only the rule grid of the groups that are present.

### RC-10 — The phase-2 scorer kept no per-row votes or provenance, and ran on any judge profile

**Missed:** per-row decisions were printed as totals and never saved, so they could not be rebuilt without paying for the judge calls again. The scorer also lacked the clean-profile refusal.
**Found by:** reading the scorer's source.
**Lesson:** save each row's ballots and verdict before aggregating, and use one shared clean-channel check.

### RC-11 — JevK5's cannot_happen score on its violation text exactly ties clean-1, so no threshold separates them

**Missed:** the positive was counted among the texts scoring above their rule's clean maximum, though it tied clean-1 exactly (0.600599).
**Found by:** rebuilding the L0 probabilities in the gate's logged order.
**Lesson:** a positive that ties a clean text cannot be separated by any threshold on that rule.

### RC-12 — 'Four of five' L0 texts was false: only three meet both properties together

**Missed:** each of two properties held on four of the five texts, but not on the same four.
**Found by:** the same rebuild of the L0 probabilities.
**Lesson:** count the conjunction itself.

### RC-13 — The stripped-arm prediction was marked 'Holds', though 7/8 is below this run's 9/10

**Missed:** the prediction held against the registered 8/10 but not against this run's 9/10.
**Found by:** arithmetic on the clean-checker cells.
**Lesson:** check a "not below" prediction against each comparator separately. Cells this small establish neither equivalence nor an effect.

### RC-14 — Family-level recall 5/17 was called an upper bound on spec tightening; it bounds only regrouping fixed outputs

**Missed:** regrouping outputs that already exist does not bound what re-specified rules could produce.
**Found by:** recounting form 3's existing rows.
**Lesson:** a bound on regrouping fixed outputs does not bound runs with changed specifications.

### RC-15 — The Phase 1b audit sampled distinct keys, dropping the paragraph context its instruction lets decide the verdict

**Missed:** 5,422 of the 8,303 (unit text, rule) keys occur in more than one distinct paragraph, and the instruction lets the paragraph decide the verdict.
**Found by:** an offline census over the frozen folds with the real segmenter.
**Lesson:** define the adjudication unit by the context actually shown.

### RC-16 — The audit's Wilson bound was estimated on distinct keys, while training consumes every admitted instance

**Missed:** a bound over keys is not a bound over the instance-weighted population that the loss averages over.
**Found by:** the key-multiplicity distribution from the same census.
**Lesson:** state which population a bound covers, and sample the population the loss averages over.

### RC-17 — The draft compared D with L2-1b on final pass/fail after pruning heads separately per arm

**Missed:** different menus alone can change a gate result, so the comparison could partly measure coverage rather than the training change.
**Found by:** reading Step 4's pruning rule against the causal comparison.
**Lesson:** read a causal comparison on the heads both arms keep. This became the common menu.

### RC-18 — Known limits says D controls the threshold half but not calibration, though Step 4 refits both for D

**Missed:** D goes through Step 4, which refits both temperature and threshold.
**Found by:** reading the sentence against Step 4.
**Outcome:** claim-corrected, 2026-09-28, in `8254d636` (patch-id `da9a0d7d52a01ec03b5c936c885b714306ffba7b`). A dated correction beside the registered sentence says that D controls for the combined post-training procedure and separates neither half.
**Lesson:** a control taken through a procedure that refits two things controls their combination, not one of them.

### RC-19 — The Stage 2 freeze asserted counts taken before segmentation, not the positive rows it wrote

**Missed:** `count[rule]` was incremented before `row()` could drop a target, yet it was described as a check on frozen positives. Four rules already differed.
**Found by:** a byte-identical baseline freeze, plus an in-memory mutation that dropped every positive and still passed.
**Lesson:** assert the invariant on the rows actually written, after every filter that can drop them.

### RC-20 — Calibration held only 4 positives each for d_semicolon and member_vs_population, so their thresholds are fragile

**Missed:** the freeze did not flag it.
**Outcome:** accepted, no change. Removals at 4 positives are reported as the small-sample case.
**Lesson:** report the denominator behind any per-rule threshold fitted on a handful of positives.

### RC-21 — Running the synthetic test module directly skipped the FreezeMenuGuard regressions defined after unittest.main()

**Missed:** the class was added below `unittest.main()`, so running the file directly ran 16 tests, not 21. The recorded 21-test pass came from discovery.
**Found by:** comparing a direct run with import discovery.
**Lesson:** keep `unittest.main()` below the last TestCase.

### RC-22 — L1's overfit check and pair alignment were said to rule out an engineering cause; they rule out two named causes

**Missed:** memorising 32 rows and a label-alignment check do not exclude optimiser, hyperparameter or full-data dynamics.
**Found by:** reading the claim against what each check establishes.
**Lesson:** a small-subset overfit and a label-alignment check rule out only their named causes.

### RC-23 — L2's 39% cross-rule firing was presented as the failure's mechanism, though it counts firing on unlabelled cells

**Missed:** 2,614 firings over 6,773 cells are firings on unknown cells, not proven false positives, and a plausible mechanism is not a causal test.
**Found by:** reading the producer of the table.
**Lesson:** a firing rate on unlabelled cells is not a false-positive rate. Already promoted: the Snow Pheasant's `masked-labels-over-fire` memory records this correction.

### RC-24 — The synthetic-pairs audit let test-set audit results decide what enters training

**Missed:** the audits were sampled by (generator, rule) without separating training pairs from T-syn pairs, and drops applied to both.
**Found by:** reading the audit and admission clauses against the rule that test sets are read last.
**Lesson:** split audit populations by generator, split and rule. Only training-side audits may decide training admission.

### RC-25 — The leakage filter did not explicitly protect the seed set S, T-syn-in or T-syn-cross

**Missed:** those sets were not named in the filter, and the inherited "move the smaller group" rule did not forbid moving held-out groups into training.
**Found by:** reading the filter clauses.
**Lesson:** protect every held-out set by name, and resolve a collision by dropping the training-side item, never by moving test data.

### RC-26 — Overlap with T was checked on positive and twin sentences only, missing overlaps through the contexts

**Missed:** 51 non-T rows overlapped T's full context, including admitted positives.
**Found by:** a shingle check over all four text fields.
**Lesson:** check overlap on every field the model actually receives.

### RC-27 — Seed selection allowed the campaign documents that the miner deliberately excludes

**Missed:** 246 eligible paragraphs came from the campaign's own scoring, registration and review documents.
**Found by:** a census against the miner's exclusion expression.
**Lesson:** a lexical overlap filter does not stop campaign outcomes leaking into generation. Exclude campaign documents whole.

### RC-28 — Seed sampling and the pilot were not reproducibly fixed, and the pilot's '22 pairs' contradicted five seeds per call

**Missed:** an RNG seed with no ordered population, algorithm or parser admits several datasets. 22 calls at five seeds each yield 110 pairs, not 22.
**Found by:** reading the sampling clauses.
**Lesson:** a seed alone does not fix a sample. Commit a deterministic extractor and an ordered manifest first.

### RC-29 — Audit size, denominator and execution were underspecified; max(10%, 8) gives 9.5 at 95 pairs

**Missed:** the sample-size rule, what counts as a source, the aggregation and invalid-answer handling were all open, so the drop decision could depend on choices made after the data existed.
**Found by:** arithmetic on the registered rule.
**Lesson:** freeze the formula, the unit, the aggregation and the invalid-answer policy before any data exists.

### RC-30 — Both generators got the same shortcut cues, so cross-generator success could not show rule learning

**Missed:** one construction recipe was shared by both generators, yet cross-generator performance was claimed to show rule learning.
**Found by:** reading the prompt against the claim.
**Lesson:** transfer between generators that follow the same recipe shows transfer under that recipe.

### RC-31 — The mechanical construction checks did not establish sentence-level labels

**Missed:** the checks did not confirm that each sentence is exactly one unit under the training segmenter, and auditors did not judge the replacement inside its paragraph.
**Found by:** reading the checks against the contradiction rule.
**Lesson:** validate generated sentences with the real segmenter, and judge replacements in their full paragraph.

### RC-32 — Independent filtering could break the pairing between the two generators' test sets

**Missed:** pairs were dropped independently per generator, so the two recalls could cover different seed populations.
**Found by:** reading the filtering clauses.
**Lesson:** compare generators only on seed ids that survive in both sets.

### RC-33 — The shortcut prediction named T-syn-in while its rule was defined on T-syn-cross, and the probe's tokenization was not frozen

**Missed:** the prediction and its decision rule named different sets, and an ordinary word tokenizer can discard the `&&` versus `;` cue.
**Found by:** reading the prediction clauses.
**Lesson:** a prediction names its own set's decision rule, and probe features are frozen so that punctuation survives.

### RC-34 — Admitting audits by cell would silently accept individually disputed pairs as training labels

**Missed:** eight pairs marked "disagree" sat in retained cells, three of them disputing the target label itself.
**Found by:** recomputing each cell's decision and locating the disputed pairs.
**Lesson:** a cell-level threshold can admit items already known to be disputed. Register an item-level decision.

### RC-35 — A top-up sized to reach 50 before the final filters could still fall short after the freeze

**Missed:** sizing from pre-freeze counts ignores how many items survive every later filter.
**Found by:** reading the stop rule against the pre-freeze counts.
**Lesson:** size a top-up from what survives every later filter.

### RC-36 — The labeller's stop on a terminal failure waited behind earlier batches, so pending batches still started

**Missed:** results were consumed in input order, so a later batch's second failure went unseen behind a slow earlier one. The existing tests covered per-call retry, not this path.
**Found by:** a fake-model run with batch 0 blocked and batch 1 failing twice: all 21 batches started.
**Lesson:** test the controller path where a later failure sits behind an earlier slow future, and bound pending work with a shared stop signal.

### RC-37 — Relaunching the labeller paid for model calls again and overwrote the prior labels and header

**Missed:** `mkdir(exist_ok=True)` plus reused filenames meant "run once" was never enforced.
**Found by:** a fake-model run into a directory that already held results.
**Lesson:** "run once" needs an exclusive reservation of the output location before any model call.

### RC-38 — The labeller checked cheap preconditions only after spending on the Codex call

**Missed:** the worker count and the Claude channel were validated only after the expensive call had been made.
**Found by:** integration tests run against the pre-fix runner.
**Lesson:** check cheap preconditions before creating outputs or spending.

### RC-39 — The judge-channel control (412 vs 412 tokens) could not show synced skills add nothing: both directories synced them

**Missed:** the control never removed the suspected injection, and equal token counts do not mean equal content.
**Found by:** reading the probe and the design of the comparison.
**Lesson:** a control must remove the suspected mechanism while holding everything else fixed. The blocked-sync control (`90e0965d`) did.

### RC-40 — The counterexample miner checked candidates against the frozen folds only, never against each other across folds

**Missed:** 64 cross-fold shingles touched 28 eligible paragraphs, and the 8 tests stayed green with every shingle check disabled.
**Found by:** the real `--count-only` pool, plus a mutation that disabled shingles and survived.
**Lesson:** compare new items with each other across folds, and test that the filters fail when switched off. Already promoted: the Snow Pheasant's `new-vs-new-overlap` memory.

### RC-41 — A passing permutation null was said to show 'the split carries no link but the labels'

**Missed:** a near-chance shuffled run shows that this run recovered no held-out signal, not that every leakage path is absent.
**Found by:** recomputing the null run's AUC and reading the conclusion against what a null can show.
**Lesson:** a passing null is a diagnostic inside its band, not proof that the split is clean.

### RC-42 — 'The loss only fell from there' went beyond the logging resolution of running means every 200 rows

**Missed:** the maximum was taken over running means logged at intervals, not over every step.
**Found by:** reading the logging frequency against the claim.
**Lesson:** a maximum over logged means is an observation at that interval only.

### RC-43 — A temperature at its lower bound was read as proof that a head's calibration items were perfectly separated

**Missed:** hitting the bound does not prove separation in general, even though all 17 cases here were in fact separated.
**Found by:** checking all 17 cases against their calibration logits.
**Outcome:** claim-corrected, 2026-09-28, in `8254d636` (patch-id `da9a0d7d52a01ec03b5c936c885b714306ffba7b`). A dated correction beside the registered sentence rests the conclusion on the logit check rather than on the bound.
**Lesson:** a parameter on its bound does not prove the property. Check the logits directly.

### RC-44 — The Phase 1b Stage-2 shell wrappers exited 0 after a child failed

**Missed:** each wrapper ended on an `echo`, and `lanes.sh` used a bare `wait`, so a failed run reported success. No recorded child failed.
**Found by:** disposable copies of the wrappers with a fake child exiting 23.
**Lesson:** a wrapper that ends on `echo` or a bare `wait` reports success whatever its children did. Pass each child's status through.

### RC-45 — The gate's claim took the first unit of highest P, so a float-saturated tie at 1.0 quoted an innocent sentence

**Missed:** `sig(z/T)` rounds to 1.0 for z/T above about 36.8. At T = 0.25, which three heads use, two confident units tie, and `max` took the first, an innocent sentence in every span text.
**Found by:** reproduced on the real `span-semicolon` text: logits 12, 30 and 5 fail the span gate.
**Outcome:** fixed by taking the argmax of the raw logit. P and every fire decision are unchanged. No tie occurred in the gate run.
**Lesson:** an argmax over saturating probabilities ties. Choose by the raw score, and record ties.

### RC-46 — A failure after the gate's reservation would lose all evidence, and errored rows kept only a count, never the error

**Missed:** output was captured in memory, and the gate code prints only a count of errored rows. A crash would have left a "running" file that could not be re-run.
**Found by:** reading the reservation and the capture paths.
**Lesson:** a one-shot run writes a crash record, holding the traceback, partial rows and error texts, before it re-raises.

### RC-47 — The gate result did not record which code produced it, and could run from uncommitted bytes

**Missed:** the header held data hashes only, and the script itself was uncommitted.
**Found by:** reading the header fields.
**Lesson:** refuse to run a one-shot evaluation from a dirty tree, and record HEAD plus the hash of every file a verdict depends on.

### RC-48 — 'Each checkpoint's gate runs once' depended on the paths the caller passed, not on the checkpoint

**Missed:** the reservation was `<out-dir>/<filename stem>`, so another directory or a renamed copy would run the gate again.
**Found by:** reading how the reservation is keyed.
**Lesson:** key a run-once reservation to the thing run: a marker beside the checkpoint.

### RC-49 — Nothing checked where common.json came from, and the summary never checked the nine results agree

**Missed:** any `--common` file was accepted, and the summary read gate-ability from the first result only.
**Found by:** reading the inputs' provenance.
**Lesson:** check a derived input against its derivation (nine runs, their intersection), and check that results being pooled share it.

### RC-50 — Predictions 4 and 5 would count an unjudged or errored clean-12 cell as 'did not fire'

**Missed:** a rule absent from the judged rows read as NO, which favours prediction 5.
**Found by:** reading how the summary reads `clean-12`.
**Lesson:** absence of a judgment is not a NO. Refuse to read a prediction from a cell that was not judged.

### RC-51 — The gate scored its texts in worker threads, while parity was checked in the main thread

**Missed:** bit-equality across threads was assumed, never measured.
**Found by:** reading where `sel.sweep` calls the judge.
**Lesson:** compute the scores the verdict reads on the same path that parity verified.

### RC-52 — A NaN logit read as YES, because the NO branch tested p < t

**Missed:** `nan < t` is False, so a NaN fired with a claim, and `max` skips a NaN unless it comes first.
**Found by:** reading the comparison.
**Lesson:** a decision written as "NO if p < t" fires on NaN. Refuse non-finite values before comparing.

### RC-53 — Failed instrument checks left no durable record, and the result file was not written atomically

**Missed:** a refusal wrote nothing, so a retry was invisible, and a crash mid-write could leave a partial file.
**Found by:** reading the refusal and write paths.
**Lesson:** record every refusal, and write results with a temporary file and a rename.

### RC-54 — Step 5's tests gave both menus one gate-ability value and never drove run_checkpoint to a PASS

**Missed:** the summary fixture set own and common gate-ability to the same value, so a test could not tell which one the code read. The only end-to-end test asserted FAIL, which cannot catch a run wrongly recorded as FAIL.
**Found by:** the reviewer's in-memory mutation lens: these mutants survived all 54 tests.
**Lesson:** a fixture that sets two fields equal cannot test which one is read, and a FAIL-only end-to-end test is monotone toward failure. It is an instance of `CLAUDE.md` § *Testing Discipline*'s monotone and fixture-annotation laws.

### RC-55 — Review advice to make lesson assignment a judge-gate pass condition would have changed the fixed go/no-go rule

**Valid:** dated 2026-09-28

**Missed:** reviewing the System 1 base-rate measurement's design, I recommended that the judge gate *require* the specific lesson each RTD case serves, and said this could be added before any data "without touching the go/no-go rule". But the rule's INCONCLUSIVE clause includes "the judge fails its gate", and the gate's thresholds are fixed in the spec. So a new pass condition changes when the rule returns INCONCLUSIVE.
**Found by:** the measurement's author, writing Amendment 7 (R109). It recorded lesson-assignment agreement as REPORTED beside the gate instead (`a0b265cf`).
**Lesson:** a validity gate that can send the verdict to INCONCLUSIVE is part of the decision rule. Adding, removing or tightening one of its pass conditions after registration changes the rule, even before any data exists; report the new check beside the gate instead.

### RC-56 — The review said every RTD gate case records the rule it serves; only the 4 bool prompts do, and 3 of 21 name no law

**Valid:** dated 2026-09-28

**Missed:** the same review told its author that "each gate case in rule-tell-detection.md already records its 'rule served'", after reading one case: RTD-3's bool prompt. Only the four bool prompts (RTD-3, 8, 9 and 10) carry a "rule served" line. Every one of the 21 cases carries a `rule:` field, and three (RTD-8, RTD-9 and RTD-15) record that no law in the corpus names their tell (`docs/evals/rule-tell-detection.md:465`), so `uncovered` is their expected answer.
**Found by:** the measurement's author, checking the premise before revising R109.
**Lesson:** an instance of `eval-design` § *A claim covers exactly the population and procedure that produced it*: one member read, the population claimed. Here it also changed the recommendation's reach, because three cases have no specific lesson to agree on.

## History

### 2026-09-27 — created

Created at the operator's request, after the phase-1b campaign stopped, from the campaign's reviews. The rows are seeded from the review documents and bug files, and cite them.
