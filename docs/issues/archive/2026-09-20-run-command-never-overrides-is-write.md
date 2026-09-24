---
id: 3cee1969ae4e9d57
kind: bug
status: fixed
title: 'BUG: run_command never overrides is_write, so every shell command is classified as a read'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- run-command
- write-guard
- telemetry
- concurrency
topic: write guard coverage and shell effect classification
closed: 2026-09-24
opened: 2026-09-20
severity: high
unverified: Only the DECLARED effect class is recorded. The OBSERVED one (what a command actually did to the tree) is not, and the Fix section asks for both because only the observation survives a wrong declaration.
---

# BUG: `run_command` never overrides `is_write`, so every shell command is classified as a read

## Summary

`Tool::is_write` defaults to `false` and its own doc comment says "Override on every mutating
tool." `RunCommand` does not override it. `run_command` executes arbitrary shell — the one tool in
the registry whose write-ness is genuinely unknowable from its arguments — and is therefore
recorded and gated as read-only whatever the command did. It was the highest-volume tool in the
frozen baseline at 8,382 calls.

## Symptom (Effect)

No error, and nothing in the response differs. Two distinct consequences:

1. **The cross-process write guard never engages for shell work.** `run_command "rm -rf target"`,
   `run_command "git checkout ."` and `run_command "cargo fmt"` all skip the lock that
   `edit_file` and `edit_code` acquire, on a checkout CLAUDE.md describes as routinely carrying
   ~6 concurrent sessions.
2. **Telemetry cannot separate mutating from non-mutating shell work.** `usage.db` has no
   effect-class column of its own, so `is_write` is the only signal that could have distinguished
   `run_command "ls"` from `run_command "git reset --hard"`. Both record identically.

## Reproduction

Read the trait impl's method list:

```
symbols(path="src/tools/run_command/mod.rs")
```

`impl Tool for RunCommand` (`src/tools/run_command/mod.rs:92-330`) declares exactly `name`,
`relevant_guide_topic`, `description`, `long_docs`, `input_schema`, `call`, `format_compact`,
`availability`. No `is_write`.

Corroborate across the whole crate rather than one directory — `grep "fn is_write"` over
`src/**/*.rs` returns 21 matches, and **no file under `src/tools/run_command/` is among them**.
The 21 are: `librarian/adapter.rs`, `tools/config/mod.rs`, `tools/core/tests.rs` (×3, test
doubles), `tools/core/types.rs` (the default), `tools/edit_file/mod.rs`, `tools/memory/mod.rs`,
`tools/semantic/index.rs` (×2), `tools/symbol/edit_code.rs`, `tools/approve_write.rs`,
`tools/create_file.rs`, `tools/library.rs` (×2), `tools/onboarding.rs`, plus `server.rs`'s
`is_write_call` and its tests.

## Environment

Branch `experiments`, HEAD `4098ad3e`. Present since `RunCommand` was written.

## Root cause

The default is permissive and the override is by convention. `src/tools/core/types.rs:1193-1200`:

```rust
/// Defaults to `false` (read-only). Override on every mutating tool.
/// For tools whose write-ness depends on input (e.g. `memory` switches on
/// `action`), inspect `input` to decide. The server calls this after
/// argument parsing, so `input` is already the same `Value` that `call()`
/// will receive.
fn is_write(&self, _input: &Value) -> bool {
    false
}
```

The gate reads it through one site — `CodeScoutServer::is_write_call`
(`src/server.rs:602-605`):

```rust
self.find_tool(tool_name)
    .map(|t| t.is_write(input))
    .unwrap_or(false)
```

consumed by `acquire_write_guard_if_writing(&req.name, &input_for_record)` at
`src/server.rs:1366-1368`. A tool that omits the override is indistinguishable at that site from
one that deliberately declared itself a read.

**The advice in the doc comment does not reach this tool.** "inspect `input` to decide" works for
`memory` and `workspace`, which dispatch on a closed `action` enum. `run_command`'s input is a
shell string; deciding write-ness from it is the halting problem wearing a costume. So
`run_command` is not a tool that *forgot* the override — it is the tool for which the prescribed
technique does not exist, which is why the omission is stable rather than an oversight anyone
would have caught by following the comment.

Measured 2026-09-20 by reading the trait default, the impl's method list, and the gate site; not
observed at runtime.

## Evidence

Volume, from the frozen UTC window [2026-09-04, 2026-09-18) in
`docs/research/2026-09-18-deep-agent-observation-baseline.md`:

| tool | calls | p50 / p95 ms |
|---|---:|---:|
| run_command | 8,382 | 71 / 43,055 |

Highest call count of any tool in that window, and `edit_file` (2,183) and `edit_code` (866) —
both of which *do* override `is_write` to `true` — are third and seventh.

CLAUDE.md already states the consequence from the other direction, in § *Companion Plugin*: the
IL-3 unbounded-pipe block and the dangerous-command `@ack_*` gate are named as things native
`Bash` does not get and `run_command` does. The write guard is a third gate, and `run_command`
does not get that one either.

