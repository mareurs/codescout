---
kind: bug
status: fixed
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-09-01
opened: 2026-09-01
owner: marius
related: []
severity: medium
---

# BUG: `library` is gated on having a library registered, and it is the only tool that can register one

## Summary

`Library::availability()` returned `RequiresLibraries`, i.e. `caps.has_libraries`, which
`current_capabilities` computes as *"at least one library is already registered for the active
project"*. But `library` is the **only** surface on which a library can be registered —
`register` is one of its actions. The tool that establishes the precondition was gated on that
precondition, so it was absent from `tools/list` in exactly the state where it is needed.

The capability was never actually lost: the tool stayed **dispatchable** throughout. This is a
discoverability defect, and that is the interesting part — discovery and dispatch disagreed, and
only discovery is a surface an agent can read.

## Symptom (Effect)

Fresh git repo, no registered libraries, driving the real binary over stdio JSON-RPC:

```
tools advertised: 15
library present : False
```

Calling the "absent" tool anyway, in the same project:

```
id=3 isError=False
    Registered library 'codescout' (rust)
```

So the tool that `tools/list` does not mention registers a library on the first attempt.

## Reproduction

```
mkdir /tmp/emptyproj && cd /tmp/emptyproj && git init -q . && echo hi > notes.txt
git add -A && git -c user.email=p@p -c user.name=p commit -qm base

printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"p","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  | /path/to/target/debug/codescout start
```

`library` is absent from the returned tool list. Swap request 2 for a
`tools/call` of `library` with `{"action":"register","path":"<any repo>"}` and it succeeds.

**The project must be genuinely fresh.** Registering once flips `has_libraries` to true and the
tool appears from then on, so a second run in the same directory cannot reproduce it — that is
the bug repairing itself the moment you work around it, and it is why this survived.

## Environment

Linux, Rust, codescout `0.15.0`, branch `experiments`, MCP stdio transport, feature `librarian`
enabled. Reproduced against `a19a6675`.

## Root cause

A capability gate whose precondition can only be established by the capability it gates.

- `src/tools/library.rs` — `Library::availability()` delegated to
  `ListLibraries.availability(caps)`, which returned `Availability::RequiresLibraries`.
- `src/tools/core/types.rs` — `Availability::RequiresLibraries => c.has_libraries`.
- `src/server.rs` — `has_libraries = self.agent.library_registry().await.map(|reg|
  !reg.all().is_empty()).unwrap_or(false)`.
- `src/server.rs:349` — `Arc::new(Library)` is the **only** registration; `register` and `list`
  are actions dispatched inside it, not separate tools.

**Measured 2026-09-01**, not inferred: the stdio reproduction above, plus the successful
`register` call against the same server that omitted the tool.

**Why it survived.** `ActivateProject::call` runs `auto_register_deps` on every project
activation, which populates the registry from detected dependency manifests. Any project with a
`Cargo.toml`, `package.json` etc. therefore has `has_libraries == true` before anyone reads the
tool list, and the gate never bites. It bites precisely for the project with **no** detectable
manifest — which is exactly the project that needs to register a library by hand.

**Why `-D warnings` never said anything about the dead flag left behind.** After the fix,
`Availability::RequiresLibraries` has no producer and `has_libraries` no consumer, and clippy
stays silent: Rust's `dead_code` lint cannot fire on `pub` items in a **library** crate, because
it cannot know the crate's consumers. That is `IC-3`'s documented blind party appearing in a
tool rather than a person, and it is why this file names the leftovers explicitly below rather
than trusting the gate to surface them.

## Evidence

### The causal proof is the unit test, not the field reproduction

Two stdio runs on fresh directories gave 15 tools (pre-fix) and 23 (post-fix), and **that delta
is not the fix.** Diffing the two sets:

```
gained: artifact, artifact_augment, artifact_event, artifact_refresh,
        index, librarian, library, semantic_search
lost  : (none)
```

Only `library` is attributable to this change. The other seven are gated on
`librarian_enabled_at_runtime` and on `has_embeddings`, which resolved differently between the
two runs — the environment moved underneath the measurement, so the runs are not comparable and
`15 → 23` must not be quoted as this fix's effect.

The clean causal evidence is the regression test with the environment held constant: reverting
`Library::availability` to `RequiresLibraries` produces exactly one difference —

```
Visible: ["read_file", "tree", "grep", ..., "librarian"]        // 25 tools, `library` absent
```

— and restoring it returns 26. One variable, one difference.

### Classification, and the two classes rejected

Tagged `cluster/gate-keyed-on-unobservable-event` (`IC-2`). The gate wants to know *"does this
project use libraries?"*, cannot observe that, and substitutes the proxy *"is one already
registered?"* — which is downstream of the very action it gates. The proxy fails silently: the
tool simply is not there.

