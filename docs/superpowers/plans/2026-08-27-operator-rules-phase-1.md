---
kind: plan
status: active
title: Operator Rules Engine — Phase 1 Implementation Plan
tags:
  - operator-rules
  - prompt-surface
  - engine-5
  - phase-1
---

# Operator Rules Engine — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make operator rules addressable and their three-profile synchronisation
mechanically verifiable, by compiling `always`-bound rules from one git-tracked ledger
into a delimited block in each `~/.claude*/CLAUDE.md`.

**Architecture:** A new `src/operator_rules/` module with five single-responsibility
files: parse the ledger into `Rule` values, validate them, check the two budget
constraints, render the `always` subset to a deterministic block, and splice that block
into each profile idempotently. A `codescout operator-rules` CLI subcommand drives it in
either `compile` or `check` mode. No MCP tool, no server changes, no dependency on the
`sdd/get-guide-section-grain` branch.

**Tech Stack:** Rust 2024, `clap` (derive, already in `src/main.rs`), `anyhow`, `dirs`,
`tempfile` for tests. Reuses `crate::util::markdown_fence::FenceState` and
`crate::librarian::frontmatter::parse`.

**Spec:** `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md` (artifact
`d2fad9fa5c012291`, committed `aa0bc5c1` + `7da4c0ed`). Executors read both.

## Global Constraints

- **Branch:** all work on `experiments`. `master` is protected; never commit to it.
- **Gate before completing any task:** `cargo fmt`, `cargo clippy -- -D warnings`,
  `cargo test`. All three, every task.
- **Test env isolation is option A, mandatory:** `docs/conventions/test-env-isolation.md`.
  Resolve env at the edge into a plain struct, pass it inward. **Never** `EnvGuard`,
  **never** `#[serial_test::serial]` — option B is documented as NOT VIABLE and was removed
  project-wide in `a656f8cec220d347` (119 → 0 `set_var` occurrences). A test that read
  `$HOME` here would also overwrite the operator's real `~/.claude/CLAUDE.md`, so this rule
  is simultaneously the correctness rule and the safety rule.
- **Error style:** this is a CLI, not an MCP tool surface. Use `anyhow::bail!` /
  `anyhow::Context`. `RecoverableError` is for agent-facing tool responses and does not
  apply here (`get_guide("error-handling")`).
- **Entry-heading grammar is fixed:** `link_scan`'s `def_re` is
  `^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+`. A heading without the dash-and-title defines no token.
  Match it exactly; do not invent a variant.
- **Fence-skipping is mandatory** wherever markdown is scanned. The spec records that the
  first three section measurements taken while drafting it were wrong because `^## `
  matched inside a fence.
- **Phase 1 excludes:** `triggered`-rule routing (Phase 2, blocked on `Tool::selector_key`
  landing), harvest of the remaining three rules (Phase 3), cross-machine sync, engines
  1–4 and 6.

---

## File Structure

| File | Responsibility |
|---|---|
| `src/operator_rules/mod.rs` | Module root. Public API: `compile`, `check`, `exit_code`. Orchestration only. |
| `src/operator_rules/rule.rs` | `Rule`, `Binding`, `Shape`, `Evidence`, `Status` + `parse_ledger`. Markdown → values. |
| `src/operator_rules/validate.rs` | Gate 6: required fields and binding coherence. |
| `src/operator_rules/budget.rs` | Gate 3: non-overlap (a) and size ceiling (b). |
| `src/operator_rules/render.rs` | `always` rules → block text. Deterministic. |
| `src/operator_rules/profiles.rs` | `OperatorProfiles` (env at the edge), `splice`, `extract_block`. |
| `src/main.rs` | `Commands::OperatorRules` variant + dispatch arm. |
| `docs/trackers/operator-rules.md` | The ledger. Created in Task 8. |

`src/lib.rs` gains `pub mod operator_rules;`.

---

## Task 1: `Rule` and the ledger parser

**Files:**
- Create: `src/operator_rules/rule.rs`
- Create: `src/operator_rules/mod.rs`
- Modify: `src/lib.rs` (add `pub mod operator_rules;`)

**Interfaces:**
- Consumes: `crate::util::markdown_fence::FenceState` (`src/lib.rs:52`, unconditional).
  **Deliberately NOT `librarian::frontmatter::parse`** — that module is behind
  `#[cfg(feature = "librarian")]` (`src/lib.rs:32`), and this parser discards the
  frontmatter anyway, so the coupling would gate the compiler on a feature it does not
  use.
- Produces: `Rule { id: String, title: String, imperative: String, binding: Binding,
  shape: Shape, covers: String, serves: Vec<String>, evidence: Evidence,
  rests_on: Option<String>, status: Status }`; enums `Binding::{Always, Triggered}`,
  `Shape::{Imperative, Guard, Procedure, Contract}`,
  `Evidence::{Measured { arm: String, base: f32, shipped: f32, n: u32 }, Unmeasured}`,
  `Status::{Active, Candidate, Retired}`; and
  `pub fn parse_ledger(doc: &str) -> anyhow::Result<Vec<Rule>>`.

- [ ] **Step 1: Write the failing test**

