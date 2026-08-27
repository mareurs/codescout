use std::collections::HashMap;

use anyhow::{bail, Result};

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
///
/// Assumes ids are unique — `validate` (Gate 6) rejects duplicate `OP-N` ids
/// and must run before this. A caller that invokes `check_budget` without
/// `validate` first could double-count a duplicated id in the size axis.
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
            always(
                "OP-1",
                "unverified-assertion",
                Status::Active,
                Evidence::Unmeasured,
            ),
            always(
                "OP-2",
                "unverified-assertion",
                Status::Active,
                Evidence::Unmeasured,
            ),
        ];
        let err = check_budget(&rules).unwrap_err().to_string();
        assert!(
            err.contains("OP-2") && err.contains("OP-1"),
            "names both: {err}"
        );
        assert!(
            err.contains("unverified-assertion"),
            "names the mode: {err}"
        );
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
