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

/// Built-in fallback rules appended after user-supplied rules.
///
/// User rules win because `classify` is first-match. These cover common
/// repo conventions that aren't worth re-declaring per workspace:
/// CHANGELOG, top-level docs, issue trackers, src-tree prompt files, etc.
/// Every entry must be commonly recognizable across the projects we expect
/// to index. Keep the list tight — broad globs cause misclassification.
pub const DEFAULT_RULES_TOML: &str = r#"
[[rule]]
glob = "**/CHANGELOG.md"
kind = "doc"

[[rule]]
glob = "**/CONTRIBUTING.md"
kind = "doc"

[[rule]]
glob = "**/docs/ARCHITECTURE.md"
kind = "doc"

[[rule]]
glob = "**/docs/QUICK-START.md"
kind = "doc"

[[rule]]
glob = "**/docs/QUICKSTART.md"
kind = "doc"

[[rule]]
glob = "**/docs/concepts/**/*.md"
kind = "doc"

[[rule]]
glob = "**/docs/configuration/**/*.md"
kind = "doc"

[[rule]]
glob = "**/docs/experimental/**/*.md"
kind = "doc"

[[rule]]
glob = "**/docs/issues/**/*.md"
kind = "tracker"
status = "active"

[[rule]]
glob = "**/docs/TODO-*.md"
kind = "tracker"
status = "active"

[[rule]]
glob = "**/docs/TODO/*.md"
kind = "tracker"
status = "active"

[[rule]]
glob = "**/docs/review-*.md"
kind = "memory"
time_scope = "dated_snapshot"

[[rule]]
glob = "**/prompts/*.md"
kind = "doc"

[[rule]]
glob = "**/src/**/prompts/*.md"
kind = "doc"

[[rule]]
glob = "**/crates/**/prompts/*.md"
kind = "doc"
"#;

/// Compile the built-in default rules. Errors only on programmer mistake
/// (a malformed glob in `DEFAULT_RULES_TOML`).
pub fn default_rules() -> Result<Vec<CompiledRule>> {
    load_rules(DEFAULT_RULES_TOML)
}

/// Path of the per-project classifier overrides file, relative to the
/// project directory. Projects can ship their own rules without polluting
/// the global workspace.toml. Rules from this file are layered ABOVE
/// workspace rules and defaults.
pub const PROJECT_RULES_REL: &str = ".codescout/librarian.toml";

/// Load per-project classifier rules from `<project_path>/.codescout/librarian.toml`.
///
/// Returns `Ok(vec![])` when the file is absent — per-project rules are
/// optional. Errors only on parse / glob compilation failure (so a broken
/// override surfaces loudly rather than silently falling back).
pub fn load_project_rules(project_path: &std::path::Path) -> Result<Vec<CompiledRule>> {
    let path = project_path.join(PROJECT_RULES_REL);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let s = std::fs::read_to_string(&path)
        .with_context(|| format!("reading project rules at {}", path.display()))?;
    load_rules(&s)
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

    #[test]
    fn default_rules_compile() {
        let rules = default_rules().expect("DEFAULT_RULES_TOML must compile");
        assert!(!rules.is_empty());
    }

    #[test]
    fn default_rules_classify_changelog() {
        let rules = default_rules().unwrap();
        let c = classify(&rules, "CHANGELOG.md").unwrap();
        assert_eq!(c.kind, "doc");
    }

    #[test]
    fn default_rules_classify_issues_dir() {
        let rules = default_rules().unwrap();
        let c = classify(&rules, "code-explorer/docs/issues/foo.md").unwrap();
        assert_eq!(c.kind, "tracker");
        assert_eq!(c.status.as_deref(), Some("active"));
    }

    #[test]
    fn default_rules_classify_todo_files() {
        let rules = default_rules().unwrap();
        let c = classify(&rules, "code-explorer/docs/TODO-tool-misbehaviors.md").unwrap();
        assert_eq!(c.kind, "tracker");
    }

    #[test]
    fn default_rules_classify_review_dated() {
        let rules = default_rules().unwrap();
        let c = classify(&rules, "code-explorer/docs/review-2026-03-05.md").unwrap();
        assert_eq!(c.kind, "memory");
        assert_eq!(c.time_scope.as_deref(), Some("dated_snapshot"));
    }

    #[test]
    fn default_rules_classify_src_prompts() {
        let rules = default_rules().unwrap();
        let c = classify(&rules, "code-explorer/src/prompts/server_instructions.md").unwrap();
        assert_eq!(c.kind, "doc");
    }

    #[test]
    fn user_rules_override_defaults_when_concatenated() {
        let mut user = load_rules(
            r#"
[[rule]]
glob = "**/CHANGELOG.md"
kind = "spec"
"#,
        )
        .unwrap();
        user.extend(default_rules().unwrap());
        let c = classify(&user, "CHANGELOG.md").unwrap();
        assert_eq!(c.kind, "spec", "user rule must win when listed first");
    }

    #[test]
    fn load_project_rules_returns_empty_when_file_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let r = load_project_rules(tmp.path()).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn load_project_rules_parses_present_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".codescout");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("librarian.toml"),
            r#"
[[rule]]
glob = "**/notes/*.md"
kind = "memory"
"#,
        )
        .unwrap();
        let r = load_project_rules(tmp.path()).unwrap();
        assert_eq!(r.len(), 1);
        let c = classify(&r, "notes/x.md").unwrap();
        assert_eq!(c.kind, "memory");
    }

    #[test]
    fn load_project_rules_errors_on_bad_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".codescout");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("librarian.toml"),
            "[[rule]]\nglob = \"[\"\nkind = \"x\"\n",
        )
        .unwrap();
        assert!(load_project_rules(tmp.path()).is_err());
    }
}
