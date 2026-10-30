---
id: '4410d44c5394afa7'
kind: bug
status: mitigated
title: A doc comment announcing unbuilt work outlives the work, and sends the next reader to rebuild it
tags:
- cluster/doc-contradicted-by-code
- doc-drift
- doctor
- statement-validity
- forward-reference
closed: 2026-10-30
unverified: 'The CLASS is still unguarded, which is why this is mitigated and not fixed. The INSTANCES in the three files named by the 2026-10-16 sweep are corrected at 4be94370. The four true forward references found on 2026-10-30 (agent/mod.rs:1199, doctor.rs:72, semantic/index.rs:348, semantic/semantic_search.rs:625) are correct today and will rot the same way. The proposed gate was decided against on 2026-10-30, see ## Resume. The count of 306 comment lines naming Phase N or Task N was not read; most may be correct.'
---

# BUG: a doc comment announcing unbuilt work outlives the work, and sends the next reader to rebuild it

## Summary

`scan_dated_stale`'s doc comment in `src/librarian/tools/doctor.rs` said an undeclared entry was
*"a different, **not-yet-shipped** check's business (Task 7, which reports it as undeclared rather
than guessing its age)"*. True when written. Task 7 shipped as `scan_cited_but_undeclared`
(`doctor.rs:3018`, wired into `call` at `:369`) and the sentence three functions above it was never
touched.

On 2026-09-02 a session read that sentence, believed it, and **wrote a tracker entry proposing to
build the check as a mechanism worklist item** — `tracker-hygiene-log:HY-25`, options list, "none
yet". The proposal was retracted before code was written, by the standing rule in CLAUDE.md
§ *Observer Blindness* (*grep `tests/`, `scripts/pre-commit-*` and hooks for a population's name
before any campaign over it*), which surfaced `statements.rs` and four wired `doctor` scans.

## Symptom (Effect)

Nothing. That is the defect. The stale sentence compiles, passes `clippy --workspace --all-targets
--features local-embed`, and passes both test lanes — 8598 tests, zero failures. It reads as an
authoritative statement of system state because it sits inside the subsystem it describes.

The only observable was a session confidently planning duplicate work, and that observable exists
in a transcript, not in any instrument.

## Reproduction

```
$ grep -n 'not-yet-shipped' src/librarian/tools/doctor.rs      # the claim
$ grep -n 'fn scan_cited_but_undeclared' src/librarian/tools/doctor.rs   # 3018 — it exists
$ grep -n 'scan_cited_but_undeclared(' src/librarian/tools/doctor.rs     # 369 — wired into call
$ librarian(action="doctor")   # entry_cited_from_outside_but_undeclared: 2 — it fires
```

Four commands. Any one of them refutes the sentence; nothing runs any of them.

## Environment

codescout `experiments`, `4aba4c3d`. The subsystem is the statement-validity apparatus from
`docs/superpowers/plans/2026-08-20-statement-validity-layers-1-2.md` (Tasks 1–8), shipped
2026-08-20.

## Root cause

**A forward reference is a claim of ABSENCE, and a claim of absence is self-invalidating on the
success path.**

That asymmetry is the whole mechanism, and it runs opposite to how prose drift normally works:

| claim shape | decays when | how often | who is holding the falsifying fact |
|---|---|---|---|
| *"X exists / X does Y"* | X is removed or changed | rare; usually breaks a compile | the remover, whose diff touches X |
| *"X does not exist yet"* | **X is created** | **the point of the project** | the creator, whose diff never touches the sentence |

The Task 7 implementer's diff adds a function and edits `call`. The sentence their success falsifies
lives in a **different function's** doc comment, outside the diff. Self-review of that diff cannot
surface it — the line is not in it. This is not carelessness: a more careful implementer reviewing
the same diff sees the same thing, because the stale line is outside the change's blast radius by
construction.

**And proximity does not help, which is the part worth keeping.** This comment was in the *same
file*, three functions from the code that falsified it — the tightest coupling prose and code can
have short of being the same line. It still rotted. Any remedy premised on "keep the docs next to
the code" is answered by this instance.

## Evidence

**Population, derived not cited.** Forward references in the Rust corpus, by
`///.*(not[- ]yet[- ](shipped|implemented|wired|built|exist)|does not yet|doesn't yet|will be
(added|shipped|wired))` over `src/**/*.rs`, `crates/**/*.rs`, `tests/**/*.rs` — **3 live, of which
1 was stale (this one).** The unit is *doc-comment forward references in Rust sources*; it does not
count markdown, and a wider phrasing would find more.

The two survivors are the interesting half:

- `tests/librarian/goal_eval.rs:64` — *"the synthesizer is not yet wired up"*. **Still true, and
  self-enforcing by accident.** `synthesize` returns `params.clone()`, so every `correct_status`
  rubric fails at T2, and the test carries `#[ignore = "… after API key set + synthesize() wired"]`.
  Shipping the synthesizer makes a test change state. The comment is wired to a check.
