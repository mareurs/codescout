---
id: '27f6c88314fe6cee'
kind: tracker
status: active
title: Gate contract consolidation — the five transcriptions of one command list, and what replaces them
owners:
- marius
tags:
- gate
- shared-contract
- duplication
- resume-queue
- cross-session
topic: gate contract duplication
entry_prefix: null
---

# Gate contract consolidation — the five transcriptions of one command list

**Deliberately owns no `PREFIX-N` namespace.** This is a resume/handoff document, not a
ledger — nothing here is citable as an entry and nothing allocates ids. See
`docs/TAXONOMY.md` § *Trackers that deliberately own no prefix*. Edit it directly.

**Status 2026-08-30, late:** design **in flight**. `codescout-ae` holds uncommitted
edits to all three of `CONTRIBUTING.md`, `docs/RELEASE.md` and `docs/ROADMAP.md`.
Nothing of this session's is half-applied.

**This document's own count was wrong.** It said four transcriptions; there are
**five**. The fifth — `docs/ROADMAP.md`:93 — was found by `codescout-ae` and verified
independently here against `git show HEAD:docs/ROADMAP.md`. The title, the H1 and the
table below are corrected; the miss itself is written up under *Why the fifth site
survived two sweeps*, because it is the most transferable thing in this file.

---

## Why this exists

The four-command gate is transcribed independently into **five** places. Each is a
separate copy of one contract, so each rots on its own, and **a reader of any one of
them sees a complete-looking list**. That is what makes the drift invisible rather than
merely present: nothing looks wrong at any single site.

Measured 2026-08-30, after `4c88e129` had already swapped `cargo test` →
`cargo test --workspace` at some of them:

| site | lean lane | clippy form | notes |
|---|---|---|---|
| `CLAUDE.md` § Development Commands | ✅ | wide (correct) | the copy an agent executes; reordered at `73066479` |
| `docs/RELEASE.md` `# 3. Full gate` (~:290) | ❌ **missing** | ❌ **neither bare nor gate form** | heading calls itself *Full gate* |
| `docs/RELEASE.md` `# 2. Build release binary and verify` (~:33) | ❌ **missing** | — | also missing `cargo fmt` entirely |
| `CONTRIBUTING.md` § Before Submitting a PR (~:110) | ❌ **missing** | both forms present | contributor-facing, meant to be copy-pasted |
| `docs/ROADMAP.md`:93 — **prose, parenthetical** | ❌ **missing** | ❌ bare form | **found fifth**; stale since 2026-08-06 |

A sixth block at `docs/ROADMAP.md`:107–111 is **not** a transcription and is correctly
excluded from the count: it is a *proposal* for a `cargo xtask gate`, labelled as one.
It is worth reading anyway, because it is the only place in the repo that lists a test
lane for the third CI config — `cargo test --features local-embed --no-default-features`
— which today's gate covers with **clippy but not with test**. That gap is real and
unclaimed; it is not part of this consolidation.

## Why the fifth site survived two sweeps

Two sessions enumerated these sites independently, and **both returned four**. The
agreement was not corroboration — it was the same defect run twice.

