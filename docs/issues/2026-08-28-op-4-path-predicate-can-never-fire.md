---
status: open
opened: 2026-08-28
closed:
severity: medium
owner: marius
related: []
tags: [operator-rules, routing]
kind: bug
---

# BUG: OP-4's `path~/.claude` predicate can never fire against a real write response

## Summary
`OP-4` (`docs/trackers/operator-rules.md`) declares `**Serves:** edit_file(path~/.claude),
create_file(path~/.claude)`. Its `path~` predicate is routed through
`names_path_containing`, which never sees a path in any write tool's response. The
rule is confirmed dead: it can never be delivered by `operator_rules::route::route`,
even though its selector, matcher, and ledger entry are all individually correct.

## Symptom (Effect)
No error, no panic — a silent non-delivery. `route(Some("edit_file"), &result)` never
includes `OP-4` in its output for any `result` a real `edit_file`/`create_file` call can
actually produce, because none of those responses carry a scanned path field.

## Reproduction
At `HEAD` = `822968d179cddebb5f977b845682fae48179f822` (this worktree,
`sdd/operator-rules-phase-2`):

```rust
let observed = json!({"status": "ok", "wrote_to": "/home/u/work/claude/codescout"});
let hit = route(Some("edit_file"), &observed);
assert!(!hit.iter().any(|r| r.id == "OP-4")); // passes today — OP-4 never fires
```

Run: `cargo test --lib operator_rules::route::tests::op_4 -- --nocapture`

## Environment
Rust, codescout server, `src/operator_rules/route.rs` (Task 3/4 of the
`2026-08-28-operator-rules-phase-2` SDD plan). Branch `sdd/operator-rules-phase-2`.

## Root cause
Two independent facts compose into a dead predicate:

1. `Selector::matches` delegates a `path~` predicate to `names_path_containing`
   (`src/prompts/guide_index.rs:194`).
2. `names_path_containing` (`src/util/librarian_response.rs:36-65`) scans exactly
   four shapes, and only these four:
   - top-level `abs_path`
   - top-level `rel_path`
   - `items[].abs_path` / `items[].rel_path`
   - `violations[].path` (the `doctor`-tool-specific shape)
3. `edit_file`/`create_file` answer with the project's no-echo write convention — a
   bare `"ok"` string. `annotate_write_root`
   (`src/tools/core/types.rs:185-201`) promotes that string to
   `{"status":"ok","wrote_to":<checkout root>}`, and only when the repo has linked
   worktrees. `wrote_to` is not one of the four scanned shapes, so it is invisible to
   `names_path_containing` regardless.
4. Even if `wrote_to` were added as a fifth scanned shape, it would not fix this: the
   value it carries is the **checkout root** of the codescout repo/worktree that
   served the call, not the path of the file that was actually written. For a file
   under `~/.claude` (outside the codescout checkout entirely), `wrote_to` carries no
   relationship to `~/.claude` at all — measured directly below, mutation (2).

No response shape a real `edit_file`/`create_file` call can produce carries the written
path, so `OP-4`'s `path~/.claude` predicate matches nothing, ever.

*Measured 2026-08-28: two mutation-testing passes on this worktree, both reverted
immediately after observing the result (see Evidence).*

## Evidence

### Live `edit_file` response shape (given in the task brief, reproduced here)
```
edit_file on /home/marius/.claude/CLAUDE.md returned:
{"status": "ok", "wrote_to": "/home/marius/work/claude/codescout"}
```

### Mutation 1 — widening the scan to include `wrote_to` still does not fire
Added `|| hit(obj.get("wrote_to"), needle)` to `any_path_field` in
`src/util/librarian_response.rs`, then ran
`cargo test --lib operator_rules::route::tests::op_4 -- --nocapture`:
```
test operator_rules::route::tests::op_4s_path_predicate_cannot_fire_against_a_write_response_today ... ok
test operator_rules::route::tests::op_4s_predicate_is_itself_sound_given_a_path_bearing_response ... ok
```
Both tests stayed green — the characterization test did **not** fire, because
`wrote_to`'s value (`/home/u/work/claude/codescout`, the checkout root) does not
contain the needle (`/.claude`) in the first place. This is the evidence for root-cause
point 4 above: scanning `wrote_to` is necessary-but-insufficient, because `wrote_to`
structurally never carries the actual written path for a file outside the codescout
checkout. Reverted immediately after this run.

### Mutation 2 — a response carrying a real `abs_path` field DOES fire
Added `"abs_path": "/home/u/.claude/CLAUDE.md"` to the characterization test's
`observed` fixture (simulating the eventual fix — a write response that names the
actual written path), then ran the same command:
```
thread '...op_4s_path_predicate_cannot_fire_against_a_write_response_today' panicked at src/operator_rules/route.rs:277:9:
OP-4 fired — the write-response shape gained a path field. This is the GOOD failure:
delete this test, assert delivery, and close the bug file.
test result: FAILED. 1 passed; 1 failed; ...
```
This confirms the selector and matcher are sound — the only missing ingredient is a
response field that names the real written path. Reverted immediately after this run;
`git diff` on both touched files was empty before continuing.

