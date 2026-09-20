---
id: db80a4adc712c971
kind: bug
status: fixed
title: 'BUG: 20 augmentation prompts prescribe dead tool calls, and no gate scans YAML'
tags:
- cluster/doc-contradicted-by-code
claimed_by: 48d1f0c8-9f60-43bb-a15e-17ec7995813a
---

## Summary

`docs/augmentations/*.yaml` `prompt:` fields are **delivered to the model at refresh time** — they
are live instructions, not documentation. Twenty of them prescribe tool calls that no longer exist:
**6 prescribe `artifact_augment(…)`** and **14 prescribe `artifact(action=…)`**. No gate scans this
directory, and no task in the tool-surface-collapse plan claims it.

## Symptom (Effect)

An augmentation refresh delivers a prompt instructing the model to call a tool that was deleted. The
model either emits the dead call and receives an unknown-tool error, or silently works around it.
Nothing in the build reports the staleness.

## Reproduction

At `5da2537d` on `tool-collapse`:

```
grep -rlw 'artifact_augment' docs/augmentations/*.yaml     →  6 files
grep -rl  'artifact(action=' docs/augmentations/*.yaml     → 14 files
```

Named instances include `docs-research-README.yaml:42` and
`docs-trackers-tool-usage-patterns.yaml`. Measured 2026-09-02 by the Opus task review of `5da2537d`.

Then confirm no gate reaches them: `present_tense_surfaces()` (`tests/doc_tool_refs.rs`) enumerates
its scope explicitly and `docs/augmentations/` is not in it; `audit_doc_refs`' default paths are
`docs/**/*.md`, `CLAUDE.md`, `**/README.md` — all markdown, and these are YAML.

## Environment

Branch `tool-collapse` at `5da2537d`. The `artifact_augment` half is branch-specific (Task 5 deleted
that tool); **the `artifact(action=…)` half is already stale on `experiments` too**, since Tasks 1-3
renamed that tool to `doc` — but those tasks are also branch-local. Re-check the counts against
whichever branch you are fixing on: this is exactly the kind of number that differs per checkout.

## Root cause

**A model-facing surface that no instrument treats as one.** Three properties combine:

1. **It is executable prose.** An augmentation `prompt:` is delivered into a model's context at
   refresh time and is acted on. It has the consequence of code and the form of documentation.
2. **It is YAML.** Every doc-scanning instrument in this repo is markdown-scoped —
   `audit_doc_refs`' default globs, `present_tense_surfaces()`' explicit list, `link_scan`'s
   artifact corpus. A `.yaml` file carrying prose falls between them.
3. **No task owns it.** The plan assigns `src/`, `docs/manual/`, `docs/trackers/` and the guides.
   `docs/augmentations/` appears in none of them, so it is not "deferred" — it was never scoped.

Inferred from the scan scopes as written, **and measured** for the counts above; the *delivery*
claim (that these prompts reach the model) is read from the refresh path, not observed at runtime —
confirm before designing a fix.

**The unverified premise is now VERIFIED, on two paths, by running rather than reading** — against artifact `f2ecdd76a6189efb` (`docs/trackers/tool-usage-patterns.md`):

- `doc(action="gather")` returns a top-level `prompt` key whose bytes are the sidecar's `prompt:` block, so it entered a model's context during the task that checked it (`src/librarian/tools/refresh.rs`).
- `librarian(action="context", anchor_id=…)` renders the prompt as `> Standing instruction: READ THIS FIRST …` — a **second delivery path this file did not name** (`src/librarian/tools/context.rs:949`).
- `doctor` reports `sidecar_shape_drift: 0` and `sidecar_unparseable: 0`, and drift compares `live.prompt != prompt` (`augmentation_sidecar.rs:352`), so the committed YAML **is** the live prompt rather than a copy of it.

**What was NOT established, stated because the difference matters:** that the sidecar is *causally upstream* of delivery. Confirming that needs a `reindex` against a catalog with the row deleted, which mutates shared machine-local state. What is observed is equality plus two live delivery paths — enough to act on, and short of a causal claim.

## Evidence

### The shape that makes it invisible

`audit_doc_refs` would flag these immediately if it read them — it resolves symbol and path
references against the filesystem and LSP index. It does not read them because they are not
markdown, and the file-type restriction is a *performance and precision* decision that silently
became a *coverage* decision when prose moved into YAML.

### Why the count is split

`artifact_augment(…)` and `artifact(action=…)` are different staleness generations from different
tasks. Reporting them as one number of 20 would hide that the second group was already stale before
this task began.

