---
kind: bug
status: fixed
tags:
- cluster/declared-not-wired
- operator-rules
- routing
closed: 2026-08-31
opened: 2026-08-28
owner: marius
related:
- docs/issues/2026-08-28-triggered-operator-rules-route-nothing-in-production.md
- docs/adrs/2026-08-31-write-responses-name-the-path-they-wrote.md
severity: medium
unverified: 'CLOSED 2026-09-01, first half: `tools::core::tests::a_real_edit_file_write_under_dot_claude_delivers_op_4` drives the REAL EditFile through call_content against an absolute path under `.claude` and asserts the OP-4 block arrives in the returned content — one call, no hand-supplied selector, no hand-fed route() call, with the negative control run FIRST so the once-per-session ledger cannot make its silence vacuous. Writable only because 30b6fc41 inverted Tool::selector_key''s default, so EditFile now supplies its own selector and call_content runs the router itself. Mutation-checked: removing the annotation kills it plus two siblings (so it establishes the annotation matters, not this test''s unique necessity); forcing the key to rel_path does NOT kill it, because names_path_containing scans both keys. STILL OPEN, second half: the path is captured from input["path"] specifically, so a write tool using a different key for its target gets no annotation and any rule serving it would be dead the same way. edit_file and create_file both use `path`, so no live rule is affected today.'
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
   (`src/tools/core/types.rs:184-201`) promotes that string to
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

There is also a second, independent blocker that stops this even earlier: neither
`edit_file` nor `create_file` overrides `Tool::selector_key` at all, so in production
`route()` is never even called with `Some("edit_file")` — both of this bug file's own
tests pass that selector directly rather than obtaining it from a real call, which is
why they exercise the predicate in isolation but not the actual routing path. This is
not specific to `OP-4`: no production tool outside the `LibrarianAdapter` family
overrides `selector_key`, so every `triggered` rule keyed to a non-librarian tool is
unreachable the same way. Full finding:
`docs/issues/2026-08-28-triggered-operator-rules-route-nothing-in-production.md`.

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

**Fixed 2026-08-31 at `a6b4fc35`** (patch-id `2d962f961ed764458b74ed5d0b67ed197945b957`),
after the routing precondition landed at `2447f709`. Both were needed: without the
selector, `route()` never ran; without the path, the predicate had nothing to match.

`annotate_write_path` in `call_content` names the written path as `abs_path` (or
`rel_path` when relative — `names_path_containing` scans both, and a relative path filed
under `abs_path` would be a lie the matcher happens not to notice). Sibling to
`annotate_write_root` rather than folded into it, and gated on `is_write` alone: that
one's `workspace_override` gate exists because a pinned call already named its checkout,
which says nothing about which file it wrote.

Rationale, the rejected alternative and its measurement, and the scope of the no-echo
exception are in `docs/adrs/2026-08-31-write-responses-name-the-path-they-wrote.md`.
No tool's `call` return changed, so none of the 44 `json!("ok")` assertions moved.

**The pin could not fire, which is the finding this file should carry forward.**
`op_4s_path_predicate_cannot_fire_against_a_write_response_today` was written to detect
this exact fix — *"when this test starts failing, that is the fix landing"* — and it did
not fail, because its fixture was a hand-written response bound to a variable named
`observed`. A tripwire aimed at a fabricated input can see neither the defect nor its
repair. Removed per its own instruction and replaced by
`op_4_routes_on_a_write_response_the_pipeline_produced`, which routes on a response the
pipeline produced, with a negative control for a path outside `~/.claude`.

**Update 2026-08-31 — the routing precondition is now closed; this bug is not.**
`2447f709` gives `edit_file` and `create_file` a `selector_key`, so `route()` is now
actually consulted for them and the synthetic `Some("edit_file")` in this module's tests is
no longer synthetic. That was a necessary precondition — without it, fixing the predicate
would have changed nothing observable — and it is not a fix for the predicate itself.

