use anyhow::{bail, Context, Result};

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
    let pct =
        |s: &str| -> Result<f32> { Ok(s.trim().trim_end_matches('%').trim().parse::<f32>()?) };
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
