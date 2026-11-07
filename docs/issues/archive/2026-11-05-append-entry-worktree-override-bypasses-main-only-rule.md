---
id: 8b466b2c4502a6ea
kind: bug
status: fixed
title: append_entry accepts worktree ledger allocation despite main-only guide
tags:
- cluster/unclassified
- librarian
- worktree
closed: 2026-11-07
opened: 2026-11-05
owner: marius
severity: medium
unverified: is_worktree_shadow covers only worktree sessions (cp.main_root is Some). A main-checkout session naming a worktree shadow's id, a call with no current project, and a session in worktree A naming worktree B's shadow outside main_root still allocate on the shadow; the params-ledger branch was left alone. This residual has no tracking bug file yet. The gate does not cover the three heavy Python suites, server-stack, librarian code in the lean lane, or the local ONNX weight tests. The fix author did not fetch get_guide("tracker-conventions") or get_guide("librarian"). Which exact row the originally observed id 7728cf1b9c13a59b was (shadow or worktree-born) is not verifiable from the bug file.
---

# BUG: append_entry accepted allocation in a worktree ledger despite the main-only guide

## Class fit

Workspace-scoping/admission contradiction at the tool boundary. Unlike a proven namespace collision, this observation is a successful worktree-local ID allocation when the served guide says such allocations are refused.

## Symptom (Effect)

Observed2026-11-05 from active MAIN=/home/marius/work/claude/codescout: doc append_entry with id7728cf1b9c13a59b and workspace=/home/marius/.codex/worktrees/615e/codescout returned idLMC-4/section_writtentrue. docget reports that artifact's absolute path inside the linked worktree. Actual gitstatus for that path is modified inV and unchanged inMAIN. SourcecontainsnewLMC-4heading/highwater4. The auto-served guide states 'From the MAIN checkout only' and says worktree-session allocation is refused.

## Reproduction

Recorded actual request: actionappend_entry,id7728cf1b9c13a59b,workspaceV,id_prefixLMC,anchor_headingTemplate fornewentries,titleComplete native low workflow passes an exposed task at lower CLI cost. Do NOT replay on the real ledger; it already consumedLMC-4. Reproduce against a disposable main/worktree fixture instead.

## Root cause

Corrected 2026-11-07 (the earlier text below said Unknown): reproduced on a fixture before the fix, append_entry on a worktree shadow of a main ledger allocated HY-11 and wrote the section. The guard in the prose-ledger branch of append_entry only refused rows under main_root, and did not refuse a shadow row under the worktree root. Earlier text, kept as recorded: Unknown. The request arrived through an active MAIN session with a per-call worktree override and a catalog ID whose path is inV. Whether the guard checks caller workspace, target ledger root or the wrong artifact must be traced before fixing. No duplicate ID or lost entry was observed.

## Evidence

The same response auto-injected tracker-conventions section 'Entry ids — refusals: main checkout only, and unpushed ledgers'. Follow-up docget returned abs_path=/home/marius/.codex/worktrees/615e/codescout/docs/trackers/low-effort-challenger-mechanisms.md, entry_high_water_LMC4 and fullnewbody. Separate gitstatus calls showedVmodified andMAINunchanged.

## Containment

Retain the allocated entry and highwater; do not renumber/delete it or allocate another namespace entry as a probe. Continue the research result recording; no infrastructure fix or push is authorized merely by this finding.

## Fix

The change lives in two files:

- src/librarian/tools/worktree.rs adds is_worktree_shadow(cat, cp, row). It is true when cp.main_root is Some, the row is under the worktree root, and the row has an outgoing worktree_of link (LINEAGE_REL) or a worktree_fork event.
- src/librarian/tools/append_entry.rs: the prose-ledger branch now refuses when is_main_checkout_artifact(...) || is_worktree_shadow(...). The check runs before resolve_write_target, so a refused call still writes nothing. The refusal message is unchanged. A worktree-born ledger with no lineage stays allowed.

Plan deviation: Unknown (none was given).

Commits, as developed on branch fix/open-issues-batch-20261107-r2. All of them are now on experiments at 143c1941f, with the same SHAs, because experiments reached that tip by fast-forward from the branch:

- 52440e6b965571f61fb8db057ad21ce7ea94aa3d (branch fix/open-issues-batch-20261107-r2) / patch-id 12e7079e3e6df054fcf1acdfdc0ccf1ae2477970 (the fix)
- 4c38aec34704c4d7f62d9fc8e57251c1fe304fbc (branch fix/open-issues-batch-20261107-r2) / patch-id 7e41f1a94d9c02407ed3419ae4094f34a91e61b4 (one test per half of the disjunction, control comment reworded)
- 0b71cad1dba27ad86109ebe21d9b39658b306e72 (branch fix/open-issues-batch-20261107-r2) / patch-id dcb47a602ba0d9ff34551c5138af1d845a71f163 (doc comment: worktree sessions only)

### Verification

./scripts/gate.sh --check was run on fix/open-issues-batch-20261107-r2 at its tip af3e2a9ec on 2026-11-07: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0 (lean nextest 6227 tests; default lane green). The gate does NOT cover: the three heavy Python suites, server-stack, librarian code in the lean lane, the local ONNX weight tests.

Not covered by the fix: only worktree sessions (cp.main_root is Some) are checked. The params-ledger branch was left alone.

## Tests added

New tests (in the librarian tools tests; exact file and line: Unknown, not given to this record):

- prose_allocation_is_refused_on_a_worktree_shadow_of_a_main_ledger (red before the fix)
- prose_allocation_is_allowed_on_a_worktree_born_ledger (control: a worktree-born ledger with no lineage stays allowed)
- two tests that pin the lineage-link half and the fork-event half of the shadow check alone; mutants removing either half went red.

The fix was reviewed adversarially by an Opus reviewer who ran the new test red against the reverted production change (or by mutants) and approved.

## Fix provenance

- **SHA:** `52440e6b965571f61fb8db057ad21ce7ea94aa3d` (`experiments`)
- **patch-id:** `12e7079e3e6df054fcf1acdfdc0ccf1ae2477970`

## Resume

The fix landed on experiments (tip 143c1941f, reached by fast-forward from fix/open-issues-batch-20261107-r2, so the SHAs are unchanged), ./scripts/gate.sh --check on that tree was green (FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0), and this file was archived to docs/issues/archive/. Nothing is waiting on a landing. Remaining gaps are listed in the unverified field.
