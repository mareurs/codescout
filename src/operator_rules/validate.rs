use std::collections::HashSet;

use anyhow::{bail, Result};

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
}
