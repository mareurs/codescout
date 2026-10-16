---
kind: bug
status: zombie
tags:
- cluster/unclassified
closed: null
opened: 2026-10-06
last_observed: 2026-10-06
owner: marius
related: []
severity: medium
unverified: Not reproduced by the filing session; the original instance rests on a subagent report plus the controller verifying the aftermath only.
---

# BUG: edit_file's heading grammar was reported writing to the main checkout when pinned to a worktree via `workspace`, on librarian-managed markdown

## Summary
A subagent that called `edit_file(workspace=<git worktree>, heading=..., action=...)` on three librarian-managed markdown files was reported to have had the edits land UNSTAGED in the MAIN checkout, not the worktree; the response's `wrote_to` named the main checkout. If real, it defeats `workspace` isolation for every subagent/worktree flow and writes into a checkout other live sessions are using. The filing session could not reproduce it (see Reproduction), so the report is recorded as a lead, not a confirmed defect.

## Symptom (Effect)
Reported, not witnessed. A subagent pinned `workspace="/home/marius/work/claude/codescout.worktrees/publish-hold"` on `edit_file` for `docs/RELEASE.md`, `docs/conventions/shared-checkout-commit-sequence.md` and `docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md`. Each response's `wrote_to` named `/home/marius/work/claude/codescout` (the main checkout). The main working tree carried the edits unstaged for a few minutes, until the subagent copied the files into the worktree and ran `git checkout --` on them in main. The controller verified the aftermath only: main showed no diff on those three files afterwards. The misrouted write itself was not witnessed.

On ordinary files (shell scripts, tests, a spec without a librarian id) the same `workspace` pin wrote into the worktree as intended (verified by `git status` in both trees, per the reporter).

## Reproduction
Attempted twice on 2026-10-06 at HEAD `4766c07f` (main checkout, branch `experiments`; worktree `/home/marius/work/claude/codescout.worktrees/publish-hold`, branch `feat/publish-hold`, HEAD `cd792f86`), via the live MCP:

1. `edit_file(action="edit", heading="### 6. If a commit captures another session's file, stop", old_string="let the other session decide.", new_string="let the other session decide. ZZPROBE", path="docs/conventions/shared-checkout-commit-sequence.md", workspace=<worktree>)` on a file carrying a librarian `id:` (`fa4a13f40ea82465`). Response: `{"status":"ok","rel_path":"docs/conventions/shared-checkout-commit-sequence.md"}` (no `wrote_to`). `git -C <worktree> diff --stat` showed that file, 1 insertion / 1 deletion. `git diff --stat -- docs/conventions` in main showed nothing. The write went to the worktree, as intended.
2. The same shape on `docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md` (`id: d9d291b44775e50d`), heading `## Tests added`. Same result: worktree diff only, main clean.

Both probes were reversed with the inverse `edit_file` call (same pin); afterwards both trees showed no diff on the probed files. `docs/RELEASE.md` was not probed: it carries no frontmatter `id:` (it is not librarian-managed).

NOT reproduced. Best lead: see Root cause (the response the reporter saw carried `wrote_to`, which the server only attaches to an UNPINNED call).

## Environment
Linux 7.2.8-zen1-2-zen; codescout live MCP binary on this checkout; repo with linked worktrees (`codescout.worktrees/*`), so `annotate_write_root` is active. Branch `experiments` in main.

## Root cause

Not reproduced, so there is no root cause to name. What is now measured rather than read:

- A server-level test (`a_pinned_heading_edit_on_a_stamped_file_writes_the_pinned_workspace_not_main`, `src/server.rs`) drives `call_tool_inner` end to end: pin extraction, access gate, `EditFile::call_content`, post-processing. Main is seeded with a linked worktree so an unpinned write annotates. A pinned heading-grammar edit on a stamped (`id:`) file, single and batch grammar, relative and absolute-in-worktree path, changes the pinned workspace's file, leaves main's byte-identical, and carries no `wrote_to`. The unpinned control does carry `wrote_to` naming main and writes main.
- Mutation: making `extract_workspace_override` ignore the pin reds that test (every row: the unpinned call meets the worktree write guard). So the test depends on the pin being honoured.
- `wrote_to` is attached only when `ctx.workspace_override.is_none()` (`src/tools/core/types.rs`, `annotate_root` in `call_content`). A response that carries it therefore came from a call the server saw WITHOUT a pin. That is still the only reading consistent with the report, and it points at the client side: the pin was dropped between the subagent and the server.
- `maybe_replay_ack` (`src/tools/core/write_ack.rs`) was the earlier candidate for dropping the pin. Read, not measured: an `@ack_*` handle is minted only for a path OUTSIDE the pinned root, and the replay returns the stored input, whose `path` is that same absolute path. An unpinned replay therefore writes the file the minting call named, not a different one, so it cannot turn a pinned relative-path edit into a main write.
- The companion `principal-stamp` hook rewrites tool input with `updatedInput` but spreads the existing input first (`{ ...(input.tool_input || {}) }`), so it does not drop `workspace`.

