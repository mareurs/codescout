---
kind: bug
status: open
tags:
- cluster/unclassified
closed: null
opened: 2026-10-06
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
Unknown - see Hypotheses tried. What reading the code establishes:

- The markdown route threads the pin correctly. `EditFile::call` (`src/tools/edit_file/mod.rs`, `call`, the `heading_grammar` branch) hands `ctx` to `crate::tools::markdown::edit`, which resolves its target through `resolve_write_or_capture(ctx, ...)` (`src/tools/markdown/edit_markdown.rs`, `edit`; `src/tools/core/write_ack.rs`, `resolve_write_or_capture`), and that resolves against `ctx.agent.require_project_root_for(ctx.workspace_override.as_deref())`. The server sets `ctx.workspace_override` from `input["workspace"]` for every tool (`src/server.rs`, `extract_workspace_override`, and `ctx.workspace_override = workspace_override` in the call path).
- `wrote_to` is only attached to UNPINNED writes: `annotate_root = ctx.workspace_override.is_none() && self.is_write(&input) && ...` (`src/tools/core/types.rs`, in `call_content`, near the `annotate_write_root` call). A response that carries `wrote_to` therefore means that call reached the server with NO `workspace` pin. My two pinned probes returned no `wrote_to`, consistent with this.
- So the reported `wrote_to` naming main is evidence the pin was LOST BEFORE `ctx.workspace_override` was set (the call left the subagent without it, or a layer between client and server dropped it), not evidence that the managed-artifact route drops it. Inferred from the code above, not measured: no call carrying the pin was ever observed arriving unpinned.
- Where the pin could be lost is not established. Candidate: `maybe_replay_ack` (`src/tools/core/write_ack.rs`) returns the STORED input of an earlier refused call when `path` is an `@ack_*` handle, and replay does not obviously re-merge a `workspace` given on the replaying call. Not tested.

## Evidence
- Probe transcript: Reproduction above (both trees' `git diff --stat` output, 2026-10-06).
- Source reading cited under Root cause. Nothing measured on the original incident; the original response bodies were not retained by the filing session.

## Hypotheses tried
1. **Hypothesis:** the heading grammar on a librarian-managed file resolves its path against the main checkout, ignoring `workspace`. **Test:** pinned `edit_file` on two managed files (see Reproduction). **Verdict:** rejected for these two files at HEAD `4766c07f`; **Evidence:** Reproduction probes 1 and 2.
2. **Hypothesis:** the original calls arrived unpinned (the pin was dropped before the server, or via an `@ack_*` replay). **Test:** none yet. **Verdict:** deferred; it is the only reading consistent with `wrote_to` appearing (Root cause, second bullet).

## Fix
Not applied.

## Tests added
N/A - nothing reproduced and nothing changed. If a cause is found, the regression test belongs beside `librarian_guard_fires_on_the_markdown_grammar_write_route` in `src/tools/edit_file/tests.rs`: a pinned heading-grammar write on a stamped file in a linked worktree must change the worktree's file and leave main's byte-identical.

## Workarounds
After any pinned `edit_file` on a managed markdown file, check `git status` in BOTH trees, and treat a response carrying `wrote_to` on a call you pinned as a misroute. Alternatively edit managed files through `doc(action="update", patch={body_edits: [...]})`, which is the sanctioned surface for them.

## Resume
Ask the subagent's session for the exact `edit_file` argument JSON of the three misrouted calls (was `workspace` present? was `path` an `@ack_*` handle?). Then replay one such call shape against a throwaway linked worktree of this repo. If `workspace` was present and the call still came back with `wrote_to`, diff `src/server.rs` `extract_workspace_override` input handling for that shape; if `path` was an `@ack_*` handle, test `maybe_replay_ack` in `src/tools/core/write_ack.rs` with a pinned replaying call. If neither is found, set status to `zombie` with `last_observed: 2026-10-06`.

## Why this is `cluster/unclassified`
The class this would belong to, `IC-15` (`accepted-parameter-silently-dropped`), asserts a parameter is accepted and then dropped. That is the REPORTED symptom, but the filing session could not reproduce it and the code read shows the markdown route honouring the pin, so asserting membership would add an unverified instance to a class whose counts promotion reads. Re-classify once the mechanism is known (a dropped pin on a specific input shape would be an `IC-15` member; a client-side omission would not be a codescout defect at all).

## References
- `src/tools/edit_file/mod.rs` (`EditFile::call`), `src/tools/markdown/edit_markdown.rs` (`edit`), `src/tools/core/write_ack.rs` (`maybe_replay_ack`, `resolve_write_or_capture`), `src/tools/core/types.rs` (`annotate_write_root`, the `annotate_root` gate), `src/server.rs` (`extract_workspace_override`).
- Merge of the branch the subagent was working on: `4766c07f` (`feat/publish-hold`, the publish hold).
- `docs/conventions/shared-checkout-commit-sequence.md` (the shared-checkout rules the misroute violated by touching main's tree).