Both sweeps searched for the gate's **shape**: a fenced block containing `cargo test`.
`docs/ROADMAP.md`:93 states the same proposition inline, in parentheses, mid-sentence —
``CLAUDE.md's pre-commit line (`cargo fmt`, `cargo clippy -- -D warnings`, `cargo
test`) covers **one of nine** CI test cells`` — so it is invisible to that search and to
any refinement of it. The failure returns a clean count with nothing marking the
omission, which is the property that made it survive: a sweep that missed a site looks
exactly like a sweep over a corpus that has none.

> **Enumerate by proposition, not by shape.** "Every place that states the gate" and
> "every fenced block listing cargo commands" are different queries with different
> answers, and only the second one is easy to write.

It was also the **stalest** of the five — three commands, unrevised since 2026-08-06,
predating both the wide clippy form and the lean lane. Sites that evade a search evade
every subsequent maintenance pass for the same reason, so the one nobody finds is the
one that drifts furthest. Prose copies should be expected to be the worst copies.

And it compounded: the same paragraph pointed readers at `docs/RELEASE.md` §
*Large-Cohort Promotion* as the "full gate definition" — the weakest of the five. **A
reader who did the right thing, distrusting the local prose and following the pointer,
landed on the copy that lints neither `crates/codescout-embed` nor the `local-embed`
module.** A stale pointer is worse than a stale copy: it converts the careful reader's
correct instinct into the wrong destination.
## The clippy finding — a form that reaches neither the crate nor the gated module

`docs/RELEASE.md:292` read `cargo clippy --all-targets -- -D warnings`. That is neither
the bare form nor the gate form. **Deleted by `bdfd7a62`** — the block now references
`CLAUDE.md` instead of restating it, so the finding below is history, not open work.

Verified at the bytes: `Cargo.toml` declares `members = [".", "crates/codescout-embed"]`,
so the root is a real package and **default package selection is the root alone**.
`--all-targets` selects target *kinds*, not packages. So that line lints the root
package's test targets and **never reached `crates/codescout-embed`** or the
`local-embed`-gated `local` module — precisely the hole CLAUDE.md's *"the gate, not
garnish"* paragraph exists to close. It sat under a heading that called itself the Full
gate, on the promotion path.

Also verified: `ci.yml`'s test matrix is **three** configs — `default` (flags `""`),
`local-embed`, and `no-features` (`--no-default-features`). The lean lane mirrors the
third exactly.

### Current state, and why it is stated as a property rather than a count

This heading used to say *"a fourth variant"*. **The ordinal is withdrawn rather than
corrected** — it was a count over a set nobody had enumerated, and it would have gone
stale the moment anyone added another clippy line, with no edit touching it. `CLAUDE.md`
already states the rule for the two test lanes: *"an ordinal is a positional reference,
correct for exactly one arrangement and silently wrong after any reorder."*

Enumerated and re-verified after `bdfd7a62`, live invocations only (comment lines
excluded):

| form | where |
|---|---|
| `cargo clippy -- -D warnings` | `ci.yml:50` — bare |
| `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` | `ci.yml:61` — **the gate form** |
| `cargo clippy --features server-stack --all-targets -- -D warnings` | `ci.yml:213` — server-stack lane |
| `scripts/build-windows.sh clippy --all-targets -- -D warnings` | `ci.yml:284` — cross-compile **wrapper** |

> **Four distinct forms, and CI executes every one of them. No form exists only in
> documentation.** That is the property worth holding, and unlike a count it is
> checkable in one grep and says what actually matters — nothing a reader could mistake
> for the gate lives anywhere unexecuted. The last two rows are legitimately different
> jobs, not drift; they are listed so the next reader does not "consolidate" them.
> `CONTRIBUTING.md:111-112` and `CLAUDE.md` carry the first two forms as instructions,
> which is documentation of a form rather than a fifth form.

**Two corrections landed on this section within an hour, both from the class this whole
document is about.** The first: it read *"five distinct forms, three of them inside
CI"* while listing four CI sites immediately below — a count contradicting its own
enumeration, caught by `codescout-ae`. The second is theirs and supersedes it: `bdfd7a62`
had already deleted `RELEASE.md`'s form, so five was stale before it was written.

**And `ci.yml:284` was itself nearly missed the same way.** `codescout-ae` grepped
`cargo clippy` to check this enumeration before disputing it, got three hits, and was
one keystroke from replying that the fourth form did not exist. It is a **wrapper**
invocation — `scripts/build-windows.sh clippy …` — so it contains no `cargo clippy`
substring and is invisible to any audit keyed on one, which is every audit anyone would
naturally write. Same structure as the `ROADMAP.md:93` miss, different surface: a query
keyed on a token the site does not contain returns a clean count with nothing marking
what the key excluded.

What caught it was **not attention**. It was a policy — *verify before contradicting a
peer* — which runs whether or not the reader suspects anything. That is the same lesson
as the counting-rule relapse recorded under *Coordination state*: knowing the class did
not prevent the instance in either case, and a procedure did. Two sessions, one evening,
three instances of one class, every one committed while writing about it.
## The reframe, and why it is better than what both of us planned

Both authorised diffs were *"add the missing lean lane"*. The operator's redirect was:

> *"we don't want too much duplicate info, not even in trackers. maybe references is
> enough."*

That **dissolves the class instead of patching instances**, and it retro-explains the
drift: each site is an independent transcription, so each can rot independently. Adding
a fourth correct copy would have set up the fifth divergence.

It also **deletes** the clippy defect rather than correcting it. A corrected line can
drift again; a removed line cannot. That is the strongest single argument for the
reframe.

## The open judgement call — which copies survive

**Resolved 2026-08-30 by `codescout-ae`, and their resolution is better than the
proposal this section previously held.** Kept below in full, because the difference is
the useful part.

- **`CLAUDE.md` stays inline** — the copy a Claude session actually executes. Agreed by
  both sessions from the start.
- **`CONTRIBUTING.md` keeps a real, copy-pasteable list** — and is now explicitly
  **subordinate**: *"This block is a deliberate second copy… CLAUDE.md § Development
  Commands is authoritative… if the two ever disagree, this block is the bug."*
- **`docs/RELEASE.md`'s two blocks become references**, each keeping a note of what it
  used to hold and why that was wrong — so the deletion carries its own rationale
  instead of looking like an omission.
- **`docs/ROADMAP.md` keeps its dated analysis** but is corrected in place, with the
  pointer re-aimed at `CLAUDE.md` and the stale three-command description marked as
  what it said on 2026-08-06 rather than silently updated.

This session had argued for **two lists with distinct audiences** — same outcome for
`CONTRIBUTING.md`, but reached by treating the second copy as *co-equal*. That is the
part that was wrong. Two co-equal lists have no tiebreak, so a future divergence yields
two defensible readings and no procedure for resolving them; the subordination clause
costs one sentence and makes every future disagreement decidable in advance. Adopting
theirs.

A consequence worth stating, because it is the cost side of the trade: consolidating
raises the blast radius of anything false in the surviving copy. While the gate lived in
five transcriptions, a wrong claim in one was one voice among five and a reader had four
chances to notice the disagreement; afterwards it propagates by reference with nothing
left to contradict it. This is **not** an argument against consolidating — five copies
is how the clippy variant survived, and one wrong line in one place is cheaper to fix
than five — but it does mean the surviving copy earns a fact-check it never needed as
one voice among many. One was already found and fixed: see *Coordination state*.
## What is explicitly NOT in scope

`docs/RELEASE.md:33`'s block is **also missing `cargo fmt` entirely**. `codescout-ae`
flagged rather than fixed it, because it was outside what their operator approved. It
is unclaimed and does not collide with the consolidation.

## Coordination state

Two sessions authorised, both stood down, **nothing half-applied**. `codescout-ae` then
picked the work back up under the new frame and holds it now.

- `codescout-ae` (pid 803654) holds the design and the three uncommitted files.
- This session had two files edited **and gated green** — fmt, wide clippy, both test
  lanes, zero failures — then restored them to HEAD anyway, because the reframe made
  that diff the wrong diff.

**One correction to this document's own record:** the sentence above used to end
"binary back to **4** subcommands". That number is wrong wherever it appears. The real
count is **8**, and 8-or-nothing by construction — the `#[cfg(feature = "librarian")]`
sits on the whole `Commands::Artifact` variant (`src/main.rs:175`) and none of `Verb`'s
eight variants (`src/cli/artifact.rs:10-28`) is individually gated, so no build
configuration yields 4. The same wrong number had reached `CLAUDE.md`'s gate paragraph
under a *"Measured 2026-08-30"* stamp and is fixed at `334cf64b` (patch-id
`58e73e72c520cff6a9b89dbe5903982c4763e2ec`).