Create `src/operator_rules/rule.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const LEDGER: &str = r#"---
kind: tracker
status: active
title: Operator rules
entry_prefix: OP
entry_high_water_OP: 2
---

# Operator rules

## OP-1 — Always verify before asserting

**Imperative:** Do not hypothesise — ALWAYS VERIFY.
**Binding:** always
**Shape:** imperative
**Covers:** unverified-assertion
**Evidence:** measured: conclude-last/b2 0% -> 100% (n=35)
**Rests on:** prompt-hamsa-audit-log:A-21
**Status:** active

## OP-2 — Sonnet is the subagent floor

**Imperative:** Never dispatch an implementer or reviewer subagent on Haiku.
**Binding:** triggered
**Shape:** guard
**Covers:** subagent-model-floor
**Serves:** Agent, Task
**Evidence:** unmeasured
**Status:** active
"#;

    #[test]
    fn parse_ledger_reads_both_entries_with_every_field() {
        let rules = parse_ledger(LEDGER).unwrap();
        assert_eq!(rules.len(), 2, "two OP entries: {rules:#?}");

        let r1 = &rules[0];
        assert_eq!(r1.id, "OP-1");
        assert_eq!(r1.title, "Always verify before asserting");
        assert_eq!(r1.imperative, "Do not hypothesise — ALWAYS VERIFY.");
        assert_eq!(r1.binding, Binding::Always);
        assert_eq!(r1.shape, Shape::Imperative);
        assert_eq!(r1.covers, "unverified-assertion");
        assert!(r1.serves.is_empty());
        assert_eq!(r1.status, Status::Active);
        assert_eq!(
            r1.evidence,
            Evidence::Measured {
                arm: "conclude-last/b2".into(),
                base: 0.0,
                shipped: 100.0,
                n: 35
            }
        );
        assert_eq!(r1.rests_on.as_deref(), Some("prompt-hamsa-audit-log:A-21"));

        let r2 = &rules[1];
        assert_eq!(r2.id, "OP-2");
        assert_eq!(r2.binding, Binding::Triggered);
        assert_eq!(r2.serves, vec!["Agent".to_string(), "Task".to_string()]);
        assert_eq!(r2.evidence, Evidence::Unmeasured);
        assert_eq!(r2.rests_on, None);
    }

    /// A worked example inside a fence teaches the syntax; it is not an entry.
    /// The spec records that the first three section measurements taken while
    /// drafting it were wrong for exactly this reason.
    #[test]
    fn a_fenced_example_entry_is_not_parsed() {
        let doc = format!(
            "{LEDGER}\n## How to add an entry\n\n```markdown\n## OP-99 — Not a real rule\n\n\
             **Imperative:** Nope.\n**Binding:** always\n**Shape:** imperative\n\
             **Covers:** nothing\n**Evidence:** unmeasured\n**Status:** active\n```\n"
        );
        let rules = parse_ledger(&doc).unwrap();
        let ids: Vec<&str> = rules.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["OP-1", "OP-2"], "OP-99 is inside a fence");
    }

    /// `def_re` requires the dash-and-title. A bare `## OP-3` defines no token,
    /// so it must not silently become a rule with an empty title.
    #[test]
    fn a_heading_without_a_dash_and_title_is_not_an_entry() {
        let doc = format!("{LEDGER}\n## OP-3\n\n**Imperative:** Orphan.\n");
        let rules = parse_ledger(&doc).unwrap();
        assert_eq!(rules.len(), 2, "OP-3 lacks the dash-and-title shape");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib operator_rules::rule`
Expected: FAIL to compile — `parse_ledger`, `Rule`, `Binding`, `Shape`, `Evidence`,
`Status` are not defined, and `src/operator_rules` is not a module.

- [ ] **Step 3: Write minimal implementation**

Create `src/operator_rules/mod.rs`:

```rust
//! Operator rules — engine 5 of the rules programme.
//!
//! Rules that hold across every project, tool and model for one operator. Rules
//! bound `always` compile into a delimited block in each Claude Code profile's
//! `CLAUDE.md`; rules bound `triggered` are routed at runtime in Phase 2.
//!
//! Spec: `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md`.

pub mod rule;
```

Add to `src/lib.rs`, alongside the other `pub mod` declarations:

```rust
pub mod operator_rules;
```

Prepend to `src/operator_rules/rule.rs`, above the test module:

```rust
use anyhow::{Context, Result, bail};

use crate::util::markdown_fence::FenceState;

/// Strip a leading YAML frontmatter block, returning the body.
///
/// Local rather than `librarian::frontmatter::parse`: that module is behind
/// `#[cfg(feature = "librarian")]` (`src/lib.rs:32`) and this parser discards the
/// frontmatter, so depending on it would gate the compiler on a feature it does
/// not use. An unterminated block is returned whole and harmlessly — a `---` line
/// matches no entry heading.
fn strip_frontmatter(doc: &str) -> &str {
    let Some(rest) = doc
        .strip_prefix("---\n")
        .or_else(|| doc.strip_prefix("---\r\n"))
    else {
        return doc;
    };
    let mut idx = 0usize;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return &rest[idx + line.len()..];
        }
        idx += line.len();
    }
    doc
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    Always,
    Triggered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Imperative,
    Guard,
    Procedure,
    Contract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Active,
    Candidate,
    Retired,
}

/// A rule's measurement status. `Unmeasured` is a first-class value, not an
/// omission: it is what stops a plausible sentence acquiring the authority of a
/// measured one by sitting next to it.
#[derive(Debug, Clone, PartialEq)]
pub enum Evidence {
    Measured {
        arm: String,
        base: f32,
        shipped: f32,
        n: u32,
    },
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub id: String,
    pub title: String,
    pub imperative: String,
    pub binding: Binding,
    pub shape: Shape,
    /// Short kebab-case failure-mode slug. Gate 3(a) compares these: two `always`
    /// rules covering one failure mode is what `A-20`'s dilution finding forbids.
    pub covers: String,
    /// Selector shapes, `triggered` only. Phase 1 stores them verbatim; Phase 2
    /// parses them with the section-grain grammar.
    pub serves: Vec<String>,
    pub evidence: Evidence,
    pub rests_on: Option<String>,
    pub status: Status,
}

/// A partially-collected entry, before field checks turn it into a `Rule`.
#[derive(Default)]
struct Draft {
    id: String,
    title: String,
    imperative: Option<String>,
    binding: Option<String>,
    shape: Option<String>,
    covers: Option<String>,
    serves: Vec<String>,
    evidence: Option<String>,
    rests_on: Option<String>,
    status: Option<String>,
}

/// `link_scan`'s definition shape: `## <ID> — <title>`. A heading missing the
/// dash-and-title defines no token, so it is not an entry here either.
fn entry_heading(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("## ")?;
    let (token, tail) = rest.split_once(char::is_whitespace)?;
    let (prefix, num) = token.split_once('-')?;
    if prefix != "OP" || num.is_empty() || !num.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let title = tail
        .trim_start()
        .strip_prefix(['—', '–', '-'])?
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }
    Some((token.to_string(), title))
}

/// `**Key:** value` at line start. Returns `(key, value)`.
fn field_line(line: &str) -> Option<(&str, &str)> {
    let rest = line.trim_start().strip_prefix("**")?;
    let (key, tail) = rest.split_once(":**")?;
    Some((key.trim(), tail.trim()))
}

/// `measured: <arm> <base>% -> <shipped>% (n=N)`, or `unmeasured`.
fn parse_evidence(raw: &str, id: &str) -> Result<Evidence> {
    if raw.trim() == "unmeasured" {
        return Ok(Evidence::Unmeasured);
    }
    let body = raw
        .trim()
        .strip_prefix("measured:")
        .with_context(|| {
            format!("{id}: **Evidence:** must be `unmeasured` or `measured: …`, got {raw:?}")
        })?
        .trim();
    let (arm, tail) = body
        .split_once(char::is_whitespace)
        .with_context(|| format!("{id}: **Evidence:** measured form needs an arm name"))?;
    let (base_raw, tail) = tail
        .trim()
        .split_once("->")
        .with_context(|| format!("{id}: **Evidence:** needs `<base>% -> <shipped>%`"))?;
    let (shipped_raw, n_raw) = tail
        .trim()
        .split_once("(n=")
        .with_context(|| format!("{id}: **Evidence:** measured form needs `(n=N)`"))?;
    let pct = |s: &str| -> Result<f32> { Ok(s.trim().trim_end_matches('%').trim().parse::<f32>()?) };
    Ok(Evidence::Measured {
        arm: arm.trim().to_string(),
        base: pct(base_raw)?,
        shipped: pct(shipped_raw)?,
        n: n_raw.trim().trim_end_matches(')').trim().parse::<u32>()?,
    })
}