## Hypotheses tried

1. **Hypothesis:** these prompts are inert — historical records rather than live instructions.
   **Test:** read the refresh path to see whether `prompt:` is delivered into model context.
   **Verdict:** rejected by reading — the augmentation prompt is the persistent instruction the
   refresh cycle is built around. **Not confirmed at runtime**; do that before fixing.
   **Evidence:** `artifact_refresh(action="gather")`'s contract.

2. **Hypothesis:** `audit_doc_refs` covers them via a `docs/**` glob.
   **Test:** read its default path list.
   **Verdict:** rejected — the defaults are `docs/**/*.md`, `CLAUDE.md`, `**/README.md`. The `*.md`
   suffix excludes YAML.
   **Evidence:** § Reproduction.

## Fix

Two independent halves; do the second even if the first is deferred.

1. **Repair the 20 prompts** — mechanical, once the target names are settled by the collapse
   programme. Do it **after** the renames land, not during, or it will be done twice.
2. **Bring the directory under a gate.** The cheapest form is adding `docs/augmentations/*.yaml` to
   an existing scanner's scope with a YAML-aware reader that checks only the `prompt:` field. The
   check that matters is the same one the markdown surfaces get: *does every tool call named here
   name a live tool?*

**Sequence matters.** Fixing the prose without wiring the gate leaves the next rename to rediscover
this by hand, which is how it reached 20.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

