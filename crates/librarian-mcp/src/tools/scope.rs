//! Scope: turn a `(scope, current_project, workspace)` triple into a
//! `FilterNode` clause that constrains a query to the agent's current
//! project, current repo, declared umbrella, or the whole workspace.
//!
//! The clause is AND'd onto whatever filter the caller supplies. Tools
//! also surface `ScopeApplied` so they can render progressive-disclosure
//! hints ("N more in repo, M more in workspace — pass scope=...").
//!
//! Defaults — when scope is omitted on a listing tool — should be
//! `Scope::Project`. Callers must explicitly pass `all` to get the
//! pre-scoping workspace-wide behaviour.

use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::current_project::CurrentProject;
use crate::filter::FilterNode;
use crate::workspace::WorkspaceConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Project,
    #[default]
    Repo,
    Umbrella,
    All,
}

/// Snapshot of what the scope helper actually applied. Tools embed this
/// into responses so callers know the implicit filter and how to widen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeApplied {
    pub scope: Scope,
    pub root: Option<String>,
    pub subdir: Option<String>,
    pub umbrella: Option<String>,
    /// True when scope=Project but the resolved project is the whole
    /// root (subdir empty) — semantically equivalent to scope=Repo.
    pub project_is_root: bool,
}

impl ScopeApplied {
    pub fn to_json(&self) -> Value {
        json!({
            "applied": match self.scope {
                Scope::Project => "project",
                Scope::Repo => "repo",
                Scope::Umbrella => "umbrella",
                Scope::All => "all",
            },
            "root": self.root,
            "subdir": self.subdir,
            "umbrella": self.umbrella,
            "project_is_root": self.project_is_root,
        })
    }
}

/// Compose `user_filter` with a scope clause. Returns the new combined
/// filter (or `None` if both inputs are empty) and a `ScopeApplied`
/// describing what was added.
///
/// Errors when:
/// - scope=Project|Repo and no current project resolved
/// - scope=Umbrella and current project has no declared umbrella
pub fn apply_scope(
    user_filter: Option<FilterNode>,
    scope: Scope,
    ws: &WorkspaceConfig,
    current: Option<&CurrentProject>,
) -> Result<(Option<FilterNode>, ScopeApplied)> {
    let scope_clause = match scope {
        Scope::All => None,
        Scope::Project => {
            let cp = current.ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=project requires a resolved current project; cwd is outside all \
                     workspace roots. Pass scope=\"all\" to query everything."
                )
            })?;
            Some(project_clause(&cp.root, &cp.subdir))
        }
        Scope::Repo => {
            let cp = current.ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=repo requires a resolved current project; cwd is outside all \
                     workspace roots. Pass scope=\"all\" to query everything."
                )
            })?;
            Some(repo_clause(&cp.root))
        }
        Scope::Umbrella => {
            let cp = current.ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=umbrella requires a resolved current project. Pass scope=\"all\"."
                )
            })?;
            let umbrella_name = cp.umbrella.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=umbrella but no umbrella is declared for {}/{}. \
                     Add an [[umbrella]] block to workspace.toml or use scope=repo|all.",
                    cp.root,
                    cp.subdir
                )
            })?;
            let umbrella = ws
                .umbrellas
                .iter()
                .find(|u| u.name == umbrella_name)
                .ok_or_else(|| {
                    anyhow::anyhow!("umbrella `{umbrella_name}` not found in workspace config")
                })?;
            if umbrella.members.is_empty() {
                bail!("umbrella `{umbrella_name}` has no members");
            }
            Some(umbrella_clause(&umbrella.members)?)
        }
    };

    let combined = match (user_filter, scope_clause) {
        (Some(u), Some(s)) => Some(FilterNode::And { and: vec![u, s] }),
        (Some(u), None) => Some(u),
        (None, Some(s)) => Some(s),
        (None, None) => None,
    };

    let applied = ScopeApplied {
        scope,
        root: current.map(|c| c.root.clone()),
        subdir: current.map(|c| c.subdir.clone()),
        umbrella: current.and_then(|c| c.umbrella.clone()),
        project_is_root: current.is_some_and(|c| c.subdir.is_empty()),
    };

    Ok((combined, applied))
}

fn repo_clause(root: &str) -> FilterNode {
    FilterNode::Leaf(
        [("repo".to_string(), json!({"eq": root}))]
            .into_iter()
            .collect(),
    )
}