- `src/agent/mod.rs:687` — *"Level-2 sub-project pinning within a pinned workspace is not yet
  wired"*. **Still true, and protected by nothing.** `with_project_at` pins at workspace granularity;
  the day someone adds level-2 pinning, this sentence rots exactly as `doctor.rs`'s did, silently.

So of three forward references, the one that survives contact with success is the one whose claim is
**tied to a failing check**, and the two that are pure prose are one shipped feature away from being
wrong. That is a design finding, not a sample size.

**This instance also cost a second false claim downstream.** `HY-25` was published to
`docs/trackers/tracker-hygiene-log.md` at `8f69c314` with `**Mechanism status:** none yet` and a
three-option list recommending exactly the shipped check. Retracted in `4aba4c3d`, struck rather
than deleted. A stale sentence propagated into a *worklist*, which is the artifact shape most likely
to be acted on.

## Hypotheses tried

- **`librarian(action="audit_doc_refs")` should have caught it.** No — and `IC-11`'s
  `**Mechanism status:**` already says why: it lints *references* (paths, symbols, line numbers,
  link targets), so a comment may cite every name correctly and still assert the opposite of what
  the code does. This comment cites no name at all; it describes a state. Verified by reading that
  field rather than by re-running the tool.
- **Rustdoc intra-doc links would catch it.** No, and the direction matters: `[\`scan_cited_but_undeclared\`]`
  warns when the target is **missing**. Here the target's *arrival* is the defect. Intra-doc linking
  is exactly backwards for a claim of absence.
- **`entry_dated_stale` should have caught it** — the check whose own comment this is. No: the
  statement-validity apparatus reads `**Valid:**` declarations in markdown entry sections. It stops
  at the `.rs` boundary. **The subsystem built to detect claim decay has no coverage of claim decay
  in its own doc comments**, which is the sentence this file exists for.

## Fix
### Re-verified 2026-09-24 — instance fixed, class unguarded

Open-bug sweep (`deep-agent-workflow-observations:DWF-7`), verifier evidence. **The instance is fixed:** `4aba4c3d` (not a merge, an ancestor of HEAD, patch-id `2d28df258492fbf2c0a0bf8008bff747a36364f5`, matching the value this file records), and `doctor.rs:4395-4398` now carries the dated retraction. **The class has no guard**, and a live instance still exists: `src/agent/mod.rs:792` reads *"Level-2 sub-project pinning within a pinned workspace is not yet wired"*. That is still true (`with_project_at` pins at workspace level), so it is a correct forward reference today and just as liable to outlive its work.

**Possible new instance, unconfirmed:** re-running this file's own population regex finds `src/retrieval/config.rs:208-211`, which says the shadow warning *"lands with Task 5"*. Task 5a landed on 2026-09-17 as `dotenv_shadowed_fields`, which covers dotenv-vs-config, not `dense_model_name_override`. Whether that sentence is now stale was not settled.

**Corrected in `4aba4c3d`** (patch-id `2d28df258492fbf2c0a0bf8008bff747a36364f5`): the comment now
names `scan_cited_but_undeclared` as shipped, and records that it read "not-yet-shipped" until
2026-09-02 and that a reader proposed rebuilding it. Kept as a dated retraction rather than a clean
rewrite, so the next reader learns the sentence has a history.

**The class is not fixed.** `src/agent/mod.rs:687` carries the same shape today — *"Level-2 sub-project pinning within a pinned workspace is not yet wired (read tools pin at workspace granularity)."* (Re-verified 2026-09-02. This citation read `:672` until then; the file shifted underneath it, which is the `path:line` decay the `**Rests on:**` convention exists to route around — the shape survived, the coordinate did not.)

**Candidate mechanism, in this repo's preferred order** — *make the correct path end in a safe
state*: require a forward reference to name the symbol it waits on, and gate that the symbol does
**not** resolve. Shipping the symbol then reds the gate, pointing at the sentence. That inverts the
asymmetry in § *Root cause*: the success path, which today silently falsifies the claim, becomes the
path that reports it. `goal_eval.rs` already has this property by accident, via a stub that fails a
rubric and an `#[ignore]` naming its precondition — evidence the shape works before anyone builds it
deliberately.

Not built here. It is a new gate over a 3-item population, and this repo's own rule is that a gate
earns its place from a named observer and a measured population, both of which this file now
supplies for a **second** reading to act on.

## Tests added

None, deliberately. The change is one doc comment; a test asserting its wording would pin prose
without touching the class, and would itself become the thing that rots. The population grep in
§ *Evidence* is the artefact to re-run.

## Workarounds