**Half 2 landed 2026-09-20. Half 1 needed no work on this branch, and that is a measurement, not an assumption.** The gate scans all 24 sidecars and flags **0**: every tool call in every prompt names `doc` or `librarian`, both live. Re-derived by this ledger independently of the implementing agent — **49 action-form calls, 42 `doc` and 7 `librarian`, zero retired names** (unit: occurrences of `name(action="`, at HEAD; the agent's own figure of 146 is a wider citation grain, so the two numbers answer different questions and neither corrects the other).

The 20 were measured on branch `tool-collapse` at `5da2537d`, and § *Environment* already warned the count is per-checkout. The collapse programme's sweep has since reached them on `experiments`. So the sequencing rule above — *"do not repair the prose until the renames land"* — was honoured by doing nothing: **this commit repaired no prompt and excluded no path.**

**A consequence worth stating, because it inverts how such a gate usually has to land:** with the population already clean, the check is a **hard assertion that fails the build**, from day one. No reporting-only severity, no known-count baseline, no allowlist. The coordinating brief had prepared two ways to land a gate that reds on 20 existing files; reproducing first made both unnecessary.

This is the same trap `present_tense_surfaces()`' doc comment records from 2026-09-06 — a bug citing 10 dead call sites, all 10 already repaired by the time it was actioned. Wiring was therefore proved by **fixture and mutation**, never by the clean pre-check: a scan that finds nothing is exactly what a scan that runs nothing returns.

## Fix provenance

- **SHA:** `737a29fe` — the YAML branch of the dead-tool gate, plus its `docs/PROBES.md` instrument row (2026-09-20). **patch-id:** `79c64ff0427f983feb6898e438a23910adc5768f`

SHA is positional and dies when `experiments` is rebased; the patch-id is a content hash of the diff and survives rebase and cherry-pick. Derived through a file, never a pipe from `git show`.

**Placement: `tests/doc_tool_refs.rs`, not `audit_doc_refs`, and the reason offered for it was itself stale.** The section above suggested extending an existing scanner's scope; § *Resume* offered *"adding a dedicated check"* as the alternative, and that is what shipped, reusing `tool_names()`, `calls_on_line()`, `billable_bare()` and `stale_tool_call_findings()` so there is one vocabulary and nothing re-derived. The implementing agent justified it by quoting that file's module header — *"`RefKind` has five variants … all five are locations"* — quoted accurately, and **false**: `RefKind` has six, and `ArtifactId` resolves against the catalog rather than the filesystem, its own doc comment opening *"Unlike every sibling, this one does not resolve against the filesystem."* A non-location kind is expressible there. The placement stands on the header's *other* ground — a test needs no wiring to be reachable, and the tool registry has no enumerable form outside `server.rs`'s private test module — so it is a decision, not an impossibility. Header corrected at `e0ceff79`.

## Tests added

None yet. Acceptance is an **observed RED**: a `docs/augmentations/*.yaml` prompt naming a dead tool
must fail a gate. Today it fails nothing.

**Added 2026-09-20:** the YAML branch of `tests/doc_tool_refs.rs` — 22 tests, green from the committed bytes.

**The acceptance RED, on the bar this section set.** A real sidecar's `doc(action="append_entry"` was rewritten to `artifact_augment(action="append_entry"` **in an isolated `mutation-probe.sh` worktree**, never the shared tree, and the gate failed: *"1 augmentation prompt(s) call a tool that does not exist (scanned 24 sidecars, 146 citations, 0 call site(s) suppressed)"*, citing `docs-trackers-codescout-usage-hookify.yaml#prompt:5`. `KILLED (rc=101)`.

**The near-miss is the most reusable part.** A hand-rolled `^prompt:` block-scalar reader was written first and rejected **on measurement**: **6 of the 24 sidecars write `prompt:` as a single-quoted flow scalar**, and all six contain `doc(action=…)` calls. Re-derived by this ledger — 6 flow, 18 block, and the six are `artifact-augmentation-followups`, `claim-decay`, `code-dupes-backlog`, `retrieval-benchmark`, `system-retrospective-improvements`, `test-escape-hardening`. That reader would have silently declined **25% of the population and reported green**: this bug's own shape, one level up, inside the instrument built to fix it. The shipped code parses through the production reader (`augmentation_sidecar::read`) instead, pinned by `a_flow_scalar_prompt_is_read_not_refused`, which also asserts the corpus still holds that form so the test cannot decay into a stale claim.

**Escape and disambiguator, which `IC-6` requires of any parser over a namespace.** Escape: `<!-- doc-tool-refs:ignore `tool` -->`, grammar borrowed from `audit_doc_refs`' `parse_ignore_marker` including its degrade-to-coarse rule — **a distinct token on purpose**, since reusing `audit-doc-refs:ignore` would let a suppression placed for a stale *path* silently cover a dead *tool*. Its limit is stated at the refusal site: suppression is per line, and a flow scalar is one line, so the bare form there suppresses the whole prompt and the message says to use the scoped form. Disambiguator: a YAML citation is addressed `path#prompt:line` (scalar-relative) against markdown's `path:line` (file-relative), because neither the parser nor the sidecar struct carries spans and an honest scalar offset beats a plausible-looking wrong file line. The heredoc — the construct that will be misread — is the block-scalar body, where a line reading as a YAML key is data; that is the second reason for a real parser over a regex.

**Seven mutations, one per guarded SITE, all KILLED**: corpus walk, whole-scalar read, suppression gate, scoped-vs-coarse, address form, refusal accounting, and a real prompt's tool name. Two findings from that run rather than from reading:

- **Mutation 6 was predicted to survive, and did, until a fixture reached it.** No real sidecar is unparseable (`doctor`: `sidecar_unparseable: 0`), so the refusal branch — the one stopping a corrupted sidecar from silently narrowing the gate — was decoration. `an_unreadable_sidecar_is_refused_loudly_not_skipped_silently` was written for it, and the re-run killed.
- **Mutations 1 and 3 left the headline assertion GREEN while gutting the scan.** The gate is monotone under removal; the non-vacuity floors (`>= 15` sidecars, `> 50` citations) are what have teeth, which is why they are floors and not ratchets.

**One blind spot, recorded rather than left to be found:** the YAML half is gated on the `librarian` cargo feature, because it reads through the production reader — so `--no-default-features` compiles the markdown guards and **silently omits this one**. The markdown guards were deliberately left ungated. This is the lean lane's standing vacuity for librarian code, inherited here.

## Workarounds

Before trusting an augmentation's refresh output, check its `prompt:` for tool calls by hand:

```
grep -n 'artifact\|doc(action=' docs/augmentations/<name>.yaml
```

## Resume

Confirm at runtime that `prompt:` reaches model context on a refresh (the one unverified premise
here), then choose between extending `audit_doc_refs`' globs with a YAML branch and adding a
dedicated check. Do not repair the 20 prompts until the collapse programme's renames have landed —
`doc` is not the final surface until Task 13.

## References

- Found during the Opus task review of `5da2537d` (Task 5 of the tool-surface-collapse plan),
  2026-09-02, as review finding M6, flagged as programme-level rather than charged to Task 5.
- `docs/trackers/issue-clusters.md` § `IC-11`. This is a **new surface** for that class — its
  fifteenth member was noted as "a tracker worklist field, the first outside code and schema
  entirely"; an augmentation prompt is the first that is *model-facing and executable*.
- `docs/PROBES.md` — the instrument index. This directory appears in no row.
