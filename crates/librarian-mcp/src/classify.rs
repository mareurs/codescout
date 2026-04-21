use anyhow::{Context, Result};
use globset::{Glob, GlobMatcher};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleFile {
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub glob: String,
    pub kind: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub time_scope: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub matcher: GlobMatcher,
    pub kind: String,
    pub status: Option<String>,
    pub time_scope: Option<String>,
}

pub fn compile_rules(rules: &[Rule]) -> Result<Vec<CompiledRule>> {
    rules
        .iter()
        .map(|r| {
            let matcher = Glob::new(&r.glob)
                .with_context(|| format!("invalid glob: {}", r.glob))?
                .compile_matcher();
            Ok(CompiledRule {
                matcher,
                kind: r.kind.clone(),
                status: r.status.clone(),
                time_scope: r.time_scope.clone(),
            })
        })
        .collect()
}

pub fn load_rules(toml_str: &str) -> Result<Vec<CompiledRule>> {
    let file: RuleFile = toml::from_str(toml_str).context("parsing classification rules")?;
    compile_rules(&file.rules)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    pub kind: String,
    pub status: Option<String>,
    pub time_scope: Option<String>,
}

pub fn classify(rules: &[CompiledRule], rel_path: &str) -> Option<Classification> {
    for r in rules {
        if r.matcher.is_match(rel_path) {
            return Some(Classification {
                kind: r.kind.clone(),
                status: r.status.clone(),
                time_scope: r.time_scope.clone(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> &'static str {
        r#"
[[rule]]
glob = "**/docs/superpowers/specs/*.md"
kind = "spec"
status = "active"

[[rule]]
glob = "**/docs/research/*.md"
kind = "memory"
time_scope = "dated_snapshot"

[[rule]]
glob = "**/ROADMAP.md"
kind = "roadmap"
"#
    }

    #[test]
    fn load_rules_parses_multiple() {
        let rules = load_rules(sample()).unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].kind, "spec");
    }

    #[test]
    fn classify_matches_spec() {
        let rules = load_rules(sample()).unwrap();
        let c = classify(&rules, "docs/superpowers/specs/foo.md").unwrap();
        assert_eq!(c.kind, "spec");
        assert_eq!(c.status.as_deref(), Some("active"));
    }

    #[test]
    fn classify_matches_memory_with_time_scope() {
        let rules = load_rules(sample()).unwrap();
        let c = classify(&rules, "docs/research/2026-01-01-foo.md").unwrap();
        assert_eq!(c.kind, "memory");
        assert_eq!(c.time_scope.as_deref(), Some("dated_snapshot"));
    }

    #[test]
    fn classify_first_match_wins() {
        let toml = r#"
[[rule]]
glob = "**/docs/*.md"
kind = "doc"

[[rule]]
glob = "**/docs/superpowers/specs/*.md"
kind = "spec"
"#;
        let rules = load_rules(toml).unwrap();
        let c = classify(&rules, "docs/superpowers/specs/x.md").unwrap();
        assert_eq!(c.kind, "doc", "earlier rule must win");
    }

    #[test]
    fn classify_returns_none_for_unknown() {
        let rules = load_rules(sample()).unwrap();
        assert!(classify(&rules, "random/path.md").is_none());
    }

    #[test]
    fn load_rules_rejects_bad_glob() {
        let toml = "[[rule]]\nglob = \"[\"\nkind = \"spec\"\n";
        assert!(load_rules(toml).is_err());
    }
}