Run `librarian(action="doctor")` before believing any claim about what the librarian does or does
not check. Thirteen entry-validity findings were sitting unread in this project at the time of
filing.

## Partial fix (2026-10-06)

- **SHA:** `5a9ff28b` (`experiments`)
- **patch-id:** `dcb91d509bca46cf8b46f83afceb79e4588d5f9f`

What is covered (comment-only, two instances):

- `src/tools/symbol/call_graph/traversal.rs` lost its stale "Traversal engine is not yet wired into the tool (Task 10); suppress dead_code until then." comment AND the `#![allow(dead_code)]` it justified. The sweep brief reports `clippy -D warnings` stays clean in both the default and `--no-default-features` lanes, so the allow was hiding nothing; this bookkeeping pass did not re-run clippy. **Why this file's own regex missed it:** the removed line was a `//` line comment, and the population regex in `## Evidence` starts at `///`, so the population count there (3 live) was an undercount.
- `src/retrieval/config.rs` `RetrievalConfig::dense_model_name` doc comment reworded from the code and the plan. It had said the shadow warning "lands with Task 5" of `docs/plans/2026-09-17-embedding-config-consolidation.md`. It now says Task 5 landed (5a `dotenv_shadowed_fields`, 5b the `CODESCOUT_EMBEDDING_*` family) WITHOUT a shadow warning for the `CODESCOUT_EMBEDDER_MODEL_NAME` override, and that the provenance that did land (`DotenvProvenance` on `EmbedEnv`) covers dotenv-versus-config for `url` / `model` / `api_key` only. This settles the "possible new instance, unconfirmed" note under `## Fix`: it was stale. The comment also states the override is read outside that path (`RetrievalConfig::from_env_and_project` calls `std::env::var("CODESCOUT_EMBEDDER_MODEL_NAME")` directly, seen at `src/retrieval/config.rs:253`); whether the deprecated-alias machinery in `src/config/embedding_env.rs` also touches that variable was not examined here.

Tests added: none, deliberately (comment-only; see `## Tests added`).

What is NOT covered:

- **The gate for the class** (a forward reference must name a symbol, and the symbol must not resolve) is not built. That is a decision at n=3, deliberately not built; the frontmatter `unverified:` caveat is kept for that reason. With `traversal.rs` found, the population is at least 4 instances, not 3.
- `src/agent/mod.rs` (now line 809: "Level-2 sub-project pinning ... is not yet wired") is still true and was not touched. It is the next scheduled instance; this file's earlier `:687` and the `## Fix` sweep note's `:792` citations have both drifted.
- The warning for `CODESCOUT_EMBEDDER_MODEL_NAME` overriding a configured `[embeddings].model` is genuinely unbuilt: build it or drop the idea.

## Partial fix (2026-10-16)