The original calls' argument JSON was never retained, so "was `workspace` present?" cannot be answered from this side.

## Evidence
- Probe transcript: Reproduction above (both trees' `git diff --stat` output, 2026-10-06).
- Source reading cited under Root cause. Nothing measured on the original incident; the original response bodies were not retained by the filing session.

## Hypotheses tried

1. **Hypothesis:** the heading grammar on a librarian-managed file resolves its path against the main checkout, ignoring `workspace`. **Test:** pinned `edit_file` on two managed files (see Reproduction). **Verdict:** rejected for these two files at HEAD `4766c07f`; **Evidence:** Reproduction probes 1 and 2.
2. **Hypothesis:** the original calls arrived unpinned (the pin was dropped before the server, or via an `@ack_*` replay). **Test:** none yet. **Verdict:** deferred; it is the only reading consistent with `wrote_to` appearing (Root cause, second bullet).
3. **Hypothesis (2026-10-16):** the pin is lost somewhere between `call_tool_inner` and the heading-grammar write, for a stamped file. **Test:** the server-level test named under Root cause, four call shapes, plus the unpinned control. **Verdict:** rejected; the write lands in the pinned workspace and main is untouched. **Evidence:** the test passes, and fails under the `extract_workspace_override` mutation.
4. **Hypothesis (2026-10-16):** an `@ack_*` replay without the pin misroutes a pinned write. **Test:** read `maybe_replay_ack` and `resolve_write_or_capture`. **Verdict:** rejected by reading; the replay writes the absolute path the minting call named. **Evidence:** `src/tools/core/write_ack.rs`, `maybe_replay_ack`, `resolve_write_or_capture`. Not run.

## Fix

None. Nothing reproduced, so nothing was changed in `src/` for this issue. Only a regression test was added, in commit `feb69976` (patch-id `09a68be4d2a7c02c5f9f01228ac0c02f34351428`) on the guards worktree branch `worktree-agent-a4ecb4e88086aea56`.

## Tests added

`a_pinned_heading_edit_on_a_stamped_file_writes_the_pinned_workspace_not_main` in `src/server.rs` (tests module, after `call_tool_inner_honors_workspace_override_for_security_config`). It went green on first run, which is the finding: the bug does not reproduce. It stays as a guard, and it was shown to discriminate by mutation (see Root cause). It covers the dispatch path, which the sketch in the earlier version of this section (a test beside `librarian_guard_fires_on_the_markdown_grammar_write_route`) would not have: that route calls `EditFile::call` directly and never reaches `extract_workspace_override`.

## Workarounds
After any pinned `edit_file` on a managed markdown file, check `git status` in BOTH trees, and treat a response carrying `wrote_to` on a call you pinned as a misroute. Alternatively edit managed files through `doc(action="update", patch={body_edits: [...]})`, which is the sanctioned surface for them.

## Resume

Status is `zombie`: no longer reproducible, root cause not confirmed. Re-open (set `open`, clear `last_observed`) if any of these appear:

- A `wrote_to` on a response to a call whose argument JSON is known to have carried `workspace`. That would mean the server saw no pin, so diff `extract_workspace_override` input handling for that shape (a non-string `workspace`, a transport that renames the key).
- A pinned `edit_file` that changes a file in a tree other than the pinned one, with the call JSON retained. Then replay that exact shape against a throwaway linked worktree.

When it recurs, capture the exact `edit_file` argument JSON before anything is reverted.

## Why this is `cluster/unclassified`
The class this would belong to, `IC-15` (`accepted-parameter-silently-dropped`), asserts a parameter is accepted and then dropped. That is the REPORTED symptom, but the filing session could not reproduce it and the code read shows the markdown route honouring the pin, so asserting membership would add an unverified instance to a class whose counts promotion reads. Re-classify once the mechanism is known (a dropped pin on a specific input shape would be an `IC-15` member; a client-side omission would not be a codescout defect at all).

## References
- `src/tools/edit_file/mod.rs` (`EditFile::call`), `src/tools/markdown/edit_markdown.rs` (`edit`), `src/tools/core/write_ack.rs` (`maybe_replay_ack`, `resolve_write_or_capture`), `src/tools/core/types.rs` (`annotate_write_root`, the `annotate_root` gate), `src/server.rs` (`extract_workspace_override`).
- Merge of the branch the subagent was working on: `4766c07f` (`feat/publish-hold`, the publish hold).
- `docs/conventions/shared-checkout-commit-sequence.md` (the shared-checkout rules the misroute violated by touching main's tree).
