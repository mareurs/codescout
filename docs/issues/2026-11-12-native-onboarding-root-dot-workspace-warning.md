---
id: b02254860a3809d9
kind: bug
status: fixed
title: Frozen native onboarding writes a workspace root entry later rejected by its index loader
tags:
- cluster/unclassified
- onboarding
- effort-steering
topic: effort-steering
closed: 2026-11-12
opened: 2026-11-12
owner: marius
severity: low
unverified: 'Fixed on main in 1ca1495de. After the 2026-11-12 13:35 EET restart (binary built 13:35:32, after the fix), this repo''s diagnostic and debug logs hold no `is not a member ... root=.` line newer than 10:16Z, while earlier builds logged one on each load (17 lines from 07:44Z to 10:16Z, id=codescout root=.). Not shown: that a load ran merge_declared_projects on this server. The home activation after the restart returned a workspace list, which suggests it did, but I did not observe the call. My positive control (a second entry root="nope" in a scratch workspace.toml, activated on the server) logged nothing, so this server was not shown able to emit the warning at all. The frozen 2026-11-04 binary still emits it, by decision. Not shown: member/tool/LSP effects on the benchmark checkout.'
---

# BUG: Native onboarding's root workspace entry is rejected by its own index loader

## Summary

On fresh Arrow Rust, the frozen native onboarding tool writes .codescout/workspace.toml with a project id testbed and root dot. A later invocation of the same frozen binary prints that this entry is not a member because its root is not a relative path below the workspace root. The index still completes for the active root. Record the producer/consumer disagreement without claiming that language capabilities or member retrieval are broken by this warning.

## Class fit

Temporarily unclassified frozen-runtime configuration disagreement. A generated configuration row conflicts with the same binary's later membership validation. Unlike the separate empty-member-store issue, this observation is a warning about a generated root entry, not an executed empty member query.

## Symptom (Effect)

The index CLI warns id=testbed root=dot. Root verification independently reports590expected/stored files,5disk/stored memories,zero missing/orphan/nonvector entries and Git up to date. Full native readiness is not inferred from those root-store predicates.

## Reproduction

Canonical Arrow commit91199c7190f1c5b931f1d0b6bcc6ccdd86a4c5c0, source-only common preparation. Execute onboarding once with frozen Codescout binary SHAcdc53728d28a659b5a697048e6e5b719149693df2cf57cb8554ee0751ebf51bb at /testbed; inspect its generated workspace.toml. Later run that binary index -p /testbed. The retained stderr contains: workspace.toml project entry is not a member; root is not a relative path below the workspace root; id=testbed root=dot.

## Environment

Scratch /home/marius/.cache/codescout-resident/codescout/native-feedback-preparation-20261112/apache__arrow-rs-7783/common-preparation-v1. onboarding/calls.json contains the actual successful tool call; index-coderank-v2/index.stderr and generated project/.codescout/workspace.toml preserve the observation. Exact tool/language/LSP/member canaries remain pending.

## Root cause

Verified on current main 2026-11-12, and it corrects the "not reproduced on current main" note above. The disagreement is not specific to the frozen binary and not a defect of onboarding's writer.

- Onboarding writes every discovered project back into `workspace.toml` as a `[[project]]` snapshot, the workspace root included (`root = "."`). This repo's own `.codescout/workspace.toml` has `id = "codescout"`, `root = "."`.
- `load_declared_projects` (`src/agent/mod.rs`) passes every entry to `merge_declared_projects` (`src/workspace.rs`). That function called `declared_relative_root(&entry.root)` first. `declared_relative_root` returns `None` for `.` by design (it answers "a directory BELOW the root"), so the entry took the skip branch and logged `workspace.toml [[project]] entry is not a member: its root is not a relative path below the workspace root`. The branch for "the walk already found it, the expected case for an onboarding snapshot" came after that check and was never reached for the root.
- The same warning is in seven of this repo's own current diagnostic logs (`id=codescout root=.`, 2026-11-11 and 2026-11-12). The root IS a member, because the walk supplies it, so the warning was a false alarm on every load of an onboarded project.

## Evidence

The generated first project row is id testbed, root dot, languages bash/markdown/python/rust. The subsequent same-binary CLI logs the rejection and still indexes root. Both native and embedding owned resource cleanups complete.

## Hypotheses tried

No main binary rebuild or configuration repair. Known issue9be2636148879f0f covers a distinct root-only/member-scope preparation gap and stays open; neither is closed by the positive root index result.

## Fix

Fixed on main in `1ca1495de`: `merge_declared_projects` skips a declared entry that names the workspace root (`names_the_workspace_root`: non-empty, relative, only `.` components) silently when the walk found a root project, before `declared_relative_root` runs. A `.` entry for a root the walk did NOT find is still skipped and still logged.

Not changed: the frozen 2026-11-04 native benchmark binary. It still emits the warning, which is what the 2026-11-12 observation recorded, and it stays unchanged on purpose. The warning did not stop the root index from completing.

## Tests added

- `workspace::tests::a_declared_entry_for_the_discovered_workspace_root_is_not_reported_as_a_mistake`: red first, with the production warning text; green after the fix.
- `workspace::tests::a_declared_workspace_root_the_walk_did_not_find_is_still_reported`: control; it passed before the fix, so the log capture is not silent.
- `workspace::tests::only_dot_components_name_the_workspace_root`: the helper's edges (`""` and `"/"` are not the root).

The full gate on this tree: clippy, LEAN (9674 passed) and DEFAULT (9771 passed) exit 0. The format lane was red on two files of other sessions (`src/librarian/identity/gate.rs`, and later `src/lsp/client.rs`), not on this change; one line wrap in the new helper was fixed by hand afterwards, and `rustfmt --check src/workspace.rs` is clean.

## Fix provenance

- **SHA:** `1ca1495de` (`experiments`)
- **patch-id:** `188988541647f8a9b4b8cd774af8694b5ccef6f6`

## Workarounds

Retain the caveat and keep readiness/admission unclaimed until real canaries, including member-scoped retrieval, pass.

## Resume

Fixed on main in `1ca1495de`. The frozen benchmark binary is unchanged and still warns. Left: a live check on a rebuilt server, where the diagnostic log of a session on an onboarded project should stop carrying `is not a member ... root=.`. Member-scoped retrieval and the other canaries named above are a separate question and are not answered by this fix. Do not infer readiness or an efficacy effect from it.
