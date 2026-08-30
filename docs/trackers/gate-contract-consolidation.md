---
id: '27f6c88314fe6cee'
kind: tracker
status: active
title: Gate contract consolidation — the four transcriptions of one command list, and what replaces them
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

# Gate contract consolidation — the four transcriptions of one command list

**Deliberately owns no `PREFIX-N` namespace.** This is a resume/handoff document, not a
ledger — nothing here is citable as an entry and nothing allocates ids. See
`docs/TAXONOMY.md` § *Trackers that deliberately own no prefix*. Edit it directly.

**Status 2026-08-30:** design in progress by `codescout-ae`; **not started** in code.
Two sessions were independently authorised to widen these surfaces and both stood down
when the operator reframed the task from *widen* to *deduplicate*. Nothing is
half-applied — every file is at HEAD.

---

## Why this exists

The four-command gate is transcribed independently into **at least four places**. Each
is a separate copy of one contract, so each rots on its own, and **a reader of any one
of them sees a complete-looking list**. That is what makes the drift invisible rather
than merely present: nothing looks wrong at any single site.

Measured 2026-08-30, after `4c88e129` had already swapped `cargo test` →
`cargo test --workspace` at some of them:

| site | lean lane | clippy form | notes |
|---|---|---|---|
| `CLAUDE.md` § Development Commands | ✅ | wide (correct) | the copy an agent executes; reordered at `73066479` |
| `docs/RELEASE.md` `# 3. Full gate` (~:290) | ❌ **missing** | ❌ **fourth variant** | heading calls itself *Full gate* |
| `docs/RELEASE.md` `# 2. Build release binary and verify` (~:33) | ❌ **missing** | — | also missing `cargo fmt` entirely |
| `CONTRIBUTING.md` § Before Submitting a PR (~:110) | ❌ **missing** | both forms present | contributor-facing, meant to be copy-pasted |

## The clippy finding — a fourth variant, on the large-cohort path

`docs/RELEASE.md:292` reads `cargo clippy --all-targets -- -D warnings`. That is
neither the bare form nor the gate form.

Verified at the bytes: `Cargo.toml` declares `members = [".", "crates/codescout-embed"]`,
so the root is a real package and **default package selection is the root alone**.
`--all-targets` selects target *kinds*, not packages. So that line lints the root
package's test targets and **never reaches `crates/codescout-embed`** or the
`local-embed`-gated `local` module — precisely the hole CLAUDE.md's *"the gate, not
garnish"* paragraph exists to close. Sitting under a heading that calls itself the Full
gate, on the promotion path.

Also verified: `ci.yml`'s test matrix is **three** configs — `default` (flags `""`),
`local-embed`, and `no-features` (`--no-default-features`). The lean lane mirrors the
third exactly.

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

`codescout-ae` is designing this and explicitly asked to be contested. State of the
argument:

- **`CLAUDE.md` must stay inline.** It is the copy a Claude session actually executes.
- **`CONTRIBUTING.md` should probably stay a real list too**, and this is the contested
  part. An external contributor has no reason to read a file whose name is an internal
  agent contract, and may not have it checked out in a form they will open. Replacing
  their copy-pasteable block with a pointer trades drift for a worse external path.
- **`docs/RELEASE.md`'s two blocks become references.** Their audience already lives in
  this repo's conventions, and this is the file that has drifted twice.

**Proposed outcome: two lists, not one** — one for agents (`CLAUDE.md`), one for humans
(`CONTRIBUTING.md`), references everywhere else. The second copy earns its place by
having a distinct audience rather than being a transcription. **Not agreed**;
`codescout-ae` may argue for one.

## What is explicitly NOT in scope

`docs/RELEASE.md:33`'s block is **also missing `cargo fmt` entirely**. `codescout-ae`
flagged rather than fixed it, because it was outside what their operator approved. It
is unclaimed and does not collide with the consolidation.

## Coordination state

Two sessions authorised, both stood down, **nothing half-applied**.

- `codescout-ae` (pid 803654) holds the design and will circulate it before writing.
- This session had both files edited **and gated green** — fmt, wide clippy, both test
  lanes, zero failures, binary back to 4 subcommands — then restored them to HEAD
  anyway, because the reframe made that diff the wrong diff.

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
the narrower's sites.

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

