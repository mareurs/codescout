---
id: '4410d44c5394afa7'
kind: bug
status: open
title: A doc comment announcing unbuilt work outlives the work, and sends the next reader to rebuild it
tags:
- cluster/doc-contradicted-by-code
- doc-drift
- doctor
- statement-validity
- forward-reference
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
- `src/agent/mod.rs:672` — *"Level-2 sub-project pinning within a pinned workspace is not yet
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

**Corrected in `4aba4c3d`** (patch-id `2d28df258492fbf2c0a0bf8008bff747a36364f5`): the comment now
names `scan_cited_but_undeclared` as shipped, and records that it read "not-yet-shipped" until
2026-09-02 and that a reader proposed rebuilding it. Kept as a dated retraction rather than a clean
rewrite, so the next reader learns the sentence has a history.

**The class is not fixed.** `src/agent/mod.rs:672` carries the same shape today.

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

## Resume

Decide whether the § *Fix* mechanism is worth a gate at n=3. If not, at minimum re-run the
population grep when `src/agent/mod.rs`'s level-2 pinning lands — that is the next scheduled
instance, and it is predictable **by name and by file**, which is more than most classes offer.

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
  instance, and that ledger says instances are bug files. It promotes if `agent/mod.rs:672` rots the
  same way, which § *Resume* makes checkable.

