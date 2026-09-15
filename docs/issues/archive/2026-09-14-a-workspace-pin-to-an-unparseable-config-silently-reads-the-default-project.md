---
kind: bug
status: fixed
tags:
- cluster/accepted-parameter-silently-dropped
closed: 2026-09-15
opened: 2026-09-14
owner: marius
related: []
severity: medium
---

# BUG: a `workspace=` pin at a project whose config fails to parse silently reads the DEFAULT project instead

## Summary

`workspace=<abs path>` is documented as "resolve this call against this workspace". When the
pinned project's `.codescout/project.toml` fails to parse, a **read** does not error and does not
return empty — it returns the **default project's** data, with nothing in the response naming the
substitution. The caller asked about project A and is answered about project B.

The write path on the identical config fails **closed** with an explicit error, so the two halves
of the same pin disagree about what an unparseable config means.

## Symptom (Effect)

Two project roots, structurally identical — same layout, same single marker memory on disk. The
**only** difference is that one `project.toml` omits the `[project]` table.

```
memory(action="list", workspace=<probe4, valid config>)
  1 topics
    probe4-unique-marker                 <- correct

memory(action="list", workspace=<probe3, malformed config>)
  23 topics
    architecture
    cargo-test-lib-skips-integration
    ...                                  <- CODESCOUT's memories, not probe3's
```

probe3's own `probe3-unique-marker` does not appear anywhere in the 23. There is no error, no
warning, and no `scope` note saying which project answered.

A write through the same malformed pin behaves correctly:

```
memory(action="write", topic=..., workspace=<malformed>)
  Error: write gate: missing field `project`
```

## Reproduction

`HEAD = 9045c56a88899d46c16387f817ca440d110e0f72` (branch `experiments`), live MCP.

1. `mkdir -p probeA/.codescout/memories`; write a valid `project.toml` (a `[project]` table with
   `name` and `languages`); drop one uniquely-named `.md` in `.codescout/memories/`.
2. `probeB` identical, except `project.toml` contains only a `[security]` table.
3. `memory(action="list", workspace=<probeA>)` → `1 topics`, the marker.
4. `memory(action="list", workspace=<probeB>)` → the **active** project's topics.

Step 3 is the control; without it, step 4 is indistinguishable from "the memory dir was unreadable".

**Each probe must be a FRESH root.** A root already resident carries a cached config, and
breaking its `project.toml` afterwards changes nothing — see
`docs/issues/archive/2026-09-14-an-out-of-band-project-toml-edit-never-invalidates-the-cached-config.md`,
which will mask this one if you reuse a root.

## Environment

Linux, `experiments`, live MCP (stdio), release binary built 2026-09-14 11:49:52. Probe roots
under the session scratchpad, each its own git repo.

## Root cause

**Traced 2026-09-15 to `resolve_memory_dirs` (`src/tools/memory/mod.rs:456`), and § *Resume*'s
fork is answered: the fallback is in the MEMORY TOOL's own lookup, not in pin resolution.**

Two defects compound, and either alone would be survivable:

1. **The residency error is discarded.** `let _ = ctx.agent.ensure_resident(root, None).await`
   — `ensure_resident` calls `ProjectConfig::load_or_default`, which is exactly where
   `missing field 'project'` is produced, so this line throws away the only value that names
   the cause. The pinned workspace is then never inserted into `inner.workspaces`.
2. **One `else` branch is reached by two conditions with opposite correct answers.** With the
   pin unresolved, `inner.workspaces.get(&key)` returns `None`, which falls to a branch whose
   own comment reads *"No workspace — fall back to the active project's memory dir"*. That is
   right for **"no pin was requested"** and wrong for **"the pin did not resolve"**. The
   comment names which of the two the author had in mind; nothing distinguishes them at the
   `if let Some(ws)`.

**The write half refuses because it resolves through a function that gets both right.**
`Agent::with_project_at` (`src/agent/mod.rs`) propagates residency with `?` **and** turns a
lookup miss into `pinned workspace not resident: …` rather than a fallback. So this was never a
disagreement about policy between the two halves — it is one code path having the check and the
other not.

**The earlier lead in this section was wrong in both particulars, and is superseded rather than
deleted because the way it was wrong is the reusable part.** It read: *"the resolution happening
before or around `Agent::ensure_resident` with a fallback to `default_workspace_root` on load
failure."* `ensure_resident` does **not** fall back — it correctly returns `Err`; and nothing
falls back to `default_workspace_root` — the fallback is to `active_project()`, one layer up, in
the caller. The lead named where the failure ORIGINATES; the defect is where it is SWALLOWED.
Same shape as `bug-fix-session-log:F-157`, one record later and in the same subsystem.

**Unverified sibling, recorded as a lead and not as a finding:**
`Agent::call_edges_project_id_for` (`src/agent/mod.rs:1589`) carries the identical
`let _ = … ensure_resident` discard and falls back to `ROOT_PROJECT_ID`. Its consequence is a
call-edge cache namespace rather than a wrong answer to the caller, it returns `String` so it
cannot propagate without a signature change, and **it has not been probed** — the fallback may
be unreachable in practice because a pin that cannot load will fail elsewhere first. Named so
the next reader does not re-derive it; not filed, because filing it would assert reachability
nobody has measured.