fn finish(d: Draft) -> Result<Rule> {
    let need = |v: Option<String>, f: &str| -> Result<String> {
        v.with_context(|| format!("{}: missing **{f}:**", d.id))
    };
    let binding = match need(d.binding.clone(), "Binding")?.as_str() {
        "always" => Binding::Always,
        "triggered" => Binding::Triggered,
        other => bail!(
            "{}: **Binding:** must be `always` or `triggered`, got {other:?}",
            d.id
        ),
    };
    let shape = match need(d.shape.clone(), "Shape")?.as_str() {
        "imperative" => Shape::Imperative,
        "guard" => Shape::Guard,
        "procedure" => Shape::Procedure,
        "contract" => Shape::Contract,
        other => bail!(
            "{}: **Shape:** must be imperative|guard|procedure|contract, got {other:?}",
            d.id
        ),
    };
    let status = match need(d.status.clone(), "Status")?.as_str() {
        "active" => Status::Active,
        "candidate" => Status::Candidate,
        "retired" => Status::Retired,
        other => bail!(
            "{}: **Status:** must be active|candidate|retired, got {other:?}",
            d.id
        ),
    };
    let evidence = parse_evidence(&need(d.evidence.clone(), "Evidence")?, &d.id)?;
    Ok(Rule {
        title: d.title,
        imperative: need(d.imperative.clone(), "Imperative")?,
        binding,
        shape,
        covers: need(d.covers.clone(), "Covers")?,
        serves: d.serves,
        evidence,
        rests_on: d.rests_on,
        status,
        id: d.id,
    })
}