- **SHA:** `4be94370` (`worktree-agent-a5fed38c0d80e9499`; the integrator's merge onto `experiments` gets a new SHA)
- **patch-id:** `694f8e8969715baff34289830196cd040498afae`

Every comment in the three listed files was checked at the bytes against the code and the plans it cites. Comment-only; the same commit carries the issue-1 fix, so the patch-id covers both.

Stale, and fixed:

- `src/librarian/tools/doctor.rs` `scan_worktree_scoped` doc comment said `fix=reseat_worktree` "(a follow-up change) will need" the surfaced `id_overlap`. `reseat_worktree` shipped (`doctor.rs:2249`) and does not consume `id_overlap`: it leaves a collision row untouched and reports it (test `reseat_worktree_leaves_collisions_for_graft`). The comment now says that and names who the overlap is for (an operator deciding a `doc(action="graft")`).
- `src/agent/mod.rs`, `AgentInner.workspaces` doc: "Phase 1: holds at most one entry ... Phase 3 lifts the clear-on-activate". `ensure_resident` (`:788`) already adds pinned workspaces alongside the default without clearing, so the map holds several entries, and nothing evicts. Reworded to the real behaviour (`activate` still clears the whole map; no eviction; per-`Workspace` locking and eviction are Phase 4b, deferred by decision in `docs/plans/2026-05-30-per-request-workspace-pinning.md` § Phase 4b). The `activate` inline comment tagged "Phase 1" reworded the same way.
- `src/agent/mod.rs`, `AgentInner::default_workspace` doc: "Phase 2+ adds selector-aware twins alongside". They exist (`with_project_at`, `with_project_at_mut`, `project_root_for`, ...). Reworded.
- `src/agent/mod.rs`, `with_project_at_mut` doc: "Phase 4a holds the single `AgentInner` write lock ... Phase 4b moves this onto the per-`Workspace` lock". 4b is deferred by decision, not pending; reworded to say writes to different workspaces still serialize here.
- `src/agent/mod.rs`, `Agent::memory_embedder` doc: "(once the local ONNX path lands) in-process". It landed: `RetrievalClient::build_embedder` calls `codescout_embed::create_embedder_with_config` on the no-url path (`src/retrieval/client.rs:326`). Reworded.

Checked and still TRUE, left alone:

- `src/agent/mod.rs:811` "Level-2 sub-project pinning within a pinned workspace is not yet wired": `with_project_at` resolves `ws.focused_active()` only, so pinning is at workspace granularity. This is the instance the file's `## Resume` told the next session to re-check.
- `src/librarian/tools/doctor.rs:72` "Deferred to a follow-up: NFC unicode normalization, orphan `artifact_augmentation` rows": no NFC or orphan-augmentation check exists anywhere in `src/librarian/` (grepped), and no doc names a follow-up.
- `src/config/embedding_env.rs`: no forward reference. Its claim that every old name is an alias is false for `CODESCOUT_EMBEDDER_MODEL_NAME` (deliberately not folded in, plan `2026-09-17-embedding-config-consolidation.md:369`); agent `cfg` owns that file and fixed the wording there, so it was not edited from here.

Population, re-derived with a widened regex (`//`, `//!` and `///` lines, `not yet wired|built|...`, `does not yet`, `lands with`, `once ... lands`, `follow-up change`) over `src/**/*.rs`: 8 hits; after this commit the only true forward references are `agent/mod.rs:811`, `doctor.rs:72`, `semantic/index.rs:354` ("sync_project does not yet stream incremental progress") and `semantic/semantic_search.rs:625` ("does not yet broaden the search"); the rest are not forward references (`path_security.rs`, `onboarding.rs`, `tool_usage.rs`, `emitters.rs`, the `doctor.rs:4640` retraction). The last two were read for this note and are still true; they are outside the three listed files and were not changed. The `Phase N` / `Task N` tags are a separate vocabulary this regex does not see; the five stale ones above were found by reading, not by it.

Tests added: none, deliberately (comment-only; see `## Tests added`).

## Resume

Every stale forward reference found in the three listed files is fixed on the worktree branch (local, not pushed). Status stays `open` because the class is still unguarded: the gate (a forward reference must name a symbol, and that symbol must not resolve) is not built, and that is a decision, not a bug fix. The population of stale instances found by reading is now at least 8 (`doctor.rs`'s original "not-yet-shipped", `traversal.rs`, `retrieval/config.rs`, and the five fixed above). The next session to touch `with_project_at` level-2 pinning must re-read `agent/mod.rs:811`.

Marius decides (1) whether to build the gate, and (2) whether the `CODESCOUT_EMBEDDER_MODEL_NAME` shadow warning is built or dropped (agent `cfg` is adding one to `src/retrieval/config.rs` on its branch; confirm it merges before closing that item).

**DECIDED BY THE USER ON 2026-10-30: option B, do not build the gate.** Status is `mitigated`: the instances are fixed and the class stays unguarded on purpose. Measured on 2026-10-30 with this file's own regex, widened to `//` and `//!` lines: 4 true forward references remain (`agent/mod.rs:1199`, `doctor.rs:72`, `semantic/index.rs:348`, `semantic/semantic_search.rs:625`). The proposed gate (a forward reference names a symbol, and the symbol must not resolve) cannot see the `Phase N` / `Task N` vocabulary, where five of the eight stale comments found by reading lived; a search for those tags matches 306 comment lines in 81 files (matching lines only, not read). Reopen if a reader acts on another stale forward reference, or if a mechanism that covers the tag vocabulary is designed. Option C (a ratchet that only lets the tag count fall) was offered and not chosen; it is untested and could push people to delete correct references. Item (2), the `CODESCOUT_EMBEDDER_MODEL_NAME` shadow warning, was not examined and stays open as a separate question.

## References

- `IC-11` (`cluster/doc-contradicted-by-code`) — this is member **7**. Its `**Blind party:**` field
  already names this instance's author-side blindness verbatim: *"gaining a capability gives you no
  reason to search prose for sentences your feature just falsified."* What this member adds is the
  **forward-reference sub-form** — the claim decays on **success**, not on change, so its trigger is
  the project working rather than the code moving; and the **proximity refutation** — same file,
  three functions away, still rotted.
- `tracker-hygiene-log:HY-25` — the downstream false claim, and its retraction.
- `docs/superpowers/plans/2026-08-20-statement-validity-layers-1-2.md:1075` — Task 7.
- CLAUDE.md § *Observer Blindness* — the standing rule that caught this before code was written.
- **Not an `OB-N`.** The observer-blindness ledger's bar is a class with a named blind party *and* a
  candidate mechanism; the blind party is nameable here, but at one stale instance this is an
  instance, and that ledger says instances are bug files. It promotes if `agent/mod.rs:687` rots the
  same way, which § *Resume* makes checkable.