- **Not `IC-3` (`declared-not-wired`)**, by that entry's own discriminator: *"was a
  caller-supplied value accepted? If no, the capability exists and no call site reaches it."*
  Here a call site **does** reach it — the `register` call succeeded. `IC-3` is further
  falsified by its own *Falsified by* line.
- **Not `IC-13` (`capped-result-presented-as-complete`)**, on the remedy test. `IC-13`'s claim
  is a *limit* — a page size, a byte budget — and its remedy is a truncation marker. This is a
  *filter*, and the remedy is to fix the predicate, not to annotate the filtering.

## Hypotheses tried

1. **Hypothesis:** the tool is genuinely unreachable, so the capability is lost.
   **Test:** called `library(action="register", path=…)` against a server that had just omitted
   it from `tools/list`.
   **Verdict:** rejected — `isError: false`, `Registered library 'codescout' (rust)`. Severity
   is discoverability, not function. Worth stating because it changes the fix: nothing in the
   dispatch path needed touching.

2. **Hypothesis:** this is a third instance of *advertised ≠ accepted* and therefore trips the
   `tool-registration-rule-of-three` in `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md`.
   **Test:** applied that spec's remedy test. Its remedy is `schemars`-derived schemas, which
   fixes **parameter**-level mismatch between an `Args` struct and its advertised schema.
   **Verdict:** rejected. This is tool-level visibility under a runtime capability predicate;
   deriving schemas from `Args` would not touch it. Different remedy, different class — the
   count stays at 2 confirmed, 1 live.

## Fix

`src/tools/library.rs`:

- `Library::availability()` now returns `Availability::Always`, with the reasoning and the
  measurement on the method so the next reader does not re-gate it.
- Deleted `ListLibraries::availability` and `RegisterLibrary::availability`. Neither tool is
  registered (`Arc::new(Library)` is the only one), so `RegisterLibrary`'s override was **never
  consulted at all** — a dead gate — and `ListLibraries`' was reachable only via the delegation
  this fix removes. Both now inherit the `Always` default, which is what they always effectively
  had.

Cost: `library` is 825 chars of tool surface, paid only by projects that previously hid it.
Projects with a registered library already advertised it, so their surface is unchanged.

**Deliberately left, and named rather than silently kept:** `Availability::RequiresLibraries`
and `ToolCapabilities::has_libraries` now have no producer and no consumer respectively.
Removing them touches the `Availability` enum, its `is_available` match, the `ToolCapabilities`
struct and ~8 test constructions — a coherent cleanup, but a public-shape change that deserves
its own decision rather than riding along inside a bug fix. It is `IC-3`-shaped and invisible to
`-D warnings` (see Root cause), so it is recorded here instead of left to be rediscovered.

**Fix commit:** `095088c4` on **`experiments`** (full: `095088c46ff954378efbc5081b0112deef340cb0`).
**patch-id:** `6eeea1df464272649b84c6604d9403a579727bf3` (`git show 095088c4 | git patch-id --stable`).

Both are recorded because they fail differently: the SHA is positional and dies when
`experiments` is rebased, while the patch-id is a content hash of the diff that survives rebase
*and* cherry-pick. Nothing is owed later.

## Tests added

`server::tests::library_is_advertised_when_no_library_is_registered_yet` (`src/server.rs`).

**Shown able to fire, not assumed:** reverting `Library::availability` to `RequiresLibraries`
turns it red with `library` absent from a 25-tool list; restoring gives 26. The load-bearing
fixture detail is `has_libraries: false`, annotated on the fixture line — flip it to `true` and
the test passes against the unfixed code, because `true` is exactly the state the old gate
admitted.

## Workarounds

Call `library(action="register", path=…)` anyway. It was always dispatchable; only the
advertisement was missing. This requires knowing the tool exists, which is the whole defect.

## Resume

N/A — fixed. Optional follow-up: decide whether to delete `Availability::RequiresLibraries` and
`ToolCapabilities::has_libraries`, per § Fix.

## References

- `docs/trackers/prompt-surface-compaction-session-log.md` (`03464a8808345846`) — **F-3**, the
  open pre-registered experiment on the four zero-call tools, which names `library`. This bug is
  a direct input: `library`'s zero calls are neither "dead" nor "unrouted" but **auto-satisfied**
  — `auto_register_deps` does the job transparently, and the manual path was unadvertised anyway.
- `docs/trackers/issue-clusters.md` — `IC-2` (this class), `IC-3` and `IC-13` (rejected, above).
- `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md` (`0e0316e9036d7f16`) —
  the rule-of-three this was checked against and does not join.