/// Parse a ledger document into its `OP-N` rules, in document order.
///
/// Fence-aware: a worked example inside a code block teaches the syntax and is
/// not an entry. Frontmatter is stripped first so a `##` inside it cannot match.
pub fn parse_ledger(doc: &str) -> Result<Vec<Rule>> {
    let body = strip_frontmatter(doc);
    let mut fence = FenceState::new();
    let mut out = Vec::new();
    let mut cur: Option<Draft> = None;

    for line in body.lines() {
        if fence.feed(line) || fence.in_fence() {
            continue;
        }
        if let Some((id, title)) = entry_heading(line) {
            if let Some(d) = cur.take() {
                out.push(finish(d)?);
            }
            cur = Some(Draft {
                id,
                title,
                ..Default::default()
            });
            continue;
        }
        let Some(d) = cur.as_mut() else { continue };
        let Some((key, value)) = field_line(line) else {
            continue;
        };
        match key {
            "Imperative" => d.imperative = Some(value.to_string()),
            "Binding" => d.binding = Some(value.to_string()),
            "Shape" => d.shape = Some(value.to_string()),
            "Covers" => d.covers = Some(value.to_string()),
            "Serves" => {
                d.serves = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            "Evidence" => d.evidence = Some(value.to_string()),
            "Rests on" => d.rests_on = Some(value.to_string()),
            "Status" => d.status = Some(value.to_string()),
            _ => {}
        }
    }
    if let Some(d) = cur.take() {
        out.push(finish(d)?);
    }
    Ok(out)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib operator_rules::rule`
Expected: PASS — 3 tests.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/lib.rs src/operator_rules/
git commit -m "feat(operator-rules): fence-aware ledger parser for OP-N entries"
```

---

## Task 2: Field validation — Gate 6

**Files:**
- Create: `src/operator_rules/validate.rs`
- Modify: `src/operator_rules/mod.rs` (add `pub mod validate;`)

**Interfaces:**
- Consumes: `Rule`, `Binding` from Task 1.
- Produces: `pub fn validate(rules: &[Rule]) -> anyhow::Result<()>`.

Task 1's `finish` refuses a *missing* field. This task refuses combinations that parse
individually but are jointly wrong, and it is where Gate 6 becomes a gate rather than a
side effect of parsing.

- [ ] **Step 1: Write the failing test**

Create `src/operator_rules/validate.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator_rules::rule::{Binding, Evidence, Rule, Shape, Status};

    fn rule(id: &str, binding: Binding, serves: &[&str]) -> Rule {
        Rule {
            id: id.into(),
            title: "T".into(),
            imperative: "Do the thing.".into(),
            binding,
            shape: Shape::Imperative,
            covers: format!("mode-{id}"),
            serves: serves.iter().map(|s| s.to_string()).collect(),
            evidence: Evidence::Unmeasured,
            rests_on: None,
            status: Status::Active,
        }
    }

    #[test]
    fn an_always_rule_carrying_serves_is_refused() {
        let rules = vec![rule("OP-1", Binding::Always, &["Agent"])];
        let err = validate(&rules).unwrap_err().to_string();
        assert!(err.contains("OP-1"), "names the rule: {err}");
        assert!(err.contains("Serves"), "names the field: {err}");
    }

    #[test]
    fn a_triggered_rule_without_serves_is_refused() {
        let rules = vec![rule("OP-2", Binding::Triggered, &[])];
        let err = validate(&rules).unwrap_err().to_string();
        assert!(err.contains("OP-2") && err.contains("Serves"), "{err}");
    }

    #[test]
    fn duplicate_ids_are_refused() {
        let rules = vec![
            rule("OP-1", Binding::Always, &[]),
            rule("OP-1", Binding::Always, &[]),
        ];
        let err = validate(&rules).unwrap_err().to_string();
        assert!(err.contains("OP-1") && err.contains("duplicate"), "{err}");
    }

    #[test]
    fn a_well_formed_set_passes() {
        let rules = vec![
            rule("OP-1", Binding::Always, &[]),
            rule("OP-2", Binding::Triggered, &["Agent"]),
        ];
        validate(&rules).unwrap();
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib operator_rules::validate`
Expected: FAIL to compile — `validate` is not defined.

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/operator_rules/validate.rs`:

```rust
use std::collections::HashSet;

use anyhow::{Result, bail};

use crate::operator_rules::rule::{Binding, Rule};

/// Gate 6 — every rule has a disposition and a coherent binding.
///
/// `tracker-conventions` records that an absent field-presence sweep left 39 of 57
/// entries unharvestable for three months. This is that sweep, run as a gate.
pub fn validate(rules: &[Rule]) -> Result<()> {
    let mut seen: HashSet<&str> = HashSet::new();
    for r in rules {
        if !seen.insert(r.id.as_str()) {
            bail!("{}: duplicate entry id", r.id);
        }
        match r.binding {
            Binding::Always if !r.serves.is_empty() => bail!(
                "{}: an `always` rule must not declare **Serves:** — it is resident, \
                 so it has no trigger",
                r.id
            ),
            Binding::Triggered if r.serves.is_empty() => bail!(
                "{}: a `triggered` rule must declare **Serves:** — without a selector \
                 it can never fire",
                r.id
            ),
            _ => {}
        }
        if r.covers.trim().is_empty() {
            bail!("{}: **Covers:** must name a failure mode", r.id);
        }
    }
    Ok(())
}
```

Add `pub mod validate;` to `src/operator_rules/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib operator_rules::validate`
Expected: PASS — 4 tests.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/operator_rules/
git commit -m "feat(operator-rules): Gate 6 — field presence and binding coherence"
```

---

## Task 3: Budget — Gate 3, both axes

**Files:**
- Create: `src/operator_rules/budget.rs`
- Modify: `src/operator_rules/mod.rs` (add `pub mod budget;`)

**Interfaces:**
- Consumes: `Rule`, `Binding`, `Evidence`, `Status` from Task 1.
- Produces: `pub const SIZE_CEILING: usize = 10;`,
  `pub fn check_budget(rules: &[Rule]) -> anyhow::Result<()>`.

The two constraints are separate and fail for different reasons. Only `Status::Active`
`always` rules count — a `retired` rule left in the ledger occupies no slot.

- [ ] **Step 1: Write the failing test**

Create `src/operator_rules/budget.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator_rules::rule::{Binding, Evidence, Rule, Shape, Status};

    fn always(id: &str, covers: &str, status: Status, evidence: Evidence) -> Rule {
        Rule {
            id: id.into(),
            title: "T".into(),
            imperative: "Do the thing.".into(),
            binding: Binding::Always,
            shape: Shape::Imperative,
            covers: covers.into(),
            serves: vec![],
            evidence,
            rests_on: None,
            status,
        }
    }

    #[test]
    fn two_always_rules_covering_one_failure_mode_are_refused() {
        let rules = vec![
            always("OP-1", "unverified-assertion", Status::Active, Evidence::Unmeasured),
            always("OP-2", "unverified-assertion", Status::Active, Evidence::Unmeasured),
        ];
        let err = check_budget(&rules).unwrap_err().to_string();
        assert!(err.contains("OP-2") && err.contains("OP-1"), "names both: {err}");
        assert!(err.contains("unverified-assertion"), "names the mode: {err}");
    }

    /// (a) binds below the ceiling too — headroom is not licence to add a second
    /// rule about the same failure.
    #[test]
    fn overlap_is_refused_even_with_headroom_to_spare() {
        let rules = vec![
            always("OP-1", "same-mode", Status::Active, Evidence::Unmeasured),
            always("OP-2", "same-mode", Status::Active, Evidence::Unmeasured),
        ];
        assert!(rules.len() < SIZE_CEILING, "well under the ceiling");
        assert!(check_budget(&rules).is_err());
    }

    #[test]
    fn distinct_failure_modes_below_the_ceiling_pass() {
        let rules: Vec<Rule> = (0..SIZE_CEILING)
            .map(|i| {
                always(
                    &format!("OP-{i}"),
                    &format!("mode-{i}"),
                    Status::Active,
                    Evidence::Unmeasured,
                )
            })
            .collect();
        check_budget(&rules).unwrap();
    }

    #[test]
    fn exceeding_the_ceiling_without_evidence_is_refused() {
        let rules: Vec<Rule> = (0..=SIZE_CEILING)
            .map(|i| {
                always(
                    &format!("OP-{i}"),
                    &format!("mode-{i}"),
                    Status::Active,
                    Evidence::Unmeasured,
                )
            })
            .collect();
        let err = check_budget(&rules).unwrap_err().to_string();
        assert!(err.contains("ceiling"), "{err}");
    }

    #[test]
    fn a_measured_rule_may_exceed_the_ceiling() {
        let mut rules: Vec<Rule> = (0..SIZE_CEILING)
            .map(|i| {
                always(
                    &format!("OP-{i}"),
                    &format!("mode-{i}"),
                    Status::Active,
                    Evidence::Measured {
                        arm: "a/b".into(),
                        base: 0.0,
                        shipped: 100.0,
                        n: 35,
                    },
                )
            })
            .collect();
        rules.push(always(
            "OP-99",
            "extra-mode",
            Status::Active,
            Evidence::Measured {
                arm: "a/b".into(),
                base: 0.0,
                shipped: 100.0,
                n: 35,
            },
        ));
        check_budget(&rules).unwrap();
    }

    #[test]
    fn a_retired_rule_occupies_no_slot() {
        let mut rules: Vec<Rule> = (0..SIZE_CEILING)
            .map(|i| {
                always(
                    &format!("OP-{i}"),
                    &format!("mode-{i}"),
                    Status::Active,
                    Evidence::Unmeasured,
                )
            })
            .collect();
        rules.push(always(
            "OP-98",
            "retired-mode",
            Status::Retired,
            Evidence::Unmeasured,
        ));
        check_budget(&rules).unwrap();
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib operator_rules::budget`
Expected: FAIL to compile — `check_budget` and `SIZE_CEILING` are not defined.

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/operator_rules/budget.rs`:

```rust
use std::collections::HashMap;

use anyhow::{Result, bail};

use crate::operator_rules::rule::{Binding, Evidence, Rule, Status};

/// Size ceiling for the resident (`always`) set. A judgement call, not a
/// measurement: `A-20`'s dilution finding constrains OVERLAP (axis (a) of
/// [`check_budget`]), not headcount. Initial working range is 3–5; this is the
/// hard ceiling beyond which an addition must carry evidence.
pub const SIZE_CEILING: usize = 10;

/// Gate 3 — the two budget constraints, which fail for different reasons.
///
/// **(a) Non-overlap.** No two active `always` rules may share a `**Covers:**`
/// failure mode. This is what `prompt-hamsa-audit-log:A-20` supports: its
/// stacking arm was `a5-both`, two treatments of the *same* behaviour. It binds
/// regardless of how much size headroom remains.
///
/// **(b) Size.** Beyond [`SIZE_CEILING`], an addition needs
/// `**Evidence:** measured: …` or the eviction of an existing rule.
pub fn check_budget(rules: &[Rule]) -> Result<()> {
    let resident: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.binding == Binding::Always && r.status == Status::Active)
        .collect();

    let mut by_mode: HashMap<&str, &str> = HashMap::new();
    for r in &resident {
        if let Some(prior) = by_mode.insert(r.covers.as_str(), r.id.as_str()) {
            bail!(
                "{}: failure mode `{}` is already covered by {} — two resident rules on \
                 one failure mode is the stacking A-20 measured as dilution. Merge them, \
                 or make one `triggered`.",
                r.id,
                r.covers,
                prior
            );
        }
    }

    if resident.len() > SIZE_CEILING {
        let unmeasured: Vec<&str> = resident
            .iter()
            .filter(|r| r.evidence == Evidence::Unmeasured)
            .map(|r| r.id.as_str())
            .collect();
        if !unmeasured.is_empty() {
            bail!(
                "resident set is {} rules, over the ceiling of {SIZE_CEILING}; these carry \
                 no measurement and must earn a slot or be evicted: {}",
                resident.len(),
                unmeasured.join(", ")
            );
        }
    }
    Ok(())
}
```

Add `pub mod budget;` to `src/operator_rules/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib operator_rules::budget`
Expected: PASS — 6 tests.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/operator_rules/
git commit -m "feat(operator-rules): Gate 3 — non-overlap and size ceiling as separate constraints"
```

---

## Task 4: Render the resident block

**Files:**
- Create: `src/operator_rules/render.rs`
- Modify: `src/operator_rules/mod.rs` (add `pub mod render;`)

**Interfaces:**
- Consumes: `Rule`, `Binding`, `Status` from Task 1.
- Produces: `pub const BEGIN: &str`, `pub const END: &str`,
  `pub fn render_block(rules: &[Rule]) -> String`.

The `<!-- rules: … -->` manifest line is load-bearing: it lets Gate 2 compare **rule ids**
before falling back to bytes, so `check` can say *which rule* is missing from *which
profile* — something `diff` cannot do.

- [ ] **Step 1: Write the failing test**

Create `src/operator_rules/render.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator_rules::rule::{Binding, Evidence, Rule, Shape, Status};

    fn rule(id: &str, binding: Binding, status: Status, imperative: &str) -> Rule {
        Rule {
            id: id.into(),
            title: "T".into(),
            imperative: imperative.into(),
            binding,
            shape: Shape::Imperative,
            covers: format!("mode-{id}"),
            serves: if binding == Binding::Triggered {
                vec!["Agent".into()]
            } else {
                vec![]
            },
            evidence: Evidence::Unmeasured,
            rests_on: None,
            status,
        }
    }

    #[test]
    fn renders_only_active_always_rules_in_id_order() {
        let rules = vec![
            rule("OP-2", Binding::Always, Status::Active, "Second."),
            rule("OP-1", Binding::Always, Status::Active, "First."),
            rule("OP-3", Binding::Triggered, Status::Active, "Triggered."),
            rule("OP-4", Binding::Always, Status::Retired, "Retired."),
        ];
        let block = render_block(&rules);
        assert!(block.starts_with(BEGIN), "opens with the marker: {block}");
        assert!(block.trim_end().ends_with(END), "closes with the marker: {block}");
        assert!(block.contains("<!-- rules: OP-1, OP-2 -->"), "manifest: {block}");
        assert!(block.contains("First.") && block.contains("Second."));
        assert!(!block.contains("Triggered."), "triggered rules are not resident");
        assert!(!block.contains("Retired."), "retired rules are not resident");
        let first = block.find("First.").unwrap();
        let second = block.find("Second.").unwrap();
        assert!(first < second, "sorted by numeric id, not document order");
    }

    /// Lexicographic order would put OP-10 before OP-2, making block order depend
    /// on how many rules exist.
    #[test]
    fn ids_sort_numerically_not_lexicographically() {
        let rules = vec![
            rule("OP-10", Binding::Always, Status::Active, "Ten."),
            rule("OP-2", Binding::Always, Status::Active, "Two."),
        ];
        let block = render_block(&rules);
        assert!(block.contains("<!-- rules: OP-2, OP-10 -->"), "{block}");
    }

    #[test]
    fn render_is_deterministic() {
        let rules = vec![rule("OP-1", Binding::Always, Status::Active, "Only.")];
        assert_eq!(render_block(&rules), render_block(&rules));
    }

    #[test]
    fn an_empty_resident_set_still_renders_a_well_formed_block() {
        let block = render_block(&[]);
        assert!(block.starts_with(BEGIN) && block.trim_end().ends_with(END), "{block}");
        assert!(block.contains("<!-- rules:  -->"), "empty manifest: {block}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib operator_rules::render`
Expected: FAIL to compile — `render_block`, `BEGIN`, `END` are not defined.

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/operator_rules/render.rs`:

```rust
use crate::operator_rules::rule::{Binding, Rule, Status};

pub const BEGIN: &str =
    "<!-- BEGIN operator-rules (generated from docs/trackers/operator-rules.md — do not edit) -->";
pub const END: &str = "<!-- END operator-rules -->";

/// Numeric sort on the `OP-<n>` suffix. Lexicographic order would put `OP-10`
/// before `OP-2` and make block order depend on how many rules exist.
fn id_ordinal(id: &str) -> u32 {
    id.rsplit('-')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(u32::MAX)
}

/// Render the resident set: active `always` rules, in numeric id order.
///
/// The `<!-- rules: … -->` manifest lets a checker compare id SETS before
/// comparing bytes, so drift is reportable as "OP-3 missing from profile X"
/// rather than as an opaque byte difference.
pub fn render_block(rules: &[Rule]) -> String {
    let mut resident: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.binding == Binding::Always && r.status == Status::Active)
        .collect();
    resident.sort_by_key(|r| id_ordinal(&r.id));

    let manifest: Vec<&str> = resident.iter().map(|r| r.id.as_str()).collect();
    let mut out = String::new();
    out.push_str(BEGIN);
    out.push('\n');
    out.push_str(&format!("<!-- rules: {} -->\n", manifest.join(", ")));
    for r in &resident {
        out.push_str(&format!("\n{}\n", r.imperative));
    }
    out.push('\n');
    out.push_str(END);
    out.push('\n');
    out
}
```

Add `pub mod render;` to `src/operator_rules/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib operator_rules::render`
Expected: PASS — 4 tests.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/operator_rules/
git commit -m "feat(operator-rules): render the resident block with an id manifest"
```