The predicate still cannot fire, for the reason recorded here: `names_path_containing`
reads the tool's **response**, and write tools answer `"ok"` under the no-echo convention.

**One correction to the deferral, which offered a single option.** `route.rs` records this
as *"giving writes a path field is a change to the no-echo convention, not a bug fix"* —
true, and it is not the only route. `route(sel, result)` never receives the call's **input**
at all, and the input is already captured in `call_content` before `self.call` consumes it
(`types.rs`, the `selector` binding). So a second option exists: match `path~` against the
input rather than the response. That changes a signature documented as a stable entry point
rather than a convention — a different cost, not obviously a larger one, and it leaves
no-echo intact.

Neither option is taken here: both are design decisions with blast radius beyond a bug fix,
and which convention to bend is the operator's call rather than a drive-by. Recording the
second option because the deferral as written reads as a dead end, and a rationale that
names one option is the least-audited kind of claim (`reconnaissance-patterns:R-95`).

*Not implemented — deliberately out of scope for the task that filed this bug.*

The remedy is to give write-tool responses (`edit_file`, `create_file`, and likely
their siblings) a genuine path field — e.g. an `abs_path` naming the file actually
written — that `names_path_containing` can see. That is a change to the project's
**no-echo write convention** (`json!("ok")` only; see memory `conventions`), a
project-wide decision with its own tradeoffs (what to add, to which tools, whether it
changes the shape for every consumer of those responses), not a bug fix that can ride
along inside a routing task.

The alternative — widening `names_path_containing`'s top-level scan to also check
`wrote_to` — was considered and rejected on two grounds: (1) an in-body comment in
the function itself (`src/util/librarian_response.rs:55-60`) explicitly declined to
widen the top-level scan to serve a single caller's need, doing so once already for the
`doctor`-specific `violations` shape and stopping there; and (2) Mutation 1 above shows
it would not even fix this specific case, since `wrote_to` never carries the actual
written path for files outside the codescout checkout.

Neither remedy is implemented by this task. No change was made to
`names_path_containing`, to any write tool's response shape, or to `OP-4`'s selector
in the ledger.

*(Superseded — this pair read `N/A, no fix commit exists yet` while the file was still
`open`, and the fix landed afterwards. The declared anchors are in § *Fix provenance*
below. Kept rather than deleted because the paragraph above it is the scope decision that
made the N/A true at the time.)*

## Fix provenance

- **SHA:** `a6b4fc35` (`experiments`)
- **patch-id:** `2d962f961ed764458b74ed5d0b67ed197945b957`
- **SHA:** `2447f709` (`experiments`)
- **patch-id:** `f83c6439691efb24ca790d00752e7cc7a43a74fe`

Two commits, neither sufficient alone — which is why both are declared rather than one
named as *the* fix. `2447f709` supplies the `selector_key` without which `route()` never
runs; `a6b4fc35` supplies the written path the predicate matches. Read `unverified:` before
treating this as end-to-end covered.
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

Fixing the path-shape gap alone will not make `OP-4` route in production: the
selector-key blocker above stops it earlier and needs its own, separate fix — see
`docs/issues/2026-08-28-triggered-operator-rules-route-nothing-in-production.md` for
its scope and the smallest-fix candidate discussed there.

## References
- `docs/trackers/operator-rules.md` § OP-4
- `src/operator_rules/route.rs` (both pinning tests)
- `src/prompts/guide_index.rs:194` (`Selector::matches` → `path~` delegation)
- `src/util/librarian_response.rs:36-65` (`names_path_containing`)
- `src/tools/core/types.rs:184-201` (`annotate_write_root`)
- `docs/issues/2026-08-28-triggered-operator-rules-route-nothing-in-production.md` (the
  broader selector-key blocker)
- `.superpowers/sdd/2026-08-28-operator-rules-phase-2/task-4-brief.md`
