---
id: '60f0981dede665eb'
kind: bug
status: open
title: 'BUG: four unregistered `impl Tool` blocks in the memory module read as the live tool, and a bug investigation stated a false root cause from them'
tags:
- memory
- dead-code
- tools
- misleading-source
closed: null
opened: 2026-08-27
owner: marius
related:
- docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md
severity: low
unverified: No sweep was done for other conclusions mis-read off these structs; one instance is confirmed, the population is unknown. Whether the ~18 tests would still pass re-pointed at the live `Memory` tool is also untested, and is the main reason to prefer option 2.
---

## Summary

`src/tools/memory/mod.rs` defines **four `impl Tool` blocks that nothing registers** —
`WriteMemory`, `ReadMemory`, `ListMemories`, `DeleteMemory`. `src/server.rs` registers
`Arc::new(Memory)`, the unified tool, and only that. The four legacy structs exist solely
to be exercised by their own tests.

They are not inert, though, because they are *indistinguishable from the live tool when
read*. They carry the same names as the tool actions, live in the same file, above the
real implementation, and their `call` bodies are plausible. A reader tracing "what does
`memory(action="list")` do?" lands on `ListMemories::call` and stops.

That has already happened once, in a bug investigation, and the wrong conclusion was
written down as established fact.

## Symptom

`docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md`
carries a section headed **"What IS established"** whose first claim is:

> **`list` and `read` ignore `project_id` entirely.** Their handlers
> (`src/tools/memory/mod.rs`, the `list` arm and the `read` arm) call
> `agent.with_project_at(ctx.workspace_override, …)` and never read the parameter.

Every sentence of that is true **of the dead structs**, and false of the live tool. The
live `Memory::call` routes each non-private read through `resolve_memory_dir`, which reads
`project_id`, accepts `project` as an alias, and validates the id against
`Workspace::has_project` before use.

The file refutes itself two sections later, under **"Measured 2026-08-26"**:

```
memory(action="list", project_id="codescout-embed")  → 5 topics
```

which cannot happen if `project_id` is ignored. The measurement was right. The code
reading was done on a surface nothing serves, and the contradiction sat unresolved in an
"established" section because nothing forces a claim about code and a claim about
behaviour to be reconciled.

## Reproduction

```
references(symbol="ListMemories", path="src/tools/memory/mod.rs")
→ 9 references in 2 files
    src/tools/memory/tests.rs (7)
    src/tools/memory/mod.rs   (2)   ← the struct and its own impl
```

Zero non-test call sites. Same for the other three.

```
grep(pattern="Arc::new\\(Memory\\)", glob="src/**/*.rs")
→ src/server.rs:342
```

## Root cause

A tool consolidation that introduced the unified `Memory` tool left the four
per-action tools in place rather than deleting them, and their tests kept passing —
so nothing reported them as dead. Rust does not warn: they are `pub(crate)` and
constructed by the tests in the same crate, which is a genuine use as far as the
compiler is concerned.

The tests are what make this stable. Dead code with no tests eventually trips a
lint or a reviewer; dead code with 18 passing tests looks maintained.

## Fix

Two options, and the choice is about what the tests are worth:

1. **Delete all four**, and with them the ~18 tests that only exercise them. Those
   tests assert real contracts (`private` field on the schema, nested topics,
   error-without-active-project), but they assert them against a code path no
   caller reaches, so their green is uninformative about the shipped tool.
2. **Delete the structs and re-point their tests at `Memory`** with the equivalent
   `action` argument. More work, and the tests become load-bearing for the first
   time — some will need real fixes, because passing against the dead path is no
   evidence they pass against the live one.

Option 2 is the honest one and will probably surface findings. Neither is urgent.

**Not the fix: a comment saying "deprecated".** The reader who was misled here was
reading `call` bodies to trace behaviour, and would have had to scroll past the
struct definition to reach one. The names are the problem, and only removal fixes
names.

## Not established

Whether anything else has been mis-read off these structs. One instance is
confirmed (above); no sweep was done for others. Also unknown whether the ~18
tests would still pass if re-pointed at `Memory` — that is the interesting part
of option 2 and the reason to prefer it.

## References

- `docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md`
  — the investigation that stated the false root cause. Left uncorrected on
  purpose: it is an archived historical snapshot, and rewriting it would falsify
  the record of what was believed. This file is the correction.
- `020ea69a` (patch-id `bf221aac`) — the fix whose reconnaissance found this.

