---
id: '6cb828a80c543bd3'
kind: plan
status: draft
title: Operator Rules Engine — Phase 2 Implementation Plan (triggered-rule routing)
owners:
- marius
tags:
- operator-rules
- routing
- prompt-surface
- phase-2
- selector
topic: operator rules engine
---

# Operator Rules Engine — Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver `triggered` operator rules just-in-time on the tool calls whose shape they serve, so `OP-2` and `OP-3` stop sitting resident in every CLAUDE.md profile and competing with `OP-1`.

**Architecture:** Reuse the section-grain matcher wholesale — `guide_index::parse_shape` for the `**Serves:**` grammar, `guide_index::Shape::matches` for the match, and `GuideLedger` for once-per-session stamping under an `op:` key namespace. **This is a second corpus fed to the same matcher, not a second matcher.** The ledger is compiled in with `include_str!` (as the guide corpus is), because routing must work in every project the server activates against while the ledger file lives only in the codescout repo. `compile`/`check` keep reading from disk, so profile compilation still needs no rebuild.

**Tech Stack:** Rust, `serde_json::Value`, `std::sync::LazyLock`, `anyhow`.

**Spec:** `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md` (`d2fad9fa5c012291`) — read § *Design 4* (the selector grammar), § *Design 5* (ledger keys), and Gates 4 and 5 before Task 1.

## Why this phase, and why now

Phase 2 was to be sequenced **after** Phase 3, on the belief that `triggered` routing had an empty population. `A-34` (2026-08-28, n=35/arm) refuted that: the population is not empty, it is **resident**.

| arm | excl-t2 (n=10) | all-plausibility (n=15) | wrong+unchecked |
|---|---:|---:|---:|
| `s2` — the block delivered alone | **7/10** | 12/15 | 0/35 |
| `s5` — the real profile | 4/10 | 6/15 | 1/35 |
| `s6` — block moved to the top | 5/10 | 7/15 | 3/35 |
| `s7` — `OP-2`/`OP-3` sections removed | **7/10** | 11/15 | 1/35 |

`s7` lands exactly on `s2`'s ceiling with the block still at the END of the file. So the mechanism is **instruction competition**, not position — and the remedy is not available to the compiler. Moving the block buys ~1 run; removing the two competing imperative sections buys all 3. **Routing is the measured fix for a 3-of-7 (~43%) loss in `OP-1`'s deployed effect.**

The retained `## Three Claude Code Instances` section (`OP-4`) cost nothing, so the effect is neither bulk nor triggered-ness — it is specifically competing imperatives resident in the same file.

## Global Constraints

- **Branch:** all work on `experiments`. `master` is protected; never commit to it.
- **Gate before completing any task — all four, every task:** `cargo fmt`; `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`; `cargo test`; `cargo check --no-default-features`. The long clippy form is the gate, not garnish — bare `cargo clippy -- -D warnings` lints only the root package's non-test targets with default features and passes trees CI fails. The lean `--no-default-features` check catches an unconditional module reaching a `#[cfg(feature = "librarian")]` one, which otherwise fails only in CI's slow 3-OS `no-features` test-matrix lane.
- **Test env isolation is option A, mandatory:** `docs/conventions/test-env-isolation.md`. Resolve env at the edge into a plain struct, pass it inward. **Never** `EnvGuard`, **never** `#[serial_test::serial]`.
- **Error style:** `src/operator_rules/**` is CLI-and-startup code — use `anyhow::bail!` / `anyhow::Context`. `RecoverableError` is for agent-facing tool responses and does not apply here (`get_guide("error-handling")`).
- **Two types are both named `Shape`.** `operator_rules::rule::Shape` is the *rule* shape (`imperative`/`guard`/`procedure`/`contract`, a measured field). `prompts::guide_index::Shape` is the *selector* (`{tool, action, path_contains}`). Always import the latter as `Selector`. Confusing them compiles in some positions and is the single most likely defect in this plan.
- **`always` rules are never stamped and never routed.** They are resident by construction; a ledger entry would assert a per-session delivery event that did not occur (spec § 5).
- **Phase 2 excludes:** harvest of further rules (Phase 3); cross-machine sync (`CM-10`); fixing `OP-4`'s `path~` predicate (Task 4 pins the gap and files it — see that task for why the fix is a convention change, not a bug fix).

---

## File Structure

| File | Responsibility |
|---|---|
| `src/operator_rules/rule.rs` *(modify)* | `Rule.serves` becomes `Vec<Selector>`, parsed in `finish()`. Gate 4 becomes structural: an unparseable selector cannot produce a `Rule`. |
| `src/operator_rules/corpus.rs` *(create)* | `include_str!` the ledger; `OPERATOR_RULES: LazyLock<Vec<Rule>>`; the build-time gate that the shipped ledger parses, validates and fits the budget. |
| `src/operator_rules/route.rs` *(create)* | `route(sel, result) -> Vec<&'static Rule>` and `ledger_key(id)`. Gate 5 lives here as a test. |
| `src/operator_rules/mod.rs` *(modify)* | Declare the two new modules. |
| `src/tools/core/types.rs` *(modify)* | The delivery hook inside `call_content`. |

