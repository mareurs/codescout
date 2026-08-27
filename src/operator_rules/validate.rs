use std::collections::HashSet;

use anyhow::{bail, Result};

use crate::operator_rules::render::{BEGIN, END};
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
        // X6: `Rule::finish`'s `need` closure only checks `Option::is_some` — a
        // field present but blank (`**Imperative:**` with nothing after it)
        // still satisfies it. This is the non-blank sweep for the fields that
        // matter most: a rule with no `covers` cannot be deduplicated by
        // failure mode (Gate 3a), and one with no `imperative`/`title` compiles
        // a block that asserts delivery of a rule with no text.
        if r.covers.trim().is_empty() {
            bail!("{}: **Covers:** must name a failure mode", r.id);
        }
        if r.imperative.trim().is_empty() {
            bail!("{}: **Imperative:** must not be blank", r.id);
        }
        if r.title.trim().is_empty() {
            bail!("{}: title must not be blank", r.id);
        }
        // X1 (second half): a rule whose rendered text contains the
        // operator-rules marker corrupts every profile it is spliced into —
        // the block boundary stops being unambiguous once the marker text can
        // also occur *inside* the block's own content. Reject it here, before
        // it ever reaches `render` or `splice`.
        reject_marker(&r.id, "**Imperative:**", &r.imperative)?;
        reject_marker(&r.id, "title", &r.title)?;
    }
    Ok(())
}

fn reject_marker(id: &str, field: &str, value: &str) -> Result<()> {
    if value.contains(BEGIN) {
        bail!("{id}: {field} contains the operator-rules BEGIN marker");
    }
    if value.contains(END) {
        bail!("{id}: {field} contains the operator-rules END marker");
    }
    Ok(())
}

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

    #[test]
    fn a_blank_covers_is_refused() {
        let mut r = rule("OP-1", Binding::Always, &[]);
        r.covers = "   ".into();
        let err = validate(&[r]).unwrap_err().to_string();
        assert!(err.contains("OP-1"), "names the rule: {err}");
        assert!(err.contains("Covers"), "names the field: {err}");
    }

    #[test]
    fn a_blank_imperative_is_refused() {
        let mut r = rule("OP-1", Binding::Always, &[]);
        r.imperative = "   ".into();
        let err = validate(&[r]).unwrap_err().to_string();
        assert!(err.contains("OP-1"), "names the rule: {err}");
        assert!(err.contains("Imperative"), "names the field: {err}");
    }

    #[test]
    fn a_blank_title_is_refused() {
        let mut r = rule("OP-1", Binding::Always, &[]);
        r.title = "".into();
        let err = validate(&[r]).unwrap_err().to_string();
        assert!(err.contains("OP-1"), "names the rule: {err}");
        assert!(err.contains("title"), "names the field: {err}");
    }

    /// X1's second half — a rule whose imperative quotes the real BEGIN marker
    /// must be refused before it ever reaches `render`/`splice`, where it would
    /// make `compile` non-idempotent.
    #[test]
    fn an_imperative_containing_the_begin_marker_is_refused() {
        use crate::operator_rules::render::BEGIN;
        let mut r = rule("OP-1", Binding::Always, &[]);
        r.imperative = format!("Quote the marker: {BEGIN}");
        let err = validate(&[r]).unwrap_err().to_string();
        assert!(err.contains("OP-1"), "names the rule: {err}");
        assert!(err.contains("BEGIN"), "names the marker: {err}");
    }

    #[test]
    fn an_imperative_containing_the_end_marker_is_refused() {
        use crate::operator_rules::render::END;
        let mut r = rule("OP-1", Binding::Always, &[]);
        r.imperative = format!("Quote the marker: {END}");
        let err = validate(&[r]).unwrap_err().to_string();
        assert!(err.contains("OP-1"), "names the rule: {err}");
        assert!(err.contains("END"), "names the marker: {err}");
    }

    #[test]
    fn a_title_containing_a_marker_is_refused() {
        use crate::operator_rules::render::BEGIN;
        let mut r = rule("OP-1", Binding::Always, &[]);
        r.title = BEGIN.to_string();
        let err = validate(&[r]).unwrap_err().to_string();
        assert!(err.contains("OP-1"), "names the rule: {err}");
        assert!(err.contains("title"), "names the field: {err}");
    }
}
