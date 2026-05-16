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

use crate::librarian::current_project::CurrentProject;
use crate::librarian::filter::FilterNode;
use crate::librarian::workspace::WorkspaceConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Project,
    #[default]
    Repo,
    Umbrella,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeApplied {
    pub scope: Scope,
    pub abs_path: Option<std::path::PathBuf>,
    pub git_root: Option<std::path::PathBuf>,
    pub umbrella: Option<String>,
}

impl ScopeApplied {
    pub fn to_json(&self) -> Value {
        json!({
            "applied": match self.scope {
                Scope::All => "all", Scope::Project => "project",
                Scope::Repo => "repo", Scope::Umbrella => "umbrella",
            },
            "abs_path": self.abs_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            "git_root": self.git_root.as_ref().map(|p| p.to_string_lossy().to_string()),
            "umbrella": self.umbrella,
        })
    }
}

pub fn apply_scope(
    user_filter: Option<FilterNode>,
    scope: Scope,
    ws: &WorkspaceConfig,
    current: Option<&CurrentProject>,
) -> Result<(Option<FilterNode>, ScopeApplied)> {
    fn require<'a>(
        current: Option<&'a CurrentProject>,
        scope_name: &str,
    ) -> Result<&'a CurrentProject> {
        current.ok_or_else(|| {
            anyhow::anyhow!(
                "scope={} requires an active project. The host has not activated one \
             (call workspace(action='activate', path=...)).",
                scope_name
            )
        })
    }

    let scope_clause = match scope {
        Scope::All => None,
        Scope::Project => {
            let cp = require(current, "project")?;
            Some(path_prefix_clause(&cp.abs_path))
        }
        Scope::Repo => {
            let cp = require(current, "repo")?;
            Some(path_prefix_clause(&cp.git_root))
        }
        Scope::Umbrella => {
            let cp = require(current, "umbrella")?;
            let umbrella_name = cp.umbrella.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=umbrella but no umbrella declared for {}. \
                     Add a [[umbrella]] block to workspace.toml or use scope=repo|all.",
                    cp.abs_path.display(),
                )
            })?;
            let umb = ws
                .umbrellas
                .iter()
                .find(|u| u.name == umbrella_name)
                .ok_or_else(|| anyhow::anyhow!("umbrella `{umbrella_name}` not found"))?;
            if umb.members.is_empty() {
                bail!("umbrella `{umbrella_name}` has no members");
            }
            Some(or_of_prefixes(&umb.members))
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
        abs_path: current.map(|c| c.abs_path.clone()),
        git_root: current.map(|c| c.git_root.clone()),
        umbrella: current.and_then(|c| c.umbrella.clone()),
    };

    Ok((combined, applied))
}

fn path_prefix_clause(p: &std::path::Path) -> FilterNode {
    let s = p.to_string_lossy().to_string();
    let prefix = format!("{s}/");
    FilterNode::Or {
        or: vec![
            FilterNode::Leaf(
                [("abs_path".to_string(), json!({"eq": s.clone()}))]
                    .into_iter()
                    .collect(),
            ),
            FilterNode::Leaf(
                [("abs_path".to_string(), json!({"prefix": prefix}))]
                    .into_iter()
                    .collect(),
            ),
        ],
    }
}

fn or_of_prefixes(members: &[std::path::PathBuf]) -> FilterNode {
    FilterNode::Or {
        or: members.iter().map(|m| path_prefix_clause(m)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::workspace::{Root, Umbrella};

    fn ws(roots: Vec<Root>, umbrellas: Vec<Umbrella>) -> WorkspaceConfig {
        WorkspaceConfig {
            roots,
            ignore: vec![],
            rules: vec![],
            umbrellas,
        }
    }

    fn cp(abs_path: &str, git_root: &str, umbrella: Option<&str>) -> CurrentProject {
        CurrentProject {
            abs_path: std::path::PathBuf::from(abs_path),
            git_root: std::path::PathBuf::from(git_root),
            umbrella: umbrella.map(str::to_string),
        }
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