*mechanism read at the bytes 2026-09-15; the four-step probe re-run first and reproduced in
both directions — read returned the default project's 23 topics with the pinned marker absent,
write refused with `write gate: missing field 'project'`.*
## Hypotheses tried

1. **Hypothesis:** probe3 returned the default project's topics because its memories directory
   was unreadable or its marker file malformed, not because of the config.
   **Test:** probe4, byte-identical in structure and marker, differing only in `project.toml`
   validity.
   **Verdict:** rejected — probe4 returned its own single topic.

## Fix

**IMPLEMENTED 2026-09-15 (`c1c8763a`).** `resolve_memory_dirs` now does what
`Agent::with_project_at` already did: propagate the residency error with `?`, and turn a
lookup miss into a refusal instead of a fallback. The two halves of `workspace=` now agree.

The refusal names the pin, the cause, and two actions the caller can actually perform:

```
workspace pin /…/probeB could not be resolved: missing field `project`
hint: Fix that project's .codescout/project.toml -- a [project] table with a `name` is
      required -- or drop the workspace= argument to read the active project instead.
```

This was the option § *Fix* called smaller — match the write path — and tracing the mechanism
showed why it is also the correct one rather than merely the cheaper one: the two halves were
never enforcing different *policies*. One resolved through a function carrying both checks and
the other did not.
## Tests added

`a_pin_at_an_unparseable_config_refuses_instead_of_answering_about_the_default`
(`src/tools/memory/tests.rs`), added 2026-09-15 with `c1c8763a`.

It follows this section's own prior instruction, which was right: **assert on IDENTITY, never a
count.** Both projects have memories, so `len() > 0`, `!is_empty()` and every count assertion
pass in the *failing* world; only the marker topic separates "answered about the pin" from
"answered about the default". The panic path prints what actually came back, so a regression
reports WHICH project answered rather than merely that something did.

**The valid-pin control is load-bearing and is now mutation-proven.** Without it the refusal is
indistinguishable from "a pin always refuses" — a test that stays green if `workspace=` were
broken outright. Disabling the residency propagation in an isolated worktree killed the test at
`tests.rs:2773` on exactly that assertion (`KILLED`, rc=101, 1 test ran).

**A limit, stated because the KILLED above does not cover it.** The defect assertion is guarded
**twice** — residency is propagated, *and* a non-resident lookup is refused — so with either one
disabled the bad pin still refuses and the defect assertion still passes. No single-literal
mutation makes it answer, which means that assertion is **not independently mutation-verified**;
the mutation above reached the control, not it. The redundancy is deliberate on a path that
decides which project a caller is told about, and deleting one guard to make the other
independently killable would optimise the mutation score against the property. What covers the
defect assertion's subject instead is the live probe below.

**Verified end to end against the live MCP** (2026-09-15, after `cargo rb` + `/mcp`, both probe
roots fresh to the new process): the broken pin refuses as quoted in § *Fix*, and the valid-pin
control still returns its own single marker. That covers what `is_err()` structurally cannot —
whether the refusal a *caller* receives names the pin, the cause, and something they can do.

Gate green on both lanes, read from a file rather than a buffer (`bug-fix-session-log:F-158`):
`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, 0 failed. The test runs in the lean lane as well as the
default one, so it is not vacuous under `--no-default-features`.
## Workarounds

Validate `project.toml` before relying on a pin. A pinned **write** to the same root surfaces the
parse error, so it can be used as a probe for whether a pinned read can be trusted.

**Obsolete as of `c1c8763a`** — the pinned read now surfaces the same error itself, so the
write-as-probe step is no longer needed. Kept because it is the correct advice for any build
predating that commit.
## Resume

**Nothing on this defect — fixed, verified live, and closed.**

One lead is recorded in § *Root cause* and deliberately **not** filed:
`Agent::call_edges_project_id_for` (`src/agent/mod.rs:1589`) carries the identical
`let _ = … ensure_resident` discard and falls back to `ROOT_PROJECT_ID`. Nobody has probed
whether that fallback is reachable — a pin that cannot load may fail elsewhere first — and its
consequence is a call-edge cache namespace rather than a wrong answer to a caller. Filing it
would assert a reachability nobody has measured; re-deriving it from scratch is the waste this
note exists to prevent.
## References

- `src/agent/mod.rs:668` — `ensure_resident`
- `src/config/project.rs:498` — `load_with_global_base`
- `docs/issues/archive/2026-09-14-an-out-of-band-project-toml-edit-never-invalidates-the-cached-config.md`
  — found in the same probe run; masks this one if a root is reused

## Fix provenance

- **SHA:** `c1c8763a` (experiments) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `4d168c62cb789a3c0840395592dacf127e022027` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id.