Nothing in `validate.rs`, `budget.rs`, `render.rs` or `profiles.rs` changes. `validate`'s two `r.serves.is_empty()` checks are type-agnostic and keep working.

---

### Task 1: `**Serves:**` parses into selectors — Gate 4

Gate 4: *"Every `**Serves:**` parses under the § 4 grammar; an unparseable selector fails the gate rather than silently never matching."*

Today `Rule.serves` is `Vec<String>` and nothing ever parses it, so a typo like `edit_file(path~` is accepted and produces a rule that can never fire — a silent-absence failure. Making the field `Vec<Selector>` moves the gate from "a check someone remembers to run" to "a state that cannot be constructed".

**Files:**
- Modify: `src/operator_rules/rule.rs:86-101` (the `Rule` struct), `:217-261` (`finish`)
- Test: `src/operator_rules/rule.rs` `mod tests`

**Interfaces:**
- Consumes: `crate::prompts::guide_index::{parse_shape, Shape}` — `parse_shape(&str) -> Result<Shape, String>`; `Shape { tool: String, action: Option<String>, path_contains: Option<String> }`
- Produces: `Rule.serves: Vec<Selector>` where `type Selector = crate::prompts::guide_index::Shape`. Tasks 3 and 5 rely on this exact field name and type.

- [ ] **Step 1: Confirm nothing else reads `Rule.serves` as strings**

Run: `references(symbol="Rule/serves", path="src/operator_rules/rule.rs")`

Expected: only `finish` (constructing it) and `validate.rs`'s two `r.serves.is_empty()` checks. `is_empty()` is type-agnostic, so no change is needed there. If anything else appears, stop and report it — the plan assumed two call sites.

- [ ] **Step 2: Write the failing tests**

Add to `src/operator_rules/rule.rs`'s `mod tests`:

```rust
const TRIGGERED_LEDGER: &str = "\
# Operator Rules (OP-N)

## OP-9 — a triggered rule for the selector tests

**Imperative:** Never dispatch an implementer subagent on Haiku.
**Binding:** triggered
**Shape:** imperative
**Covers:** underpowered-subagent-dispatch
**Serves:** Agent, memory.write, edit_file(path~/.claude)
**Evidence:** unmeasured
**Status:** active
";

#[test]
fn serves_parses_into_selectors() {
    let rules = parse_ledger(TRIGGERED_LEDGER).unwrap();
    let r = &rules[0];
    assert_eq!(r.serves.len(), 3, "all three selectors parse; got {:?}", r.serves);
    assert_eq!(r.serves[0].tool, "Agent");
    assert_eq!(r.serves[0].action, None);
    assert_eq!(r.serves[1].tool, "memory");
    assert_eq!(r.serves[1].action.as_deref(), Some("write"));
    assert_eq!(r.serves[2].path_contains.as_deref(), Some("/.claude"));
}

#[test]
fn an_unparseable_selector_fails_naming_the_rule_and_the_defect() {
    let src = TRIGGERED_LEDGER.replace("edit_file(path~/.claude)", "edit_file(path~/.claude");
    let err = parse_ledger(&src).unwrap_err().to_string();
    assert!(err.contains("OP-9"), "the error must name the rule; got {err}");
    assert!(
        err.contains("unterminated predicate"),
        "the error must name the defect, not just fail; got {err}"
    );
}

#[test]
fn a_selector_with_a_malformed_tool_name_is_refused() {
    // `is_ident` rejects a tool name with a space. Left unchecked this would be
    // a rule that parses and never matches anything, forever.
    let src = TRIGGERED_LEDGER.replace("**Serves:** Agent,", "**Serves:** not a tool,");
    let err = parse_ledger(&src).unwrap_err().to_string();
    assert!(err.contains("OP-9"), "the error must name the rule; got {err}");
    assert!(err.contains("malformed tool"), "got {err}");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --lib operator_rules::rule::tests`

Expected: FAIL — `serves_parses_into_selectors` fails to compile (`r.serves[0].tool` — no field `tool` on `String`), and the two error tests fail because `parse_ledger` currently returns `Ok`.

- [ ] **Step 4: Change the field type and parse in `finish`**

In `src/operator_rules/rule.rs`, add near the top imports:

```rust
/// The selector type, borrowed wholesale from the section-grain matcher.
///
/// Aliased because this module already has a `Shape` — the RULE shape
/// (`imperative`/`guard`/…), a measured field with nothing to do with call
/// matching. Importing `guide_index::Shape` unaliased here shadows it in some
/// positions and still compiles.
pub use crate::prompts::guide_index::Shape as Selector;
```

Change the `Rule` field (`:97`):

```rust
    /// Parsed selectors. `always` rules carry none — `validate` enforces that.
    pub serves: Vec<Selector>,
```

`Draft.serves` stays `Vec<String>` — it holds the raw lines. In `finish`, replace `serves: d.serves,` with a parse. Build it above the `Ok(Rule { .. })` literal:

```rust
    let mut serves = Vec::with_capacity(d.serves.len());
    for raw in &d.serves {
        // Gate 4: an unparseable selector is refused here rather than
        // silently never matching. `parse_shape` returns a String error that
        // already names the defect; prefix it with the rule id so the operator
        // learns WHICH rule to fix — the diagnostic failure `OP-5`'s evidence
        // line hit (`invalid float literal`, naming neither rule nor field).
        let sel = parse_shape(raw)
            .map_err(|e| anyhow::anyhow!("{}: **Serves:** {e}", d.id))?;
        serves.push(sel);
    }
```

and use `serves,` in the struct literal. Add `parse_shape` to the import:

```rust
use crate::prompts::guide_index::parse_shape;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --lib operator_rules`

Expected: PASS, all of `operator_rules`' tests including the pre-existing ledger tests.

- [ ] **Step 6: Run the full gate**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test
cargo check --no-default-features
```

Expected: all four clean. `cargo check --no-default-features` matters here specifically — `src/operator_rules` is unconditional and now reaches `src/prompts/guide_index`; if that module is feature-gated, this is where you find out.

- [ ] **Step 7: Commit**

```bash
git add src/operator_rules/rule.rs
git commit -m "feat(operator-rules): Gate 4 — **Serves:** parses into selectors at ledger-parse time

An unparseable selector produced a rule that could never fire, with nothing
reporting it. Making Rule.serves a Vec<Selector> moves the gate from a check
someone runs to a state that cannot be constructed."
```

---

### Task 2: Compile the ledger in, and gate it at build time

Routing has to work in **every** project the server is activated against. The ledger lives at `docs/trackers/operator-rules.md` in the codescout repo, so a runtime disk read would find it only when codescout happens to be the active project — silently delivering nothing everywhere else. The guide corpus solved this with `include_str!`; do the same.

The tradeoff, stated rather than discovered later: **a ledger edit changes routing only after a rebuild.** Profile compilation is unaffected — `compile`/`check` still read from disk via `LEDGER_PATH`, so the operator can edit a rule and recompile profiles without touching cargo. Only routing is pinned to build time, and that is the same bargain every guide topic already makes.

**Files:**
- Create: `src/operator_rules/corpus.rs`
- Modify: `src/operator_rules/mod.rs:9-13` (module declarations)

**Interfaces:**
- Consumes: `rule::parse_ledger(&str) -> Result<Vec<Rule>>`, `validate::validate(&[Rule]) -> Result<()>`, `budget::check_budget(&[Rule]) -> Result<()>`
- Produces: `corpus::OPERATOR_RULES: LazyLock<Vec<Rule>>` and `corpus::LEDGER_SRC: &'static str`. Task 3 reads both.

- [ ] **Step 1: Write the failing test**

Create `src/operator_rules/corpus.rs` containing only the test module and the two items it needs, so the test drives the shape:

```rust
//! The operator-rules ledger, compiled in.

use super::rule::{parse_ledger, Rule};
use std::sync::LazyLock;

/// The shipped ledger source.
///
/// `include_str!` rather than a runtime read: routing must work in every
/// project the server activates against, and this file exists only in the
/// codescout checkout. A disk read would deliver nothing, everywhere else,
/// with no error — the same silent-absence failure Gate 4 exists to rule out.
///
/// `compile`/`check` still read from disk (`super::LEDGER_PATH`), so editing a
/// rule and recompiling profiles needs no rebuild. Only ROUTING is pinned to
/// build time.
pub const LEDGER_SRC: &str = include_str!("../../docs/trackers/operator-rules.md");

/// Every rule in the shipped ledger, parsed once.
///
/// Panics on a malformed ledger. That is correct and deliberate: the test
/// below runs in the same build, so a ledger that would panic here fails
/// `cargo test` before any binary ships.
pub static OPERATOR_RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    parse_ledger(LEDGER_SRC).expect("the compiled-in operator-rules ledger must parse")
});

#[cfg(test)]
mod tests {
    use super::*;

    /// Gates 4 and 6 against the SHIPPED ledger, at build time.
    ///
    /// `compile`/`check` run these against whatever is on disk when a human
    /// invokes them. This runs them against the bytes actually compiled into
    /// the binary, which is what routing reads.
    #[test]
    fn the_shipped_ledger_parses_validates_and_fits_the_budget() {
        let rules = parse_ledger(LEDGER_SRC).expect("shipped ledger must parse");
        super::super::validate::validate(&rules).expect("shipped ledger must validate");
        super::super::budget::check_budget(&rules).expect("shipped ledger must fit the budget");
        assert!(
            rules.iter().any(|r| r.id == "OP-1"),
            "the ledger lost OP-1 — either the file moved or include_str! is pointing \
             at the wrong path; got ids {:?}",
            rules.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
    }

    /// The lazy static must agree with a fresh parse. If `LazyLock` were ever
    /// pointed at a different source this is what catches it.
    #[test]
    fn the_lazy_corpus_matches_a_fresh_parse() {
        let fresh = parse_ledger(LEDGER_SRC).unwrap();
        assert_eq!(OPERATOR_RULES.len(), fresh.len());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib operator_rules::corpus`

