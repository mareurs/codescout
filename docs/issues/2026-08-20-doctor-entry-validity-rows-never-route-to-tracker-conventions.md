---
status: open
opened: 2026-08-20
closed:
severity: medium
owner: marius
related: []
tags:
  - librarian
  - doctor
  - prompt-surface
  - guide-routing
kind: bug
---

# BUG: `librarian(action="doctor")` routes entry-validity rows to the wrong guide

## Summary

`get_guide("tracker-conventions")` is the only runtime surface that teaches
`**Valid:**`, `**Rests on:**` and the four entry-validity checks. It is
auto-injected when a librarian response *names a tracker path* — but the
`doctor` response keys its rows under `violations[].path`, which the trigger
does not scan. So the agent most likely to need the concept, one holding a
worklist of `entry_cited_from_outside_but_undeclared` rows, is the one
guaranteed not to be handed the guide that explains them. It receives
`get_guide("librarian")` instead, which mentions the concept zero times.

## Symptom (Effect)

`librarian(action="doctor")` on this repo returns rows like:

```
R-2 is cited 20× from other files and declares no **Valid:** class — add one of: …
```

alongside `_guide_hint`:

```
First call this session for topic 'librarian'
```

The injected guide is `librarian` (20,545 B, 0 mentions of `**Valid:**` or any
of the four check names). `tracker-conventions` (27,172 B, 7 mentions) is not
injected, despite the response naming ~30 paths under `docs/trackers/`.

## Reproduction

Commit: `a98b8c43` (branch `experiments`).

In a **fresh** MCP session — the guide ledger is per-session and each topic
auto-injects at most once, so a session that has already fetched either topic
cannot observe this:

1. `workspace(action="activate", path=<codescout root>)`
2. `librarian(action="doctor")`
3. Read `_guide_hint` on the response.

Expected: `tracker-conventions`, since the payload names tracker paths and the
rows are about tracker entry fields. Got: `librarian`.

## Environment

Linux, codescout `v0.15.0`, branch `experiments`, stdio MCP transport, project
`codescout`.

## Root cause

`names_tracker_path` (`src/librarian/adapter.rs:276-292`) decides the topic. It
checks `abs_path` and `rel_path`, at the top level of the response and one level
into a `find`-style `items` array, and its own doc comment calls this
"deliberately shallow" — a deep walk over an arbitrary response was judged to
cost more than the guide saves.

The `doctor` response satisfies neither branch:

- Its rows live under the key `violations` (`src/librarian/tools/doctor.rs:466`),
  not `items`.
- `Violation`'s path field is named `path`
  (`src/librarian/tools/doctor.rs:135`), not `abs_path`/`rel_path`.

So `names_tracker_path` returns `false`, the `tracker-conventions` branch is
skipped, and the fallback `librarian` topic wins.

This is not a regression — the shallow scan predates the entry-validity checks.
What changed is that `doctor` acquired four checks whose remediation lives
entirely in `tracker-conventions`, which turned a benign routing gap into a
teaching gap.

**Measured 2026-08-20** — static, not runtime: `names_tracker_path` read at
`src/librarian/adapter.rs:276-292`; the `violations` key read at
`src/librarian/tools/doctor.rs:466`; the `path` field name read at
`src/librarian/tools/doctor.rs:135`. The runtime `_guide_hint: 'librarian'`
observation in *Symptom* was made by a subagent earlier the same day, in a
session where neither topic had yet been triggered. It has **not** been
re-observed since, because this session had already triggered
`tracker-conventions` explicitly, which suppresses the very injection under
test.

## Evidence

### `names_tracker_path` — `src/librarian/adapter.rs:283-292`

```rust
fn any_path_field(obj: &Value) -> bool {
    is_tracker_path(obj.get("abs_path")) || is_tracker_path(obj.get("rel_path"))
}

if any_path_field(result) {
    return true;
}
result
    .get("items")
    .and_then(Value::as_array)
    .is_some_and(|items| items.iter().any(any_path_field))
```

### The doctor payload's shape — `src/librarian/tools/doctor.rs:466` and `:135`

```rust
"violations": all_violations,
```

```rust
/// The path string that triggered the violation.
pub path: String,
```

## Hypotheses tried

1. **Hypothesis:** the topic is simply absent from the routing table.
   **Test:** read `PULL_ONLY_GUIDE_TOPICS` (`src/prompts/mod.rs:391`) and the
   `relevant_guide_topic` branch (`src/librarian/adapter.rs:182-205`).
   **Verdict:** rejected — `tracker-conventions` *is* wired and is not
   pull-only. It routes correctly for `artifact` calls naming tracker paths;
   only `doctor` misses.
   **Evidence:** `src/librarian/adapter.rs:182-205`.

## Fix

Not yet implemented. Two options, in preference order:

1. **Teach `names_tracker_path` the `violations` shape** — add
   `violations[].path` to the scan alongside `items`. Smallest change, and it
   keeps the "does this response name a tracker path" framing intact. Note the
   field is `path`, so `any_path_field` needs to accept that key too, which
   widens the top-level check as a side effect — check that no other response
   carries an unrelated `path` key that would now mis-route.
2. **Branch on the check names** — return `Some("tracker-conventions")` when any
   violation's `check` starts with `entry_` or equals `validity_unparseable`.
   More precise and immune to key-shape drift, but couples the adapter to
   doctor's check vocabulary, which `Violation::check`'s own doc comment warns
   has gone stale before.

Partially mitigated already: the four checks' `detail` strings now carry their
own remediation text, so a mis-routed agent can still fix the row from the
report alone. That is a mitigation of the *consequence*, not of this defect.

## Tests added

None yet — this file is the capture, the fix is not written. A fix must add a
test asserting `relevant_guide_topic` returns `Some("tracker-conventions")` for
a synthetic doctor response carrying one `violations[].path` under
`docs/trackers/`.

## Workarounds

Call `get_guide("tracker-conventions")` explicitly after any
`librarian(action="doctor")` run that reports `entry_*` or
`validity_unparseable` rows.

## Resume

Read `relevant_guide_topic` at `src/librarian/adapter.rs:182-205` and decide
between Fix options 1 and 2 above. Before choosing option 1, grep the librarian
tool responses for a top-level `path` key that is *not* a tracker path
(`grep -rn '"path":' src/librarian/tools/`) — if one exists, option 1 mis-routes
it and option 2 is correct. Then add the `relevant_guide_topic` test named under
*Tests added*.

## References

- `src/librarian/adapter.rs:182-205`, `:276-292` — the routing decision
- `src/librarian/tools/doctor.rs:135`, `:466` — the payload shape
- `src/prompts/guides/tracker-conventions.md` — the guide that should be served
- `docs/manual/src/concepts/statement-validity.md` — the concept
- `docs/trackers/statement-validity-session-log.md` — the work stream that
  surfaced this