## Hypotheses tried

1. **Hypothesis:** the override lives elsewhere — on a wrapper, or in the `inner` submodule.
   **Test:** `grep "fn is_write"` across `src/**/*.rs`, all 21 matches enumerated above.
   **Verdict:** rejected — no file under `src/tools/run_command/` implements it. The initial grep
   was scoped to that directory alone and returned 0; widening to the crate is what makes the
   zero a measurement rather than a filter artifact.

2. **Hypothesis:** `run_command` is exempt because the server sandboxes or otherwise contains it.
   **Test:** read `availability` and `call` on the impl.
   **Verdict:** not established either way here. Whatever containment exists, it is not the
   `is_write` write guard, which is the gate this bug is about.

## Fix

**FIXED 2026-09-24 at `399f2a63`, by operator ruling "Declare + record, no lock"** — taken after
a measurement changed the question. The server holds the cross-process write lock for the WHOLE
call and refuses waiters after `write_lock_timeout_secs` (5), while `run_command`'s p95 is 43 s, so
the first two options below would have turned one session's foreground `cargo test` into every
peer's refused edits — not the per-`ls` latency they were first priced at.

Shipped: an `effect` parameter (`"read"` | `"write"`, default `"write"`), one definition
(`run_command::declared_effect`) read by the tool (refuses anything else) and by the usage
recorder, which writes it to a new `usage.db` column `effect_class` — the declared class for
`run_command`, NULL for every other tool. `RunCommand::is_write` is now an explicit `false` with
the ruling in its doc comment, so consequence 1 above (the lock never engages for shell work) is
a **decision**, pinned by `run_command_never_takes_the_write_lock_whatever_it_declares`, rather
than an omission. Consequence 2 (telemetry cannot separate mutating from non-mutating shell work)
is closed for the DECLARED class.

Not done, and recorded in `unverified:`: the OBSERVED effect class this section asks for.

The options as originally filed, kept for the record:

- **Declare it always-write.** One line, `true`. Correct and safe; costs every `run_command "ls"`
  the cross-process write lock, on the highest-volume tool in the registry.
- **Classify the command string.** A conservative parse — a new parser over an open namespace.
- **Add a caller-declared effect class** — an explicit parameter, defaulting to write.

## Tests added

In `src/tools/run_command/mod.rs` (`effect_tests`), `src/server.rs` and `src/usage/mod.rs`:
`declared_effect_defaults_to_write_and_accepts_only_read_or_write`,
`run_command_refuses_an_effect_it_does_not_know`,
`run_command_never_takes_the_write_lock_whatever_it_declares` (the ruling pin — inert for a red
on the fixing commit by design), and
`record_content_stores_run_commands_declared_effect_and_nothing_for_other_tools`. Mutation via
`scripts/mutation-probe.sh`, 7/7 KILLED.

## Fix provenance

- **SHA:** `399f2a63` (`experiments`)
- **patch-id:** `d19a1d941edf55d79bed099cf7d9d1df3a510aa1`

## Workarounds

Treat any `run_command` row in `usage.db` as effect-unknown, never as a read. For concurrency,
the write guard is not protecting shell work today — sequence destructive shell commands against
peers by hand (`CLAUDE.md` § *Reaching a Peer Session*), not by trusting the lock.

## Resume

N/A for what was ruled. The observed-effect half is open work, not an obligation of this fix; see `unverified:`.

## References

- `docs/issues/archive/2026-09-02-is-write-omits-five-mutating-actions-so-the-write-guard-never-fires.md`
  — same mechanism on the librarian surface; tagged `cluster/guard-narrower-than-its-name`.
  Its *The test cannot fail on an omission* section is the reason this bug could sit unfiled: the
  suite asserts that listed actions ARE writes and nothing asserts the list is complete.
- `docs/issues/archive/2026-09-03-workspace-activate-writes-libraries-json-outside-the-write-lock.md` —
  same mechanism on `workspace`.
- `docs/issues/archive/2026-06-01-librarian-adapter-stale-is-write.md` — same mechanism,
  stale tool names.
- **Prior-instance count deliberately not stated.** Derive it rather than cite it:
  `doc(action="find", kind="bug", include_archived=true, filter={"tags": {"contains":
  "cluster/guard-narrower-than-its-name"}})`. A number written here decays on the next instance
  and cannot be re-checked without running that query anyway
  (`CLAUDE.md` § *Observer Blindness*, third position).
- **Cluster fit.** `IC-14`, "a guard's coverage is narrower than its name". The write guard's name
  promises it covers writes; its coverage is the set of tools that remembered to say so. This
  instance differs from its siblings in one way worth recording: the others omitted an entry from
  an enumeration they could have completed, whereas here the prescribed technique — "inspect
  `input` to decide" — is not available at all.
