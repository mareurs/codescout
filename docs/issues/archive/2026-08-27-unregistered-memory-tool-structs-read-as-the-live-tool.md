---
id: 2cdc5815808a6634
kind: bug
status: open
title: 'BUG: four unregistered `impl Tool` blocks in the memory module read as the live tool, and a bug investigation stated a false root cause from them'
tags:
- memory
- dead-code
- tools
- misleading-source
closed: 2026-08-27
opened: 2026-08-27
owner: marius
related:
- docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md
severity: low
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

## Fix

**Shipped 2026-08-27** — `82d5d48c` (`experiments`), patch-id
`d13480c75b14eeaa76015f679a855159d44ebbab`. Option 2: all four structs deleted (194 lines
of `mod.rs`), their 19 tests re-pointed at the live `Memory` tool with an explicit
`action`.

### CORRECTION — § *Root cause* above is wrong, and this file wrote it hours ago

It says: *"A tool consolidation that introduced the unified `Memory` tool left the four
per-action tools in place rather than deleting them, and their tests kept passing — so
nothing reported them as dead."*

The last clause is right. The first is not. `git log -S` finds `282582a8` (2026-04-30):

> **chore(memory): gate legacy single-action structs to test builds**

They were identified as legacy four months ago and deliberately put behind
`#[cfg(test)]` — a considered intermediate step, not an oversight. That also means they
never existed in a release build, so the earlier claim that `server.rs` "never registers
them" understates it: they **cannot** be registered, and the strongest possible statement
that they are not live was sitting two lines above each struct the whole time.

Which makes the finding sharper rather than weaker. The halfway house is what kept the
hazard alive: a `#[cfg(test)]`-gated `pub(crate) struct` still reads like a live one when
the 40-line `impl Tool` beneath it is entirely plausible and shares its name with a tool
**action**. The reader who was misled had the signal available and scanned past it.
Gating documented the intent for whoever wrote it; deleting is what communicates it to
everyone else.

(This file was written during the session that fixed the sibling bug, which is the
condition `reconnaissance-patterns:R-49` names — a self-authored artifact re-read on
re-entry, wrong on a checkable point of history its author never checked.)

### The predicted findings did not materialise

§ *Fix* option 2 predicted the tests *"become load-bearing for the first time — some will
need real fixes, because passing against the dead path is no evidence they pass against
the live one."*

**All 19 passed against `Memory` unchanged, first run.** The prediction was reasonable
and wrong, and is recorded here rather than quietly dropped: the re-point was worth doing
for the deletion it unblocked, not for the defects it was expected to shake out.

### What it did find — the mechanism, confirmed by the compiler

§ *Root cause* argued that "dead code with 18 passing tests looks maintained" and that
Rust cannot warn because the tests construct them in the same crate. That is now measured
rather than argued. The moment the tests stopped constructing them, `cargo test` emitted:

```
warning: struct `WriteMemory` is never constructed
warning: struct `ReadMemory` is never constructed
warning: struct `ListMemories` is never constructed
warning: struct `DeleteMemory` is never constructed
```

The same four `references()` had named. **The tests were the only thing standing between
this code and the lint that exists to find it.**

### Four schema tests became one

`write_` / `read_` / `delete_memory_schema_has_private_field` and
`list_memories_schema_has_include_private_field` each asserted `private` or
`include_private` on the schema of a tool no client is served. There is one live schema,
so there is one test: `memory_schema_carries_the_private_store_fields`.

### Duplication created, and deliberately left

Re-pointing made three tests near-duplicates of ones that already exercised `Memory`:

| re-pointed | pre-existing |
|---|---|
| `write_and_read_roundtrip` | `memory_write_and_read_via_dispatch` |
| `delete_removes_entry` | `memory_delete_via_dispatch` |
| `nested_topic_works` | `memory_write_and_read_via_dispatch` (its `"test/key"` is a nested path) |

They are not identical — different topics, and the dispatch ones use
`assert_memory_write_ok` — the cost is negligible, and deleting pre-existing coverage to
tidy up *while doing something else* is not a call to make in passing. Listed here so a
later consolidation has the set rather than having to re-derive it.

**`list_after_writes` is not on that list**: it pins list **sort order**, which no
dispatch test asserts. It was the one test of the nineteen with coverage nothing else
had.

### Gate

`cargo fmt`, `cargo clippy --all-targets -- -D warnings` (the four `dead_code` warnings
are gone), `cargo test` — **4612 passed, 0 failed**. Net −3 tests: four schema tests
removed, one added.

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