---

## Task 5: Profiles and idempotent splice — Gate 1

**Files:**
- Create: `src/operator_rules/profiles.rs`
- Modify: `src/operator_rules/mod.rs` (add `pub mod profiles;`)

**Interfaces:**
- Consumes: `BEGIN`, `END` from Task 4.
- Produces: `pub const PROFILE_DIRS: [&str; 3]`;
  `pub struct OperatorProfiles { pub paths: Vec<std::path::PathBuf> }` with
  `OperatorProfiles::from_env() -> anyhow::Result<Self>`;
  `pub fn extract_block(doc: &str) -> Option<&str>`;
  `pub fn splice(doc: &str, block: &str) -> anyhow::Result<String>`.

**`from_env` is the only function in this module that touches the environment.** Tests
construct `OperatorProfiles { paths: … }` literally with tempdir paths and never call it —
`docs/conventions/test-env-isolation.md` option A. That boundary is also what stops a test
run from overwriting the operator's real `~/.claude/CLAUDE.md`.

- [ ] **Step 1: Write the failing test**

Create `src/operator_rules/profiles.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn block() -> String {
        format!("{BEGIN}\n<!-- rules: OP-1 -->\n\nVerify.\n\n{END}\n")
    }

    #[test]
    fn splice_appends_the_block_when_no_markers_are_present() {
        let doc = "# My notes\n\nHand-written.\n";
        let out = splice(doc, &block()).unwrap();
        assert!(
            out.starts_with("# My notes\n\nHand-written.\n"),
            "preserves the original: {out}"
        );
        assert!(out.contains("<!-- rules: OP-1 -->"), "{out}");
    }

    #[test]
    fn splice_replaces_only_the_block_and_preserves_everything_else() {
        let doc = format!("# Head\n\nBefore.\n\n{}\nAfter.\n", block());
        let new_block = block()
            .replace("OP-1", "OP-1, OP-2")
            .replace("Verify.", "Verify.\n\nAlso this.");
        let out = splice(&doc, &new_block).unwrap();
        assert!(out.starts_with("# Head\n\nBefore.\n\n"), "prefix intact: {out}");
        assert!(out.ends_with("\nAfter.\n"), "suffix intact: {out}");
        assert!(out.contains("OP-2") && out.contains("Also this."), "{out}");
    }

    /// Gate 1 — compile is a fixed point after the first pass.
    #[test]
    fn splice_is_idempotent() {
        let doc = "# Head\n\nBefore.\n";
        let once = splice(doc, &block()).unwrap();
        let twice = splice(&once, &block()).unwrap();
        assert_eq!(once, twice, "second compile must be a no-op");
    }

    #[test]
    fn extract_block_returns_none_without_markers_and_the_block_with_them() {
        assert!(extract_block("no markers here").is_none());
        let doc = format!("x\n{}y\n", block());
        let got = extract_block(&doc).expect("markers present");
        assert!(got.contains("<!-- rules: OP-1 -->"), "{got}");
    }

    #[test]
    fn splice_refuses_a_document_with_an_unterminated_begin_marker() {
        let doc = format!("{BEGIN}\ndangling\n");
        let err = splice(&doc, &block()).unwrap_err().to_string();
        assert!(err.contains("END"), "names the missing marker: {err}");
    }

    #[test]
    fn profiles_are_constructible_without_touching_the_environment() {
        let dir = tempfile::tempdir().unwrap();
        let p = OperatorProfiles {
            paths: vec![dir.path().join("CLAUDE.md")],
        };
        assert_eq!(p.paths.len(), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib operator_rules::profiles`