fn project_clause(root: &str, subdir: &str) -> FilterNode {
    let repo = repo_clause(root);
    if subdir.is_empty() {
        // Project IS the root — scope=project collapses to scope=repo.
        return repo;
    }
    let path_prefix = format!("{}/", subdir.trim_end_matches('/'));
    let prefix = FilterNode::Leaf(
        [("rel_path".to_string(), json!({"prefix": path_prefix}))]
            .into_iter()
            .collect(),
    );
    FilterNode::And {
        and: vec![repo, prefix],
    }
}

fn umbrella_clause(members: &[String]) -> Result<FilterNode> {
    let mut clauses = Vec::with_capacity(members.len());
    for m in members {
        let (root, subdir) = match m.split_once('/') {
            Some((r, s)) => (r, s),
            None => (m.as_str(), ""),
        };
        clauses.push(project_clause(root, subdir));
    }
    Ok(if clauses.len() == 1 {
        clauses.into_iter().next().unwrap()
    } else {
        FilterNode::Or { or: clauses }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{Root, Umbrella};

    fn ws(roots: Vec<Root>, umbrellas: Vec<Umbrella>) -> WorkspaceConfig {
        WorkspaceConfig {
            roots,
            ignore: vec![],
            rules: vec![],
            umbrellas,
        }
    }

    fn cp(root: &str, subdir: &str, umbrella: Option<&str>) -> CurrentProject {
        CurrentProject {
            root: root.into(),
            subdir: subdir.into(),
            umbrella: umbrella.map(Into::into),
            ..Default::default()
        }
    }

    #[test]
    fn project_scope_with_subdir_ands_repo_and_prefix() {
        let w = ws(vec![], vec![]);
        let cur = cp("mono", "svc-a", None);
        let (filter, applied) = apply_scope(None, Scope::Project, &w, Some(&cur)).unwrap();
        let f = filter.unwrap();
        match f {
            FilterNode::And { and } => assert_eq!(and.len(), 2),
            _ => panic!("expected And, got {f:?}"),
        }
        assert_eq!(applied.subdir.as_deref(), Some("svc-a"));
        assert!(!applied.project_is_root);
    }

    #[test]
    fn project_scope_with_empty_subdir_collapses_to_repo() {
        let w = ws(vec![], vec![]);
        let cur = cp("flat", "", None);
        let (filter, applied) = apply_scope(None, Scope::Project, &w, Some(&cur)).unwrap();
        // Should be a single leaf on repo, not an And.
        assert!(matches!(filter, Some(FilterNode::Leaf(_))));
        assert!(applied.project_is_root);
    }

    #[test]
    fn project_scope_without_current_project_errors() {
        let w = ws(vec![], vec![]);
        let err = apply_scope(None, Scope::Project, &w, None).unwrap_err();
        assert!(err.to_string().contains("scope=project"));
    }

    #[test]
    fn all_scope_passes_user_filter_through() {
        let w = ws(vec![], vec![]);
        let user = FilterNode::Leaf(
            [("kind".to_string(), json!({"eq": "tracker"}))]
                .into_iter()
                .collect(),
        );
        let (filter, applied) = apply_scope(Some(user.clone()), Scope::All, &w, None).unwrap();
        assert!(matches!(filter, Some(FilterNode::Leaf(_))));
        assert_eq!(applied.scope, Scope::All);
    }

    #[test]
    fn umbrella_scope_ors_member_clauses() {
        let w = ws(
            vec![],
            vec![Umbrella {
                name: "platform".into(),
                members: vec!["infra/svc-a".into(), "infra/svc-b".into()],
            }],
        );
        let cur = cp("infra", "svc-a", Some("platform"));
        let (filter, _) = apply_scope(None, Scope::Umbrella, &w, Some(&cur)).unwrap();
        match filter.unwrap() {
            FilterNode::Or { or } => assert_eq!(or.len(), 2),
            f => panic!("expected Or, got {f:?}"),
        }
    }

    #[test]
    fn umbrella_scope_without_umbrella_errors() {
        let w = ws(vec![], vec![]);
        let cur = cp("infra", "svc-a", None);
        let err = apply_scope(None, Scope::Umbrella, &w, Some(&cur)).unwrap_err();
        assert!(err.to_string().contains("umbrella"));
    }

    #[test]
    fn user_filter_and_scope_compose_via_and() {
        let w = ws(vec![], vec![]);
        let cur = cp("mono", "svc-a", None);
        let user = FilterNode::Leaf(
            [("kind".to_string(), json!({"eq": "tracker"}))]
                .into_iter()
                .collect(),
        );
        let (filter, _) = apply_scope(Some(user), Scope::Project, &w, Some(&cur)).unwrap();
        // Outer And combines user + scope
        match filter.unwrap() {
            FilterNode::And { and } => assert_eq!(and.len(), 2),
            f => panic!("expected outer And, got {f:?}"),
        }
    }
}
