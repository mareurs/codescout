---
id: '4b6294bf495dbfb3'
kind: tracker
status: draft
title: Goal-Tracker × Audit-Doc-Refs Cross-Pollination ADRs
owners: []
tags:
- adr
- goal-tracker
- cross-pollination
- reflective
topic: null
time_scope: null
---

## Why this exists

The 2026-05-17 deep-pass review of the goal-tracker archetype against its
sibling feature `audit_doc_refs` (the tracker→code sync) surfaced several
architectural decisions that merit their own ADR rather than being buried
in the audit log or the amendment doc. Each decision below has had
alternatives weighed, a verdict reached with stated confidence, and a
revisit condition recorded.

## Options being weighed

(Decisions complete — see Decision deferred / made.)

## Anti-goals

- We are **not** abstracting tracker mergers into a shared trait yet
  (see ADR-1). One concrete impl exists; researcher-tracker and
  goal-tracker are spec-only.
- We are **not** rewriting goal-tracker's augmentation flow to bypass the
  LLM entirely (see ADR-3). The LLM keeps the narrative + freeform signal
  jobs; Rust takes the mechanical predicates.
- We are **not** rewriting the Stop hook in Rust right now (see ADR-4).
  The CLI subcommand inversion is endorsed but deferred to a separate
  refactor.

## Decision deferred / made

---

### ADR-1 — Reject `TrackerMerger` trait extraction (rule of three not met on shared rule rows)

**Decision:** Do **not** extract a `TrackerMerger` trait at this scale.
Keep per-feature merge modules.

**Context:** Three tracker-synth features now exist or are designed:
`audit_doc_refs` (implemented, pure Rust merger at
`src/librarian/tools/audit_doc_refs/merger.rs:5-66`), `researcher-tracker`
(spec-only at `docs/superpowers/specs/2026-05-08-researcher-tracker-design.md`),
and `goal-tracker` (spec-only at
`docs/superpowers/specs/2026-05-16-goal-tracker-design.md`). The temptation
to extract a shared trait at this point is strong — three concretes,
shared `artifact_augment(merge=true)` write path, shared `tracker` kind.

**Alternatives considered:**

1. **Extract `trait TrackerMerger { type Input; type Params; fn merge(...); }`**
   — Rejected. Side-by-side comparison of the three concretes' merge-policy
   tables (primary key, carry-forward, idempotency, status-flip, severity-escalate)
   shows **zero shared rule rows**. The trait collapses into a no-op
   lowest-common-denominator interface that erases audit-doc-refs's
   strongest property (byte-identical idempotence). Lion's Operating
   Principle 4 calls this out: three concretes "by name" but zero shared
   *behavior* is the LCD trap.

2. **Use `render_template` + `params_schema` as the unifying surface
   (status quo).** Adopted. The shared contract is at the storage layer
   (`artifact_augment` enforces additive merge + schema validation
   uniformly). Synth strategy stays per-feature.

3. **Pull only severity-rank-comparison up into a `MergeRule` enum.**
   Rejected for now — only audit-doc-refs has severity-escalate semantics;
   one concrete is not enough.

**Consequences:**
- *Now easier:* each feature owns its full pipeline; failure modes stay
  local; no shared blast radius from a trait change.
- *Now harder:* if a fourth tracker arrives that *does* share merge shape,
  the refactor is bigger because nothing was pre-shaped. Acceptable cost.

**Revisit-when:** A fourth tracker arrives whose merge is byte-identical-
idempotent AND async AND has primary-key carry-forward (3-constraint
match), OR two existing trackers co-change on a shared bug (co-change
signal per heuristic 1).

**Confidence:** High.

---

### ADR-2 — Soften "Container pattern" language in goal-tracker spec

**Decision:** Treat "container pattern" as a post-hoc descriptive label,
not a Capitalized Architectural Pattern. The current spec names it as
the latter; this attracts gravitational pull toward the abstraction.

**Context:** Today there is one container archetype (`goal`). There is no
`Container` trait, no `child_archetype` registry, no plug-point. Naming
the pattern in the spec invites future readers to fit new trackers into
"the container pattern" rather than designing each on its own evidence.

**Alternatives considered:**

1. **Leave spec language as-is.** Rejected — premature pattern naming
   creates premature pattern matching.

2. **Replace "Container pattern" with "container archetype (specific to
   goal)".** Adopted. The lowercase descriptive form preserves the design
   intent without inviting abstraction inheritance.