Expected: FAIL to compile — `splice`, `extract_block`, `OperatorProfiles` are not defined.

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/operator_rules/profiles.rs`:

```rust
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::operator_rules::render::{BEGIN, END};

/// The Claude Code profile directories whose `CLAUDE.md` receives the resident
/// block. Machine-scoped by design — the spec puts cross-machine sync out of
/// scope.
pub const PROFILE_DIRS: [&str; 3] = [".claude", ".claude-sdd", ".claude-kat"];

/// Resolved profile paths.
///
/// Constructed literally in tests with tempdir paths; [`Self::from_env`] is the
/// only thing that reads the environment, per
/// `docs/conventions/test-env-isolation.md` option A. That boundary is why the
/// test suite cannot overwrite the operator's real `~/.claude/CLAUDE.md`.
#[derive(Debug, Clone)]
pub struct OperatorProfiles {
    pub paths: Vec<PathBuf>,
}

impl OperatorProfiles {
    /// Read the home directory once, at the edge. The only env access here.
    pub fn from_env() -> Result<Self> {
        let home = dirs::home_dir().context("cannot resolve the home directory")?;
        Ok(Self {
            paths: PROFILE_DIRS
                .iter()
                .map(|d| home.join(d).join("CLAUDE.md"))
                .collect(),
        })
    }
}

/// The generated block, markers included, or `None` when the document has none.
pub fn extract_block(doc: &str) -> Option<&str> {
    let start = doc.find(BEGIN)?;
    let end = doc[start..].find(END)? + start + END.len();
    Some(&doc[start..end])
}

/// Replace the generated block, or append it when absent.
///
/// Everything outside the markers is preserved byte for byte — Gate 1.
pub fn splice(doc: &str, block: &str) -> Result<String> {
    let Some(start) = doc.find(BEGIN) else {
        let mut out = doc.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(block);
        return Ok(out);
    };
    let Some(rel_end) = doc[start..].find(END) else {
        bail!(
            "document has a BEGIN operator-rules marker with no matching END marker; \
             refusing to guess where the generated block ends"
        );
    };
    let end = start + rel_end + END.len();
    let mut out = String::with_capacity(doc.len() + block.len());
    out.push_str(&doc[..start]);
    out.push_str(block.trim_end_matches('\n'));
    out.push_str(&doc[end..]);
    Ok(out)
}
```

Add `pub mod profiles;` to `src/operator_rules/mod.rs`.

Confirm `dirs` is in `[dependencies]` and `tempfile` in `[dev-dependencies]` in
`Cargo.toml` — both are already used elsewhere in the tree. If `dirs` is absent, add it
rather than reading `std::env::var("HOME")`, which is not Windows-correct.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib operator_rules::profiles`
Expected: PASS — 6 tests.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/operator_rules/ Cargo.toml
git commit -m "feat(operator-rules): idempotent marker splice, env resolved at the edge"
```

---

## Task 6: `compile` and `check` — Gate 2

**Files:**
- Modify: `src/operator_rules/mod.rs` — replace its contents with the orchestration below
  and add its test module.

**Interfaces:**
- Consumes: `parse_ledger`, `validate`, `check_budget`, `render_block`, `splice`,
  `extract_block`, `OperatorProfiles`.
- Produces: `pub struct Drift { pub path: PathBuf, pub reason: String }`;
  `pub fn compile(ledger: &str, profiles: &OperatorProfiles) -> anyhow::Result<Vec<PathBuf>>`
  (the paths actually rewritten);
  `pub fn check(ledger: &str, profiles: &OperatorProfiles) -> anyhow::Result<Vec<Drift>>`.

Gate 2 is the point of the whole phase: `check` must fire on a **rule** difference and stay
silent on a **whitespace** difference outside the markers. That is the distinction `diff`
cannot draw, and whose absence is the defect in the spec's Problem section.

- [ ] **Step 1: Write the failing test**

Append to `src/operator_rules/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const LEDGER: &str = r#"---
kind: tracker
entry_prefix: OP
---

## OP-1 — Always verify