It came from counting a **truncated** help display — the hazard the IL-3 buffer warning
already states in general terms (*a capped buffer "can report 0 for something
present"*). A count taken off a trimmed view measures the view, not the thing. Note this
is not a stale measurement: a stale number was true once, which places the fault in the
tree. This one was never achievable, which places it in the reading.

**And the first fix was still under-specified — caught by `codescout-ae` on review.**
The help block prints **nine** lines under `Commands:`: the eight verbs plus clap's
auto-appended `help`. So a bare "8" attached to what `--help` shows is wrong-by-one
against the literal output, and the next person to re-measure by counting the block gets
9, concludes the correction is also wrong, and reverts it to 4. `CLAUDE.md` now states
the counting rule rather than the value alone.

> **A number in a measurement claim inherits the ambiguity of its counting rule.**
> 4-vs-8-vs-9 is not three measurement errors — it is one measurement under three
> defensible rules, and a bare integer selects none of them. This is the same defect as
> the withdrawn clippy ordinal one level down, and it was reintroduced *in the commit
> fixing it*, which is the useful part: knowing the class did not prevent the instance.

That is also the concrete case for `codescout-ae`'s strengthening of the blast-radius
argument. Consolidation does not merely raise the **cost** of a wrong number, it removes
the **redundancy that could have caught it** — five copies disagreeing is a signal, and
one copy is never in disagreement with anything. The remedy is not to keep copies but to
make the surviving one carry its **derivation**: `CLAUDE.md` now says *why* it is 8
(the cfg sits on the whole variant; no `Verb` variant is individually gated) so a reader
can re-check the claim instead of re-measuring it and getting a different number by a
different rule.

### One protocol amendment this produced

Our tiebreak was *"an uncommitted working-tree edit outranks a claim"*. It decides
**who writes a change both parties agree on**, and says nothing about whether the
change is still wanted. Here it would have awarded priority to the inferior design and
forced a reopen of two freshly-rewritten files.

> **Amendment: an uncommitted edit outranks a claim, UNLESS the frame changed after the
> edit — in which case it is stale work, not a stake. Priority must not survive the
> reason for the work.**

`codescout-ae`'s earlier tiebreak — **containment**, the wider scope subsumes the
narrower — is better than the edit rule and should be tried first: it is checkable, it
produces one edit instead of two, and it fails safe because the wider holder must name
the narrower's sites. This round is its own best evidence: the wider holder found a
site the narrower one never had.
## Resume — for whoever picks this up

1. Ask `codescout-ae` for the design before writing anything; they own it and asked to
   circulate first. Reach them at `uds:/run/user/1000/cc-socks/803654.sock` if
   `ListAgents` does not show them — it under-reports arbitrarily
   (`docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md`).
2. Contest or accept the two-lists proposal above.
3. Whatever lands, **name both test lanes by what they are, never by ordinal.** An
   ordinal is a positional reference — correct for one arrangement, silently wrong after
   any reorder, the same defect a bare SHA has. Two sentences in `CLAUDE.md` had already
   rotted this way and were fixed at `73066479`.
4. **Review the reorder diff by grepping, not by reading it.** A reordering diff shows
   what moved and hides every sentence that *described* the old order — which are
   exactly the sentences the reorder falsifies. Both stale sentences were found by
   grepping for old wording, not by diff review.
5. Gate in the new order. It leaves `target/debug/codescout` correct, so the next
   session does not inherit the `cli_artifact` clobber
   (`docs/issues/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md`).

## References

- `73066479` — the gate reorder in `CLAUDE.md`, with the by-construction argument and both caveats.
- `4c88e129` — the earlier `cargo test` → `cargo test --workspace` swap that widened some sites and not others.
- `d92f5f9c` — the shared-`target/` clobber this ordering exists to avoid.
- `docs/issues/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md`