3. **Extract a `ContainerArchetype` trait or chassis.** Rejected — same
   reasoning as ADR-1 plus there's only one concrete container.

**Consequences:**
- *Now easier:* future container-shaped archetype gets to design its own
  data model from its own pressure, without inheriting goal's shape.
- *Now harder:* if a second container does arrive with materially
  similar shape, the lack of a shared chassis costs duplication.

**Revisit-when:** A second container-shaped archetype is proposed.
Then either extract `ContainerArchetype` for real, or admit they aren't
siblings and let them diverge.

**Confidence:** Low — this is style critique, not architectural defect.
Flagged because the lion is principle-bound to call out premature
abstraction language regardless of whether the code is yet abstracted.

---

### ADR-3 — Adopt variant (b) gather-time injection over variants (a) pre-write hook and (c) new action

**Decision:** Goal-tracker's Rust kernel runs at **gather time**, surfacing
deterministic child statuses as a `deterministic_child_statuses` block in
the augmentation prompt's context. The LLM reads ground truth from
context and copies values verbatim.

**Context:** Yak's refactor seam S4 showed the augmentation pipeline today
does not call Rust mid-prompt. The Rust kernel for child-status reconciliation
(I1) must integrate somewhere. Three variants surfaced:

**Alternatives considered:**

1. **Variant (a) — Pre-write hook in `augment::call`.**
   When `merge=true` and the artifact is a goal-tracker, the augment
   handler runs `goal_aggregation::reconcile(submitted, gathered)` and
   overwrites `children[].status` before merging. Rejected: makes the
   Tool abstraction archetype-aware, inverts trust direction (LLM
   submits, Rust silently rewrites), future debugging surface is
   "why did my params come back different from what I sent?"

   **Exception (narrow):** ADR-7 (plan T10) does adopt a *narrow* form of
   variant (a) — auto-close gate validation on `status: "done"`
   transitions only. The narrowness (refuse a single forbidden
   transition, not silently rewrite anything) is what keeps the trust
   direction intact.

2. **Variant (b) — Gather-time injection.**
   Refresh gathers each child's params, runs the kernel, surfaces results
   as `context["deterministic_child_statuses"]`. Prompt rule 1 collapses
   to "copy verbatim from this block". **Adopted.** Does not change Tool
   trait shape. Reuses the LLM as the writer (keeps audit trail of who
   wrote what). Testable as a pure function plus an integration test on
   the gather path.

3. **Variant (c) — New `librarian(action="reconcile_goal", id=...)` action.**
   Side-channel that bypasses augmentation, computes new params, writes
   back. Rejected — doubles the surface area (two refresh paths is one too
   many), and the Stop hook has to choose which to call (race condition).

**Consequences:**
- *Now easier:* I1 lands as a 5-task refactor (Phase 1 of the plan).
  Idempotency property test is the kernel's pure function being tested
  in isolation, no integration mocking needed.
- *Now harder:* the `gather_goal_children` helper has to know the
  structural shape of a goal-tracker (presence of `acceptance_signals`
  and `children` in params) to decide whether to inject. The detection
  is structural rather than archetype-name based, which is sniffy
  but works.

**Revisit-when:** A second tracker kind wants similar gather-time
augmentation. Then extract a `GatherHook` trait or generalize
`gather_goal_children`. Rule of three guards against extracting now.

**Confidence:** High.

---

### ADR-4 — Endorse Stop hook → Rust CLI subcommand inversion (deferred to follow-up refactor)

**Decision:** The Stop hook's decision logic belongs in Rust (a
`codescout goal stop-decide` CLI subcommand). The bash hook reduces to
~5 lines that exec the binary and emit its stdout. **Endorsed but
deferred** — separate refactor outside Phases 1-4 of the I1 plan.

**Context:** Today's Stop hook (in `codescout-companion`) is bash + jq +
two CLI calls. The 7-branch decision matrix (8 after Hamsa S-2 fix) is
fully deterministic. The original spec acknowledged "the hook prompt is
status-reader only, not progress-judge" — i.e. the LLM has no job.
Despite that, the hook today shells to the CLI; the spec sketch even
proposed a Haiku 4.5 LLM call (never implemented). Today's
implementation is pure bash — but the *contract* still incurs two
binary cold-spawns per Stop event (find + get).

**Alternatives considered:**

1. **Status quo (bash + CLI calls).** Cost: ~50-100ms × 2 forks per Stop
   event. Latency cost real but tolerable for solo use.