## Hypotheses tried
1. **Hypothesis:** Widening `names_path_containing` to also scan `wrote_to` would let
   `OP-4` fire on real write responses.
   **Test:** Mutation 1 above.
   **Verdict:** rejected — `wrote_to` carries the checkout root, not the written
   file's path, so scanning it changes nothing for files outside the checkout (which
   is exactly `OP-4`'s target: `~/.claude`).
   **Evidence link:** Evidence § Mutation 1.
2. **Hypothesis:** `OP-4`'s selector itself is malformed (parses wrong, or the
   `path~` operator is broken).
   **Test:** Mutation 2 above — feed a response that actually carries the write
   path and confirm the rule routes.
   **Verdict:** rejected — the selector and matcher work correctly given a
   path-bearing response; the gap is entirely in what write tools report back.
   **Evidence link:** Evidence § Mutation 2; also
   `op_4s_predicate_is_itself_sound_given_a_path_bearing_response` in
   `src/operator_rules/route.rs`, which pins this permanently.

## Fix

*Not implemented — deliberately out of scope for the task that filed this bug.*

The remedy is to give write-tool responses (`edit_file`, `create_file`, and likely
their siblings) a genuine path field — e.g. an `abs_path` naming the file actually
written — that `names_path_containing` can see. That is a change to the project's
**no-echo write convention** (`json!("ok")` only; see memory `conventions`), a
project-wide decision with its own tradeoffs (what to add, to which tools, whether it
changes the shape for every consumer of those responses), not a bug fix that can ride
along inside a routing task.

The alternative — widening `names_path_containing`'s top-level scan to also check
`wrote_to` — was considered and rejected on two grounds: (1) the function's own doc
comment (`src/util/librarian_response.rs:22-29`) explicitly declined to widen the
top-level scan to serve a single caller's need, doing so once already for the
`doctor`-specific `violations` shape and stopping there; and (2) Mutation 1 above shows
it would not even fix this specific case, since `wrote_to` never carries the actual
written path for files outside the codescout checkout.

Neither remedy is implemented by this task. No change was made to
`names_path_containing`, to any write tool's response shape, or to `OP-4`'s selector
in the ledger.

- **SHA** — N/A, no fix commit exists yet.
- **patch-id** — N/A.

## Tests added
- `src/operator_rules/route.rs::tests::op_4s_path_predicate_cannot_fire_against_a_write_response_today`
  — characterization test: pins that `OP-4` does not fire against a real (no-echo)
  write response today. **Intentionally an absence assertion** — see the positive
  control below for why it is not vacuous.
- `src/operator_rules/route.rs::tests::op_4s_predicate_is_itself_sound_given_a_path_bearing_response`
  — positive control: pins that `OP-4` DOES fire once a response actually carries a
  path field. Without this test, the characterization test above would be equally
  satisfied by a `route` that always returns nothing (mutation-tested directly: see
  the task's TDD evidence, where forcing `route_in` to `.filter(|_| false)` left the
  characterization test green while this one failed loudly).

## Workarounds
None. `OP-4`'s imperative ("apply every Claude Code config change to all three
profiles") is not enforced by routing; it still appears verbatim in
`~/.claude/CLAUDE.md` § *Three Claude Code Instances* per its `**Rests on:**` field, so
the guidance itself is not lost — only the just-in-time delivery via `route()` is
absent.

## Resume
No further action needed from routing/matcher code. If a future task adds a path field
to write-tool responses (a no-echo-convention change, decided independently of this
bug), then re-run
`cargo test --lib operator_rules::route::tests::op_4 -- --nocapture`: expect
`op_4s_path_predicate_cannot_fire_against_a_write_response_today` to fail with the
message "OP-4 fired — the write-response shape gained a path field... This is the GOOD
failure". At that point: delete that test, add a delivery assertion in its place
(mirroring `op_4s_predicate_is_itself_sound_given_a_path_bearing_response` but against
the real write-response shape), and close this bug file (`status: fixed`, `closed:`
filled in, fix SHA + patch-id recorded).

## References
- `docs/trackers/operator-rules.md` § OP-4
- `src/operator_rules/route.rs` (both pinning tests)
- `src/prompts/guide_index.rs:194` (`Selector::matches` → `path~` delegation)
- `src/util/librarian_response.rs:36-65` (`names_path_containing`)
- `src/tools/core/types.rs:185-201` (`annotate_write_root`)
- `.superpowers/sdd/2026-08-28-operator-rules-phase-2/task-4-brief.md`
