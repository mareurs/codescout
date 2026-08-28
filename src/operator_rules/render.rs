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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator_rules::rule::{Binding, Evidence, Rule, Shape, Status};
    use crate::prompts::guide_index::parse_shape;

    fn rule(id: &str, binding: Binding, status: Status, imperative: &str) -> Rule {
        Rule {
            id: id.into(),
            title: "T".into(),
            imperative: imperative.into(),
            binding,
            shape: Shape::Imperative,
            covers: format!("mode-{id}"),
            serves: if binding == Binding::Triggered {
                vec![parse_shape("Agent").unwrap()]
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
        assert!(
            block.trim_end().ends_with(END),
            "closes with the marker: {block}"
        );
        assert!(
            block.contains("<!-- rules: OP-1, OP-2 -->"),
            "manifest: {block}"
        );
        assert!(block.contains("First.") && block.contains("Second."));
        assert!(
            !block.contains("Triggered."),
            "triggered rules are not resident"
        );
        assert!(
            !block.contains("Retired."),
            "retired rules are not resident"
        );
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
        assert!(
            block.starts_with(BEGIN) && block.trim_end().ends_with(END),
            "{block}"
        );
        assert!(
            block.contains("<!-- rules:  -->"),
            "empty manifest: {block}"
        );
    }
}