Expected: FAIL — `error[E0583]: file not found for module 'corpus'`, because `mod corpus;` is not declared yet.

- [ ] **Step 3: Declare the module**

In `src/operator_rules/mod.rs`, add to the module list at `:9-13`, keeping alphabetical order:

```rust
mod budget;
pub mod corpus;
mod profiles;
mod render;
mod rule;
mod validate;
```

`corpus` is `pub` because Task 3's `route` and Task 5's delivery both read `OPERATOR_RULES`. Note `rule` is currently private; if `Rule` does not resolve from outside the module after this change, make `rule` `pub` too rather than re-exporting piecemeal.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib operator_rules::corpus -- --nocapture`

Expected: PASS, both tests. If `the_shipped_ledger_parses_validates_and_fits_the_budget` fails, the ledger on disk is genuinely broken — fix the ledger, not the test.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test
cargo check --no-default-features
```

- [ ] **Step 6: Commit**

```bash
git add src/operator_rules/corpus.rs src/operator_rules/mod.rs
git commit -m "feat(operator-rules): compile the ledger in, and gate it at build time

Routing must work in every activated project; the ledger file exists only in
this checkout. include_str! matches the guide corpus. compile/check keep
reading from disk, so profile compilation still needs no rebuild."
```

---

### Task 3: `route()` and the `op:` key namespace — Gate 5

Gate 5: *"No `op:` ledger key can collide with a guide topic or section key, asserted directly rather than by naming convention."* Guide keys are `<topic>` and `<topic>#<heading>`; operator keys are `op:<id>`. The gate asserts it rather than trusting that no topic will ever be called `op:OP-1`.

**Files:**
- Create: `src/operator_rules/route.rs`
- Modify: `src/operator_rules/mod.rs` (declare the module)

**Interfaces:**
- Consumes: `corpus::OPERATOR_RULES`; `rule::{Binding, Rule, Status}`; `Selector::matches(&self, sel: Option<&str>, result: &Value) -> bool`
- Produces: `route::route(sel: Option<&str>, result: &Value) -> Vec<&'static Rule>` and `route::ledger_key(id: &str) -> String`. Task 5 calls both.

- [ ] **Step 1: Write the failing tests**

Create `src/operator_rules/route.rs`:

```rust
//! Selecting the `triggered` rules a call should receive.
//!
//! A second corpus fed to the section-grain matcher — not a second matcher.

use super::corpus::OPERATOR_RULES;
use super::rule::{Binding, Rule, Status};
use serde_json::Value;

/// Ledger key for a delivered `triggered` rule.
///
/// Spec § 5. `GuideLedger` stores opaque `String` keys, so a third namespace
/// needs no on-disk format change. Guide keys are `<topic>` and
/// `<topic>#<heading>`; the `op:` prefix keeps this disjoint from both, and
/// `op_keys_collide_with_no_guide_key` asserts it rather than trusting it.
pub fn ledger_key(id: &str) -> String {
    format!("op:{id}")
}