2. **Inline Haiku 4.5 call (per original spec sketch).** Rejected — cost
   ~$0.001/turn × N users × M turns to run a switch statement, plus
   ~500ms latency, plus the Hamsa #8 same-model-self-critique trap. The
   spec proposed it; reality didn't ship it; we endorse the omission.

3. **Rust CLI subcommand `codescout goal stop-decide`.** Endorsed. The
   bash hook becomes 5 lines: pipe stdin to `codescout`, exec, emit
   stdout, fail-open on non-zero exit. Logic, branch matrix, fail-open
   semantics, fixture tests all live in Rust where they can be
   `cargo test`'d. The 8 matrix branches become Rust unit tests; the
   bash hook is reduced to a thin wrapper that doesn't need its own
   test matrix.

**Why deferred:** Plan Phases 1-4 are large enough; adding I9 as a
required-prereq would couple this refactor to a CC-specific concern.
The cross-process boundary (bash → codescout binary) is the named
change scenario OP4 requires — that boundary is real, the inversion is
clean, but it doesn't need to land in the same PR as the kernel
extraction.

**Consequences (when implemented):**
- *Now easier:* matrix logic in Rust = `cargo test` + clippy.
- *Now harder:* a new CLI subcommand to maintain; one more surface in
  the codescout binary that codescout-companion couples to.

**Revisit-when:** After Phase 1 of the plan lands, OR sooner if Stop hook
adds any new decision branch (next branch should drive the move).

**Confidence:** High on the inversion; deferred-not-rejected on the
timing.

---

### ADR-5 — Synth-mechanism spectrum framing for tracker features

**Decision:** Treat tracker-synth features as living on a spectrum from
*pure Rust* (audit-doc-refs) through *hybrid* (researcher-tracker)
through *pure LLM* (current goal-tracker). The spectrum is the design
vocabulary, not a strict trichotomy.

**Context:** Lion's deep-pass Q5 surfaced that researcher-tracker is the
*closest to correct shape* of the three — Rust for mechanical extraction
(frontmatter parse, link count, sources_count), LLM for genuine NL work
(topic-cluster summarization, straggler description). Goal-tracker
today is at the pure-LLM end; the spec was written assuming "the LLM
will do everything" because that was the cheap MVP path on the existing
augmentation pipeline.

**Alternatives considered:**

1. **Goal-tracker stays pure-LLM (status quo).** Rejected — see
   audit findings F5, A-1, F-B (NEVER list as architecture-upside-down).

2. **Goal-tracker becomes pure-Rust like audit-doc-refs.** Rejected
   — rule 2 (`acceptance_signals[].met` for freeform signals) is
   genuinely LLM-bearing; pulling it into Rust requires freezing every
   signal into a structured kind, which is the wrong abstraction for
   anything you can't currently parametrize.

3. **Goal-tracker becomes hybrid (variant b adopted in ADR-3):**
   Rust kernel handles mechanical predicates + structured-kind signals;
   LLM handles freeform signals + scope-growth + narrative prose.
   **Adopted.** Matches researcher-tracker's shape, which empirically
   works.

**The spectrum's design heuristic:**

> *LLM should do NL synthesis on top of a Rust-prepared input,
> not arithmetic on top of an LLM-parsed input.*

Audit-doc-refs is pure-Rust because its synth is purely mechanical
(parse → resolve → merge). Goal-tracker is hybrid because its synth
has both mechanical (reconciliation) and judgment (freeform signals,
scope-growth) halves. Researcher-tracker is hybrid because its synth
has both mechanical (frontmatter scan) and judgment (topic clustering)
halves. The spectrum is real; the framing is useful.

**Consequences:**
- Future tracker designs default to hybrid unless one half is empty.
- The phrase "synth-mechanism inversion" enters our design vocabulary
  for catching the pattern that triggered this entire review.

**Revisit-when:** A fourth tracker is designed. If it sorts cleanly into
the spectrum (pure-Rust, hybrid, or pure-LLM) the framing holds. If it
needs a fourth axis (e.g., "async / sync / streaming"), revisit.

**Confidence:** Medium-high. The spectrum is empirically validated by
three concretes. Future cases may stretch it.

---

## History

### 2026-05-17 — All 5 ADRs recorded in one session

ADR-1 through ADR-5 written from the lion + yak deep-pass review.
status: `decided`. Revisit conditions documented per-ADR.