**Imperative:** Do not hypothesise — ALWAYS VERIFY.
**Binding:** always
**Shape:** imperative
**Covers:** unverified-assertion
**Evidence:** unmeasured
**Status:** active
"#;

    fn profiles_in(dir: &std::path::Path, n: usize) -> OperatorProfiles {
        let paths: Vec<std::path::PathBuf> = (0..n)
            .map(|i| {
                let p = dir.join(format!("profile{i}"));
                std::fs::create_dir_all(&p).unwrap();
                p.join("CLAUDE.md")
            })
            .collect();
        for p in &paths {
            std::fs::write(p, "# Operator notes\n\nHand-written prose.\n").unwrap();
        }
        OperatorProfiles { paths }
    }

    #[test]
    fn check_reports_drift_before_the_first_compile_and_none_after() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);

        let drift = check(LEDGER, &profiles).unwrap();
        assert_eq!(drift.len(), 3, "no profile has the block yet: {drift:#?}");

        compile(LEDGER, &profiles).unwrap();
        assert!(check(LEDGER, &profiles).unwrap().is_empty(), "clean after compile");
    }

    /// Gate 2, the discriminating half. A hand-edit OUTSIDE the markers is the
    /// operator's own prose and must not read as drift — this is exactly the
    /// blank-line difference that made the three real profiles look divergent.
    #[test]
    fn check_ignores_whitespace_outside_the_markers() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);
        compile(LEDGER, &profiles).unwrap();

        let p = &profiles.paths[1];
        let doc = std::fs::read_to_string(p).unwrap();
        std::fs::write(p, format!("{doc}\n\n\nA trailing note from the operator.\n")).unwrap();

        assert!(
            check(LEDGER, &profiles).unwrap().is_empty(),
            "prose outside the block is not drift"
        );
    }

    /// Gate 2, the firing half — and it must name the RULE, not just the bytes.
    #[test]
    fn check_names_the_missing_rule_when_a_profile_falls_behind() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);
        compile(LEDGER, &profiles).unwrap();

        let extended = format!(
            "{LEDGER}\n## OP-2 — Prefer the tracker\n\n\
             **Imperative:** Record durable facts in a tracker.\n\
             **Binding:** always\n**Shape:** imperative\n\
             **Covers:** durable-fact-placement\n**Evidence:** unmeasured\n\
             **Status:** active\n"
        );
        let drift = check(&extended, &profiles).unwrap();
        assert_eq!(drift.len(), 3, "every profile is now behind");
        assert!(drift[0].reason.contains("OP-2"), "names the rule: {}", drift[0].reason);
    }

    #[test]
    fn compile_rewrites_only_profiles_that_changed() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);
        assert_eq!(compile(LEDGER, &profiles).unwrap().len(), 3, "all three on first run");
        assert!(compile(LEDGER, &profiles).unwrap().is_empty(), "second run is a no-op");
    }

    #[test]
    fn compile_refuses_a_ledger_that_fails_the_budget_and_writes_nothing() {
        let two_same_mode = format!(
            "{LEDGER}\n## OP-2 — Duplicate mode\n\n\
             **Imperative:** Also verify.\n**Binding:** always\n**Shape:** imperative\n\
             **Covers:** unverified-assertion\n**Evidence:** unmeasured\n**Status:** active\n"
        );
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);
        let err = compile(&two_same_mode, &profiles).unwrap_err().to_string();
        assert!(err.contains("unverified-assertion"), "{err}");
        let doc = std::fs::read_to_string(&profiles.paths[0]).unwrap();
        assert!(!doc.contains("operator-rules"), "nothing written on a refused compile");
    }

    #[test]
    fn exit_code_is_one_on_drift_and_zero_when_clean() {
        assert_eq!(exit_code(&[]), 0);
        assert_eq!(
            exit_code(&[Drift {
                path: "x".into(),
                reason: "r".into()
            }]),
            1
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib operator_rules::tests`
Expected: FAIL to compile — `compile`, `check`, `Drift`, `exit_code` are not defined.

- [ ] **Step 3: Write minimal implementation**

Replace everything in `src/operator_rules/mod.rs` above its test module with:

```rust
//! Operator rules — engine 5 of the rules programme.
//!
//! Rules that hold across every project, tool and model for one operator. Rules
//! bound `always` compile into a delimited block in each Claude Code profile's
//! `CLAUDE.md`; rules bound `triggered` are routed at runtime in Phase 2.
//!
//! Spec: `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md`.

pub mod budget;
pub mod profiles;
pub mod render;
pub mod rule;
pub mod validate;

use std::path::PathBuf;

use anyhow::{Context, Result};

pub use profiles::OperatorProfiles;

/// Default ledger path, relative to the repo root.
pub const LEDGER_PATH: &str = "docs/trackers/operator-rules.md";

/// One profile's disagreement with the compiled block.
#[derive(Debug, Clone)]
pub struct Drift {
    pub path: PathBuf,
    pub reason: String,
}

/// The `check` exit-code contract, as a pure function so the CLI arm cannot
/// drift from it: non-zero exactly when there is drift.
pub fn exit_code(drift: &[Drift]) -> i32 {
    if drift.is_empty() { 0 } else { 1 }
}

/// Parse, validate and budget-check the ledger, then render the resident block.
fn resident_block(ledger: &str) -> Result<String> {
    let rules = rule::parse_ledger(ledger)?;
    validate::validate(&rules)?;
    budget::check_budget(&rules)?;
    Ok(render::render_block(&rules))
}

/// Extract the `<!-- rules: … -->` manifest from a rendered or on-disk block.
fn manifest_of(block: &str) -> Vec<String> {
    block
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("<!-- rules:")?
                .strip_suffix("-->")
                .map(str::to_string)
        })
        .map(|ids| {
            ids.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Write the resident block into every profile that does not already match.
///
/// Returns the paths actually rewritten — empty on a second run, which is Gate 1's
/// idempotence observed from outside. Nothing is written if the ledger fails
/// validation or the budget: `resident_block` runs to completion first.
pub fn compile(ledger: &str, profiles: &OperatorProfiles) -> Result<Vec<PathBuf>> {
    let block = resident_block(ledger)?;
    let mut written = Vec::new();
    for path in &profiles.paths {
        let doc = std::fs::read_to_string(path)
            .with_context(|| format!("reading profile {}", path.display()))?;
        let next = profiles::splice(&doc, &block)?;
        if next != doc {
            std::fs::write(path, &next)
                .with_context(|| format!("writing profile {}", path.display()))?;
            written.push(path.clone());
        }
    }
    Ok(written)
}

/// Report every profile whose generated block disagrees with the ledger.
///
/// Comparison is over rule ids first, then rendered bytes — so drift reads as
/// "missing OP-2" rather than as an opaque byte difference. Content OUTSIDE the
/// markers is never compared, which is what lets an operator keep hand-written
/// prose in one profile without it reading as drift.
pub fn check(ledger: &str, profiles: &OperatorProfiles) -> Result<Vec<Drift>> {
    let expected = resident_block(ledger)?;
    let want_ids = manifest_of(&expected);
    let mut drift = Vec::new();

    for path in &profiles.paths {
        let doc = std::fs::read_to_string(path)
            .with_context(|| format!("reading profile {}", path.display()))?;
        let Some(found) = profiles::extract_block(&doc) else {
            drift.push(Drift {
                path: path.clone(),
                reason: format!("no generated block; expected rules: {}", want_ids.join(", ")),
            });
            continue;
        };
        let got_ids = manifest_of(found);
        if got_ids != want_ids {
            let missing: Vec<&String> = want_ids.iter().filter(|i| !got_ids.contains(i)).collect();
            let extra: Vec<&String> = got_ids.iter().filter(|i| !want_ids.contains(i)).collect();
            drift.push(Drift {
                path: path.clone(),
                reason: format!("rule set differs — missing: {missing:?}, unexpected: {extra:?}"),
            });
            continue;
        }
        if found.trim_end() != expected.trim_end() {
            drift.push(Drift {
                path: path.clone(),
                reason: "same rules, different rendered text — recompile".to_string(),
            });
        }
    }
    Ok(drift)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib operator_rules`
Expected: PASS — all tests across the five submodules plus 6 here.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/operator_rules/
git commit -m "feat(operator-rules): Gate 2 — check discriminates rule drift from operator prose"
```

---

## Task 7: CLI subcommand

**Files:**
- Modify: `src/main.rs` — add a `Commands::OperatorRules` variant after
  `Commands::ConstitutionCheck` (currently the last variant, `src/main.rs:218`), and a
  dispatch arm in `main` (`src/main.rs:231-488`).

**Interfaces:**
- Consumes: `codescout::operator_rules::{compile, check, exit_code, OperatorProfiles,
  LEDGER_PATH}` from Task 6.
- Produces: `codescout operator-rules compile` and `codescout operator-rules check`.
  `check` exits **1** on drift, **0** when clean.

The exit-code contract is already unit-tested as `exit_code` in Task 6, so this arm stays
thin enough that there is nothing left to test in-process.

- [ ] **Step 1: Add the clap variant**

In `src/main.rs`, immediately after the `ConstitutionCheck` variant:

```rust
    /// Compile operator rules into each Claude Code profile's CLAUDE.md, or check for drift.
    OperatorRules {
        /// `compile` writes; `check` reports drift and exits 1 if any.
        #[arg(value_parser = ["compile", "check"])]
        mode: String,
        /// Ledger path. Defaults to docs/trackers/operator-rules.md.
        #[arg(long)]
        ledger: Option<std::path::PathBuf>,
    },
```

- [ ] **Step 2: Run the build to verify it fails on the missing match arm**

Run: `cargo build`
Expected: FAIL — `non-exhaustive patterns: 'Commands::OperatorRules { .. }' not covered`.

- [ ] **Step 3: Add the dispatch arm**

In `main`, alongside the other command arms:

```rust
        Commands::OperatorRules { mode, ledger } => {
            use codescout::operator_rules as ops;
            let path = ledger.unwrap_or_else(|| ops::LEDGER_PATH.into());
            let doc = std::fs::read_to_string(&path)
                .with_context(|| format!("reading ledger {}", path.display()))?;
            let profiles = ops::OperatorProfiles::from_env()?;
            match mode.as_str() {
                "compile" => {
                    let written = ops::compile(&doc, &profiles)?;
                    if written.is_empty() {
                        println!(
                            "operator-rules: already current in all {} profiles",
                            profiles.paths.len()
                        );
                    } else {
                        for p in &written {
                            println!("operator-rules: wrote {}", p.display());
                        }
                    }
                }
                "check" => {
                    let drift = ops::check(&doc, &profiles)?;
                    for d in &drift {
                        eprintln!("operator-rules: DRIFT {} — {}", d.path.display(), d.reason);
                    }
                    if drift.is_empty() {
                        println!("operator-rules: all {} profiles current", profiles.paths.len());
                    }
                    std::process::exit(ops::exit_code(&drift));
                }
                _ => unreachable!("clap value_parser restricts mode"),
            }
        }
```

- [ ] **Step 4: Verify the exit-code contract from the shell**

```bash
cargo build --release
printf -- '---\nkind: tracker\nentry_prefix: OP\n---\n' > /tmp/empty-ledger.md
./target/release/codescout operator-rules check --ledger /tmp/empty-ledger.md; echo "exit=$?"
```

Expected: three `DRIFT` lines naming your real profile paths (none has a block yet) and
`exit=1`.

**Do not run `operator-rules compile` against your real profiles yet** — Task 8 seeds the
ledger first, and compiling an empty ledger would write an empty block into all three.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/main.rs
git commit -m "feat(operator-rules): codescout operator-rules compile|check"
```

---

## Task 8: Seed the ledger and verify end to end

**Files:**
- Create: `docs/trackers/operator-rules.md`

**Interfaces:**
- Consumes: everything above.
- Produces: a live ledger with one measured rule.

This seeds **one** rule — the only one with recorded evidence. The other three
(`subagent model floor`, `codescout memory not CC memory`, `three-profile config sync`) are
Phase 3, and two are projected `triggered`, which Phase 1 cannot deliver anyway.

- [ ] **Step 1: Create the ledger through the librarian**

Do **not** hand-write the `## OP-1` heading. The server formats it as `## <ID> — <title>`,
the only shape `link_scan` accepts as a definition; a hand-written heading missing the
dash-and-title defines no token and every citation of it dangles.

```
artifact(action="create", kind="tracker", status="active",
  rel_path="docs/trackers/operator-rules.md",
  title="Operator Rules (OP-N)",
  tags=["operator-rules", "engine-5", "ledger"],
  body="# Operator Rules (OP-N)\n\nRules that hold across every project, tool and model for this operator. Compiled into each Claude Code profile's CLAUDE.md by `codescout operator-rules compile`.\n\nSpec: `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md`.\n\n## Index\n\n| ID | Binding | Covers | Evidence | Status |\n|---|---|---|---|---|\n\n## Template for new entries\n\n<!-- Insert new OP-N entries above this line. Use artifact(action=\"append_entry\", id=<this artifact>, id_prefix=\"OP\", anchor_heading=\"## Template for new entries\", title=…, body=…) — never hand-format the heading. -->\n")
```

Then declare the ledger, so `append_entry` will allocate against it:

```
artifact(action="update", id="<returned id>",
  patch={extra: {"entry_prefix": "OP"}})
```

- [ ] **Step 2: Append OP-1 through the allocator**

```
artifact(action="append_entry", id="<ledger id>", id_prefix="OP",
  anchor_heading="## Template for new entries",
  title="Always verify before asserting",
  body="**Imperative:** Do not hypothesise — ALWAYS VERIFY.\n**Binding:** always\n**Shape:** imperative\n**Covers:** unverified-assertion\n**Evidence:** measured: conclude-last/b2 0% -> 100% (n=35)\n**Rests on:** prompt-hamsa-audit-log:A-21\n**Status:** active\n\n**Valid:** invariant\n\nThe active ingredient is an unconditional imperative that binds at every claim. A-21 measured 11 arms: b2 imperative-only scored 100.0%, beating the full paragraph at 93.3%, against 0% bare. Conditional guards gate on the doubt a planted belief suppresses, which is why the guard-shaped variants lost.")
```

Then add the Index row using the id the call returned — **after** the section exists, never
before, because the allocator counts an id already claimed by an index row.

- [ ] **Step 3: Verify check fires, compile, verify it stops**

```bash
cargo build --release
./target/release/codescout operator-rules check;   echo "before=$?"
./target/release/codescout operator-rules compile
./target/release/codescout operator-rules check;   echo "after=$?"
```

Expected: `before=1` with three DRIFT lines; `compile` prints three `wrote` lines;
`after=0`. This is the spec's Verification prediction 1 — the drift check inverting.

- [ ] **Step 4: Verify the discriminating half by hand**

```bash
printf '\n\nA note only this profile has.\n' >> ~/.claude-kat/CLAUDE.md
./target/release/codescout operator-rules check; echo "after_prose_edit=$?"
```

Expected: `after_prose_edit=0`. Operator prose outside the markers is not drift. If this
prints 1, Gate 2 has not been met and Task 6 is wrong — stop and fix it before committing.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add docs/trackers/operator-rules.md
git commit -m "feat(operator-rules): seed the ledger with OP-1, the one measured rule"
```

---

## Post-Phase-1

Not tasks — the handoff.

- **Phase 2** is blocked on `sdd/get-guide-section-grain` merging to `experiments`. It adds
  `Tool::selector_key` and the prefix-aware `re_arm` that `triggered` routing reuses.
  Starting earlier means writing a second matcher, which is what that phase exists to avoid.
- **Phase 3** harvests the remaining three rules and commissions arms for them. Two are
  projected `triggered` and cannot be delivered until Phase 2 lands; the third
  (`three-profile config sync`) becomes largely redundant once `check` exists, and Phase 3
  should decide whether to **retire** it rather than transcribe it.
- **Verification prediction 2** — the compiled block reproduces `conclude-last`'s shipped
  100%/100% at n=35 — is not a Phase 1 task because it runs in the `prompt-engineering`
  repo. Run it once Task 8 lands; a drop means compilation altered the rule's effective
  form and `render_block` is at fault.