/// The `triggered`, `active` rules whose selector matches this call.
///
/// `always` rules are excluded unconditionally: they are resident in the
/// profile by construction, so routing one would deliver it twice and stamping
/// one would assert a per-session delivery event that did not occur.
///
/// `retired` rules are excluded on the same predicate `render_block` and
/// `check_budget` use, so a retirement takes effect on every path at once.
pub fn route(sel: Option<&str>, result: &Value) -> Vec<&'static Rule> {
    OPERATOR_RULES
        .iter()
        .filter(|r| r.binding == Binding::Triggered && r.status == Status::Active)
        .filter(|r| r.serves.iter().any(|s| s.matches(sel, result)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_matching_selector_routes_its_rule() {
        // OP-3 declares `**Serves:** memory.write`.
        let hit = route(Some("memory.write"), &json!({"status": "ok"}));
        assert!(
            hit.iter().any(|r| r.id == "OP-3"),
            "memory.write must route OP-3; got {:?}",
            hit.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_non_matching_selector_routes_nothing() {
        let hit = route(Some("grep"), &json!({"status": "ok"}));
        assert!(hit.is_empty(), "grep serves no rule; got {:?}",
            hit.iter().map(|r| &r.id).collect::<Vec<_>>());
    }

    #[test]
    fn a_tool_that_opted_out_of_selector_key_routes_nothing() {
        // `Shape::matches` treats `None` as "cannot match" deliberately. A
        // wildcard here would deliver every triggered rule on every call from
        // every tool that has not opted in — the opposite of just-in-time.
        assert!(route(None, &json!({"status": "ok"})).is_empty());
    }

    #[test]
    fn always_rules_are_never_routed() {
        // OP-1 is `always`. It is resident in CLAUDE.md; routing it would
        // deliver it twice and contradict spec § 5.
        for sel in ["memory.write", "Agent", "Task", "edit_file", "artifact.update"] {
            let hit = route(Some(sel), &json!({"abs_path": "/home/u/.claude/CLAUDE.md"}));
            assert!(
                !hit.iter().any(|r| r.id == "OP-1"),
                "OP-1 is `always` and must never route; fired on {sel}"
            );
        }
    }

    #[test]
    fn retired_rules_are_never_routed() {
        assert!(
            OPERATOR_RULES.iter().any(|r| r.status == Status::Retired),
            "this test is vacuous unless the ledger has at least one retired rule — \
             OP-5 was retired 2026-08-28; if it is gone, retire another or delete this"
        );
        for r in OPERATOR_RULES.iter().filter(|r| r.status == Status::Retired) {
            for s in &r.serves {
                let sel = match &s.action {
                    Some(a) => format!("{}.{}", s.tool, a),
                    None => s.tool.clone(),
                };
                let hit = route(Some(&sel), &json!({"abs_path": "/home/u/.claude/x.md"}));
                assert!(
                    !hit.iter().any(|h| h.id == r.id),
                    "retired rule {} routed on its own selector {sel}", r.id
                );
            }
        }
    }

    /// Gate 5, asserted directly.
    #[test]
    fn op_keys_collide_with_no_guide_key() {
        use crate::prompts::guide_index::GUIDE_INDEX;
        let op_keys: Vec<String> = OPERATOR_RULES.iter().map(|r| ledger_key(&r.id)).collect();
        assert!(!op_keys.is_empty(), "no rules — the corpus failed to load");
        for (topic, entry) in &GUIDE_INDEX.topics {
            assert!(
                !op_keys.iter().any(|k| k == topic),
                "an op: key collides with guide topic {topic}"
            );
            for sec in &entry.sections {
                let sk = sec.ledger_key();
                assert!(
                    !op_keys.iter().any(|k| *k == sk),
                    "an op: key collides with guide section key {sk}"
                );
            }
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib operator_rules::route`

Expected: FAIL — `error[E0583]: file not found for module 'route'`.

- [ ] **Step 3: Declare the module**

In `src/operator_rules/mod.rs`:

```rust
pub mod route;
```

If `Binding`, `Rule` or `Status` do not resolve, make `mod rule;` into `pub mod rule;` — `route` is a sibling, so the fields are already reachable; only the module's own visibility can block it.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib operator_rules::route -- --nocapture`

Expected: PASS, all six. If `a_matching_selector_routes_its_rule` fails, read the ledger's `OP-3` entry — its `**Serves:**` line is `memory.write` and the test is pinned to that exact string.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test
cargo check --no-default-features
```

- [ ] **Step 6: Commit**

```bash
git add src/operator_rules/route.rs src/operator_rules/mod.rs
git commit -m "feat(operator-rules): route triggered rules, and assert Gate 5 directly

always and retired rules are excluded on the same predicate render_block and
check_budget use, so a retirement takes effect on every path at once."
```

---

### Task 4: Pin `OP-4`'s dead predicate, and file it

`OP-4` declares `**Serves:** edit_file(path~/.claude), create_file(path~/.claude)`. Its own ledger entry flagged this as *"the selector most likely to need work"*, and it is now confirmed dead:

- `Selector::matches` delegates `path~` to `names_path_containing` (`src/prompts/guide_index.rs:194`).
- `names_path_containing` (`src/util/librarian_response.rs:36-65`) scans exactly four shapes: top-level `abs_path`, top-level `rel_path`, `items[].abs_path|rel_path`, and `violations[].path`.
- `edit_file` answers with the project's **no-echo write convention** — a bare `"ok"`. `annotate_write_root` (`src/tools/core/types.rs:185-201`) promotes that to `{"status":"ok","wrote_to":<checkout root>}`, and only when the repo has linked worktrees. `wrote_to` is the **checkout**, not the file.
- Observed live 2026-08-28: `edit_file` on `/home/marius/.claude/CLAUDE.md` returned `{"status": "ok", "wrote_to": "/home/marius/work/claude/codescout"}`.

No shape carries the written path, so the predicate matches nothing, ever.

**The fix is deliberately out of scope**, and the reason is not laziness: giving write responses an `abs_path` changes the no-echo write convention (`memory: conventions`), which is a project-wide decision with its own tradeoffs, not a bug fix riding along inside a routing plan. `names_path_containing`'s own doc explicitly declined to widen the top-level scan to serve one action. So this task pins the current behaviour with an escape hatch and files the gap.

`OP-2` and `OP-3` carry no predicate, so the measured `A-34` win is unaffected.

**Files:**
- Modify: `src/operator_rules/route.rs` (`mod tests`)
- Create: `docs/issues/2026-08-28-op-4-path-predicate-can-never-fire.md`

**Interfaces:**
- Consumes: `route::route` from Task 3. Produces nothing new.

- [ ] **Step 1: Write the characterization test**

Append to `src/operator_rules/route.rs`'s `mod tests`:

```rust
/// `OP-4`'s `path~` predicate cannot fire against a real write response.
///
/// Pinned rather than fixed: write tools answer with the no-echo `"ok"`
/// convention, and `names_path_containing` scans only `abs_path`/`rel_path`
/// (top level and `items[]`) plus `violations[].path`. Giving writes a path
/// field is a change to the no-echo convention, not a bug fix — see
/// docs/issues/2026-08-28-op-4-path-predicate-can-never-fire.md
///
/// **When this test starts failing, that is the fix landing.** Delete it and
/// assert delivery instead; close the bug file.
#[test]
fn op_4s_path_predicate_cannot_fire_against_a_write_response_today() {
    let observed = json!({"status": "ok", "wrote_to": "/home/u/work/claude/codescout"});
    let hit = route(Some("edit_file"), &observed);
    assert!(
        !hit.iter().any(|r| r.id == "OP-4"),
        "OP-4 fired — the write-response shape gained a path field. This is the \
         GOOD failure: delete this test, assert delivery, and close the bug file."
    );
}

/// The same rule DOES fire once a response names the path — so the defect is
/// the response shape, not the selector or the matcher.
///
/// Without this cell the test above is indistinguishable from "OP-4's selector
/// is malformed", which is a different bug with a different fix.
#[test]
fn op_4s_predicate_is_itself_sound_given_a_path_bearing_response() {
    let hit = route(Some("edit_file"), &json!({"abs_path": "/home/u/.claude/CLAUDE.md"}));
    assert!(
        hit.iter().any(|r| r.id == "OP-4"),
        "OP-4's selector is broken independently of the response shape; got {:?}",
        hit.iter().map(|r| &r.id).collect::<Vec<_>>()
    );
}
```

- [ ] **Step 2: Run the tests to verify they pass**

Run: `cargo test --lib operator_rules::route::tests::op_4 -- --nocapture`

Expected: PASS, both. The second is the positive control that keeps the first meaningful — a pair that both pass means "the selector is sound and the response shape is the gap", which is a claim; the first alone would only be an absence.

- [ ] **Step 3: File the bug**

Copy `docs/issues/_TEMPLATE.md` to `docs/issues/2026-08-28-op-4-path-predicate-can-never-fire.md` with `status: open` and a body carrying: the four scanned shapes, the observed `edit_file` response above, the two test names that pin it, and the statement that the remedy is a no-echo-convention decision (add `abs_path` to write responses) rather than a change to `names_path_containing`, whose doc declined to widen the top-level scan for exactly this kind of single-caller need.

- [ ] **Step 4: Run the full gate**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test
cargo check --no-default-features
```

- [ ] **Step 5: Commit**

```bash
git add src/operator_rules/route.rs docs/issues/2026-08-28-op-4-path-predicate-can-never-fire.md
git commit -m "test(operator-rules): pin OP-4's dead path~ predicate, with a positive control

The selector is sound; the write-response shape carries no path. Fixing that
is a no-echo-convention decision, not a drive-by. The paired test says which
of the two is broken, which one test alone could not."
```

---

### Task 5: Deliver routed rules in `call_content`

**Files:**
- Modify: `src/tools/core/types.rs:1019-1120` (after the guide-block computation, before `let primary = …`) and `:1192-1195` (the assembly)

**Interfaces:**
- Consumes: `route::route`, `route::ledger_key`; `ctx.guide_hints_emitted` (a `Mutex<GuideLedger>`); `GuideLedger::contains(&str) -> bool`, `GuideLedger::insert(String) -> bool`
- Produces: operator-rule `Content` blocks appended after the guide blocks.

- [ ] **Step 1: Write the failing test**

Add to `src/tools/core/tests.rs`:

```rust
/// A routed `triggered` rule is delivered once per session, on the calls whose
/// shape it serves — and not on others.
#[tokio::test]
async fn a_triggered_operator_rule_is_delivered_once_for_its_call_shape() {
    let ctx = test_ctx();

    // OP-3 serves `memory.write`.
    let first = call_content_for_test(&ctx, "memory", serde_json::json!({"action": "write"})).await;
    let joined = first.iter().filter_map(text_of).collect::<String>();
    assert!(
        joined.contains("operator-rule OP-3"),
        "OP-3 must be delivered on memory.write; got {joined}"
    );

    // Same shape again in the same session: already stamped, so silent.
    let second = call_content_for_test(&ctx, "memory", serde_json::json!({"action": "write"})).await;
    let joined2 = second.iter().filter_map(text_of).collect::<String>();
    assert!(
        !joined2.contains("operator-rule OP-3"),
        "a second call of the same shape must not re-deliver; got {joined2}"
    );
}

/// `always` rules never reach this path — they are resident in CLAUDE.md.
#[tokio::test]
async fn an_always_rule_is_never_delivered_by_the_router() {
    let ctx = test_ctx();
    let out = call_content_for_test(&ctx, "memory", serde_json::json!({"action": "write"})).await;
    let joined = out.iter().filter_map(text_of).collect::<String>();
    assert!(
        !joined.contains("operator-rule OP-1"),
        "OP-1 is `always` and resident; delivering it here doubles it. Got {joined}"
    );
}
```

Reuse whatever `test_ctx` / `call_content_for_test` / `text_of` helpers `src/tools/core/tests.rs` already has — read the module's existing tests first (e.g. around `:1313`, which builds a `ToolContext` with `GuideLedger::mid_session()`), and match their names exactly rather than introducing new ones.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib tools::core::tests::a_triggered_operator_rule -- --nocapture`

Expected: FAIL — no `operator-rule OP-3` text in the output, because nothing delivers it yet.

- [ ] **Step 3: Add the delivery block**

In `src/tools/core/types.rs`, immediately after the `let (guide_hint, guide_content) = { … };` block closes (around `:1120`) and **before** `// Build the primary response block`:

```rust
        // --- Operator rules: `triggered`-rule delivery (Phase 2) -----------
        //
        // Independent of the guide path above — a call may receive both. The
        // two stamp disjoint key namespaces (`op:OP-N` vs `<topic>` /
        // `<topic>#<heading>`), asserted by Gate 5 in
        // `operator_rules::route::tests::op_keys_collide_with_no_guide_key`.
        //
        // Computed here, while `&val` is still borrowable: the small-output
        // branch below moves `val`.
        //
        // `always` rules are excluded inside `route`, not here — a resident
        // rule delivered on a call would arrive twice, and stamping it would
        // assert a per-session delivery event that never happened (spec § 5).
        let op_content: Vec<Content> = {
            let mut emitted = ctx.guide_hints_emitted.lock();
            let mut out = Vec::new();
            for r in crate::operator_rules::route::route(selector.as_deref(), &val) {
                let key = crate::operator_rules::route::ledger_key(&r.id);
                // `contains` then `insert` rather than relying on `insert`'s
                // return value: a repeat insert REFRESHES the stamp (see
                // `a_repeat_insert_refreshes_the_stamp_and_persists_it`), so
                // its bool does not mean "was absent".
                if emitted.contains(&key) {
                    continue;
                }
                emitted.insert(key);
                out.push(Content::text(format!(
                    "<!-- operator-rule {} — delivered once this session for this call \
                     shape; see docs/trackers/operator-rules.md -->\n{}",
                    r.id, r.imperative
                )));
            }
            out
        };
```

Then extend the assembly at `:1192-1195`:

```rust
        let mut blocks = vec![primary];
        blocks.extend(guide_content);
        blocks.extend(op_content);
        Ok(blocks)
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib tools::core::tests -- --nocapture`

Expected: PASS, including every pre-existing test in that module. If a guide-delivery test now fails on block **count**, read it before changing it — an operator block is a legitimate new block, and the right fix is usually to make that assertion filter by prefix rather than count blocks.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test
cargo check --no-default-features
```

- [ ] **Step 6: Commit**

```bash
git add src/tools/core/types.rs src/tools/core/tests.rs
git commit -m "feat(operator-rules): deliver triggered rules just-in-time in call_content

Routed rules arrive on the calls whose shape they serve, once per session,
stamped under op: keys disjoint from the guide namespace."
```

---

### Task 6: Retire `OP-2` and `OP-3` from the resident profile

This is the task that banks `A-34`'s measured win. Until the two sections leave the profile, routing has **added** a delivery path without removing the competition — which is strictly worse than before.

The honest cost, from `A-34`: unlike the `Conclude Last` deletion, `OP-2`/`OP-3` have no measured replacement. Routing is that replacement in principle — Task 5 built the mechanism, but per the blocking note below it does not yet reach either rule in production, so the trade is not yet available.

**Files:**
- Modify: `~/.claude/CLAUDE.md`, `~/.claude-sdd/CLAUDE.md`, `~/.claude-kat/CLAUDE.md` (untracked, outside the repo)
- Modify: `docs/trackers/operator-rules.md` (via `artifact(action="update")` — it is a guarded ledger)

> **BLOCKED — do not run this task as written.** Its premise is false. Routing exists but
> reaches neither rule: no production tool outside the `LibrarianAdapter` family overrides
> `Tool::selector_key`, so `memory` (OP-3) produces no selector; and `Agent`/`Task` (OP-2) are
> Claude Code's own tools that never enter this process, so no override can reach them.
> Deleting these sections would leave both rules undelivered everywhere, with the ledger as
> their only copy. See
> `docs/issues/2026-08-28-triggered-operator-rules-route-nothing-in-production.md`.

- [ ] **Step 1: Back up all three profiles**

```bash
mkdir -p /tmp/claude-md-backup-phase2
for p in claude claude-sdd claude-kat; do cp -v /home/marius/.$p/CLAUDE.md /tmp/claude-md-backup-phase2/$p-CLAUDE.md; done
```

- [ ] **Step 2: Confirm the routing actually delivers, live, before deleting anything**

Rebuild and reconnect, then exercise the two shapes:

```bash
cargo rb
```

Then `/mcp` to reconnect, call `memory(action="write", …)` once, and confirm an `operator-rule OP-3` block arrives. **Do not proceed if it does not** — deleting the resident text before delivery is verified leaves the rules undelivered on every path at once, and the profiles are untracked.

- [ ] **Step 3: Delete the two sections from all three profiles**

```
approve_write("/home/marius/.claude")        # and the two sibling profile dirs
edit_markdown("/home/marius/.claude/CLAUDE.md",
  heading="### Subagent Dispatch — Model Floor + Review Escalation", action="remove")
edit_markdown("/home/marius/.claude/CLAUDE.md",
  heading="### Memory — Use Codescout, Not Claude Code Memory", action="remove")
```

Repeat for `~/.claude-sdd/CLAUDE.md` and `~/.claude-kat/CLAUDE.md`. **Retain `## Three Claude Code Instances`** — `A-34` measured it as costing nothing, and it is factual context rather than a competing imperative.

Serialize these calls; do not batch writes in parallel (`BUG-021`).

- [ ] **Step 4: Recompile and verify all three converge**

```bash
codescout operator-rules compile
codescout operator-rules check
for p in claude claude-sdd claude-kat; do printf '%s bytes=%s md5=%s\n' "$p" "$(wc -c < /home/marius/.$p/CLAUDE.md)" "$(md5sum < /home/marius/.$p/CLAUDE.md | cut -c1-8)"; done
```

Expected: `check` prints `all 3 profiles current`, and all three report identical byte counts and md5s. They were uniform at 3845 B / `9b554ef6` before this task.

- [ ] **Step 5: Record the transition in the ledger**

`OP-2` and `OP-3` keep `**Status:** active` — they are still live rules, now delivered by routing rather than residency. Add a note to each entry's body recording that Phase 2 shipped, the date, and the `A-34` figures that motivated it:

```
artifact(action="update", id="fa21bfb35684794d", patch={body_edits: [
  {heading: "## OP-2 — Sonnet is the subagent-dispatch floor", action: "edit",
   old_string: "**Status:** active", new_string: "**Status:** active\n**Delivered-by:** routing since 2026-08-28 (Phase 2); removed from the resident profiles the same day on `A-34`"}]})
```

Do the same for `OP-3`. Do **not** hand-edit the file — it is a guarded ledger.

- [ ] **Step 6: Commit**

```bash
git add docs/trackers/operator-rules.md
git commit -m "feat(operator-rules): retire OP-2/OP-3 from the resident profiles

A-34 measured the competing-imperative cost at 3 of 7 (~43%) of OP-1's effect,
with s7 landing exactly on s2's ceiling once these two sections were removed.
Routing (Phase 2) is the replacement that made the trade available."
```

- [ ] **Step 7: Re-run `s5`/`s7` to confirm the deployed profile moved**

The profile is now materially `s7`. Re-run those arms in `prompt-engineering` per the `conclude-last` README, and record the outcome as a new `A-N` pre-registration in `docs/trackers/prompt-hamsa-audit-log.md`. Read `prompt-engineering:docs/trackers/prompt-tdd-operating-guide.md` first — in particular `OP-22`: this suite's checkers are inline `python:`, so `run_arms.py` silently skips re-scoring. Use the suite's own `analyze.py`.

Note the suite's `t2-cat-gate` trap is still stale (codescout `be4a679b` inverted its expected answer), so quote the excl-t2 cut.

---

## Self-Review

**Spec coverage.** § 4 (selector grammar, matcher reuse) → Tasks 1, 3, 5. § 5 (ledger keys, `always` never stamped) → Task 3 (`ledger_key`, the `always`/`retired` filters) and Task 5 (stamping). Gate 4 → Task 1. Gate 5 → Task 3. Gates 1, 2, 3, 6 are Phase 1 and unchanged; Task 2 additionally runs Gates 4 and 6 against the *compiled-in* bytes, which Phase 1 only ran against disk. § *Rollout* Phase 2's "do not build a second matcher" → satisfied by importing `parse_shape` and `Shape::matches` rather than reimplementing them.

**Known gap, deliberate.** `OP-4`'s `path~` predicate cannot fire; Task 4 pins it with a positive control and files it. Fixing it is a no-echo-write-convention decision. `OP-2`/`OP-3` carry no predicate, so the `A-34` win is unaffected.

**Type consistency.** `Selector` = `crate::prompts::guide_index::Shape` throughout (Tasks 1, 3). `Rule.serves: Vec<Selector>` defined in Task 1, read in Task 3. `route(sel: Option<&str>, result: &Value) -> Vec<&'static Rule>` and `ledger_key(id: &str) -> String` defined in Task 3, called in Tasks 4 and 5. `GuideLedger::contains(&str) -> bool` / `insert(String) -> bool` used in Task 5 with the `contains`-then-`insert` idiom, because `insert`'s bool does not mean "was absent".

**Ordering.** Task 6 is last and gated on live verification in its own Step 2, because it deletes text from untracked files whose only other copy is the ledger.

