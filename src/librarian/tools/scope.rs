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

use super::RecoverableError;
use anyhow::Result;
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

/// How a handler treats an explicit `scope="all"`.
///
/// Both answers below are correct — for different kinds of surface. The
/// difference used to live in whether a handler had copied the umbrella block
/// or not, which made a deliberate choice indistinguishable from a truncated
/// copy; naming it puts the choice in the signature at every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UmbrellaPolicy {
    /// Search surfaces (`find`, `workspace_state_at`). `all` is a request to
    /// widen, so refuse it when there is no umbrella to widen *to*, and read it
    /// as `umbrella` when there is: someone looking for their own work should
    /// not silently receive every unrelated project in the workspace.
    Require,
    /// Orientation surfaces (`context`). `all` means all — reaching across every
    /// project is the point of the tool, so no umbrella is imposed. Deliberate,
    /// not an omission; see
    /// `docs/issues/2026-08-15-context-scope-all-crosses-umbrella-boundary.md`.
    Literal,
}

/// Resolve the caller's requested scope into the one the query actually runs
/// under, applying `policy` to an explicit `all`.
///
/// Returns `(effective_scope, scope_fallback)`. `scope_fallback` is set when a
/// `project`/`repo` request was widened to `all` because no project is active;
/// callers surface it so a response can explain why the result set came back
/// broader than what was asked for.
pub fn resolve_scope(
    requested: Option<Scope>,
    current: Option<&CurrentProject>,
    policy: UmbrellaPolicy,
) -> Result<(Scope, bool)> {
    let scope = requested.unwrap_or_default();
    if policy == UmbrellaPolicy::Require && requested == Some(Scope::All) {
        if let Some(cp) = current {
            if cp.umbrella.is_none() {
                return Err(RecoverableError::new(
                    "scope=\"all\" requires a configured umbrella — without one it crosses into \
                     unrelated workspace projects. Use scope=\"repo\" to widen to your repo, or \
                     configure [[umbrella]] in workspace.toml to group related projects.",
                ));
            }
        }
    }
    // scope=all is an alias for umbrella when the current project has one;
    // without a current project or umbrella, All passes through (no-cwd fallback path).
    let scope = if policy == UmbrellaPolicy::Require
        && scope == Scope::All
        && current.and_then(|c| c.umbrella.as_deref()).is_some()
    {
        Scope::Umbrella
    } else {
        scope
    };
    Ok(match (scope, current.is_some()) {
        (Scope::Project | Scope::Repo, false) => (Scope::All, true),
        (s, _) => (s, false),
    })
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
    exclude_worktrees: &[String],
) -> Result<(Option<FilterNode>, ScopeApplied)> {
    fn require<'a>(
        current: Option<&'a CurrentProject>,
        scope_name: &str,
    ) -> Result<&'a CurrentProject> {
        current.ok_or_else(|| {
            RecoverableError::new(format!(
                "scope={} requires an active project. The host has not activated one \
             (call workspace(action='activate', path=...)).",
                scope_name
            ))
        })
    }

    let scope_clause = match scope {
        Scope::All => None,
        Scope::Project => {
            let cp = require(current, "project")?;
            Some(match &cp.main_root {
                // Overlay: a worktree session sees its own rows AND the main
                // checkout's rows; shadow-vs-main dedup happens post-query in find.
                Some(main) => FilterNode::Or {
                    or: vec![path_prefix_clause(&cp.abs_path), path_prefix_clause(main)],
                },
                None => path_prefix_clause(&cp.abs_path),
            })
        }
        Scope::Repo => {
            let cp = require(current, "repo")?;
            Some(match &cp.main_root {
                // Overlay: a worktree session sees its own rows AND the main
                // checkout's rows; shadow-vs-main dedup happens post-query in find.
                Some(main) => FilterNode::Or {
                    or: vec![path_prefix_clause(&cp.git_root), path_prefix_clause(main)],
                },
                None => path_prefix_clause(&cp.git_root),
            })
        }
        Scope::Umbrella => {
            let cp = require(current, "umbrella")?;
            let umbrella_name = cp.umbrella.as_deref().ok_or_else(|| {
                RecoverableError::new(format!(
                    "scope=umbrella but no umbrella declared for {}. \
                     Add a [[umbrella]] block to workspace.toml or use scope=repo|all.",
                    cp.abs_path.display(),
                ))
            })?;
            let umb = ws
                .umbrellas
                .iter()
                .find(|u| u.name == umbrella_name)
                .ok_or_else(|| {
                    RecoverableError::new(format!("umbrella `{umbrella_name}` not found"))
                })?;
            if umb.members.is_empty() {
                return Err(RecoverableError::new(format!(
                    "umbrella `{umbrella_name}` has no members"
                )));
            }
            Some(or_of_prefixes(&umb.members))
        }
    };

    // Shadow rows belong to their worktree's overlay: every other session
    // excludes them. (In-repo layouts like <main>/.worktrees/<n> would
    // otherwise match the main prefix.)
    let scope_clause = match (scope_clause, exclude_worktrees.is_empty()) {
        (Some(sc), false) => Some(FilterNode::And {
            and: vec![
                sc,
                FilterNode::Not {
                    not: Box::new(or_of_prefix_strings(exclude_worktrees)),
                },
            ],
        }),
        (sc, _) => sc,
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
    // Forward-slash normalize so the filter matches catalog rows (which are
    // stored in forward-slash form via artifact::upsert), regardless of which
    // platform built the path.
    let s = crate::util::fs::RepoPath::from(p).into_string();
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

// Sibling of `or_of_prefixes` over `&[String]` — `exclude_worktrees` carries
// forward-slash root strings (from `worktree::active_roots`), not the
// `PathBuf` umbrella-member list `or_of_prefixes` takes.
fn or_of_prefix_strings(roots: &[String]) -> FilterNode {
    FilterNode::Or {
        or: roots
            .iter()
            .map(|s| path_prefix_clause(std::path::Path::new(s)))
            .collect(),
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
            main_root: None,
            umbrella: umbrella.map(str::to_string),
        }
    }

    fn cp_wt(abs_path: &str, git_root: &str, main_root: &str) -> CurrentProject {
        CurrentProject {
            abs_path: abs_path.into(),
            git_root: git_root.into(),
            main_root: Some(main_root.into()),
            umbrella: None,
        }
    }

    #[test]
    fn project_scope_without_current_project_errors() {
        let w = ws(vec![], vec![]);
        let err = apply_scope(None, Scope::Project, &w, None, &[]).unwrap_err();
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
        let (filter, applied) = apply_scope(Some(user.clone()), Scope::All, &w, None, &[]).unwrap();
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
        let (filter, _) = apply_scope(None, Scope::Umbrella, &w, Some(&cur), &[]).unwrap();
        match filter.unwrap() {
            FilterNode::Or { or } => assert_eq!(or.len(), 2),
            f => panic!("expected Or, got {f:?}"),
        }
    }

    #[test]
    fn umbrella_scope_without_umbrella_errors() {
        let w = ws(vec![], vec![]);
        let cur = cp("infra", "svc-a", None);
        let err = apply_scope(None, Scope::Umbrella, &w, Some(&cur), &[]).unwrap_err();
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
        let (filter, _) = apply_scope(Some(user), Scope::Project, &w, Some(&cur), &[]).unwrap();
        // Outer And combines user + scope
        match filter.unwrap() {
            FilterNode::And { and } => assert_eq!(and.len(), 2),
            f => panic!("expected outer And, got {f:?}"),
        }
    }

    #[test]
    fn worktree_project_scope_unions_worktree_and_main_prefixes() {
        let ws = ws(vec![], vec![]);
        let current = cp_wt("/repo/.worktrees/feat", "/repo/.worktrees/feat", "/repo");
        let (f, _) = apply_scope(None, Scope::Project, &ws, Some(&current), &[]).unwrap();
        let s = serde_json::to_string(&f.unwrap()).unwrap();
        assert!(
            s.contains("/repo/.worktrees/feat/"),
            "worktree prefix present: {s}"
        );
        assert!(
            s.contains(r#""prefix":"/repo/""#),
            "main prefix present: {s}"
        );
    }

    #[test]
    fn worktree_repo_scope_unions_worktree_and_main_prefixes() {
        let ws = ws(vec![], vec![]);
        let current = cp_wt("/repo/.worktrees/feat", "/repo/.worktrees/feat", "/repo");
        let (f, _) = apply_scope(None, Scope::Repo, &ws, Some(&current), &[]).unwrap();
        let s = serde_json::to_string(&f.unwrap()).unwrap();
        assert!(
            s.contains("/repo/.worktrees/feat/"),
            "worktree prefix present: {s}"
        );
        assert!(
            s.contains(r#""prefix":"/repo/""#),
            "main prefix present: {s}"
        );
    }

    #[test]
    fn exclusion_wraps_scope_with_not_prefix() {
        let ws = ws(vec![], vec![]);
        let current = cp("/repo", "/repo", None);
        let (f, _) = apply_scope(
            None,
            Scope::Project,
            &ws,
            Some(&current),
            &["/repo/.worktrees/feat".to_string()],
        )
        .unwrap();
        let s = serde_json::to_string(&f.unwrap()).unwrap();
        assert!(s.contains(r#""not""#), "NOT clause present: {s}");
        assert!(
            s.contains("/repo/.worktrees/feat/"),
            "excluded prefix present: {s}"
        );
    }

    // ---- resolve_scope ------------------------------------------------------
    //
    // This matrix is the behaviour-preservation record for the extraction that
    // produced `resolve_scope` (SD-10). Before it, the `Require` behaviour sat
    // verbatim in find.rs and workspace_state_at.rs, and `Literal` existed only
    // as the ABSENCE of that block in context.rs — which is exactly why a
    // deliberate choice was indistinguishable from a truncated copy.

    #[test]
    fn require_policy_refuses_all_when_the_project_has_no_umbrella() {
        let c = cp("/w/p", "/w/p", None);
        let err = resolve_scope(Some(Scope::All), Some(&c), UmbrellaPolicy::Require).unwrap_err();
        assert!(err.to_string().contains("umbrella"), "got: {err}");
    }

    #[test]
    fn require_policy_aliases_all_to_umbrella_when_one_is_configured() {
        let c = cp("/w/p", "/w/p", Some("main"));
        let (scope, fallback) =
            resolve_scope(Some(Scope::All), Some(&c), UmbrellaPolicy::Require).unwrap();
        assert_eq!(scope, Scope::Umbrella);
        assert!(!fallback);
    }

    #[test]
    fn literal_policy_keeps_all_as_all_even_with_an_umbrella() {
        // The behaviour `librarian(action="context")` is built on: an explicit
        // `all` reaches every project, umbrella or not. Intentional — confirmed by
        // a live A/B against the running server, then by the owner. See
        // docs/issues/2026-08-15-context-scope-all-crosses-umbrella-boundary.md.
        let c = cp("/w/p", "/w/p", Some("main"));
        let (scope, fallback) =
            resolve_scope(Some(Scope::All), Some(&c), UmbrellaPolicy::Literal).unwrap();
        assert_eq!(scope, Scope::All);
        assert!(!fallback);
    }

    #[test]
    fn the_two_policies_differ_on_exactly_one_input() {
        // The discriminating pair: identical inputs, opposite outcomes, and the
        // only difference is the policy named at the call site.
        let c = cp("/w/p", "/w/p", None);
        assert!(resolve_scope(Some(Scope::All), Some(&c), UmbrellaPolicy::Require).is_err());
        let (scope, _) =
            resolve_scope(Some(Scope::All), Some(&c), UmbrellaPolicy::Literal).unwrap();
        assert_eq!(scope, Scope::All);
    }

    #[test]
    fn project_and_repo_fall_back_to_all_without_a_current_project() {
        for policy in [UmbrellaPolicy::Require, UmbrellaPolicy::Literal] {
            for requested in [Scope::Project, Scope::Repo] {
                let (scope, fallback) = resolve_scope(Some(requested), None, policy).unwrap();
                assert_eq!(scope, Scope::All, "{requested:?} under {policy:?}");
                assert!(
                    fallback,
                    "{requested:?} under {policy:?} must flag the fallback"
                );
            }
        }
    }

    #[test]
    fn policies_agree_on_every_input_except_an_explicit_all() {
        // The parity half. The two surfaces are permitted to differ on ONE input;
        // an edit that makes them differ on any other is a regression. Written as a
        // sweep rather than per-case because the failure this guards against is
        // precisely a divergence nobody enumerated.
        let with = cp("/w/p", "/w/p", Some("main"));
        let without = cp("/w/p", "/w/p", None);
        for current in [None, Some(&with), Some(&without)] {
            for requested in [
                None,
                Some(Scope::Project),
                Some(Scope::Repo),
                Some(Scope::Umbrella),
            ] {
                let r = resolve_scope(requested, current, UmbrellaPolicy::Require)
                    .expect("Require must succeed when `all` was not requested");
                let l = resolve_scope(requested, current, UmbrellaPolicy::Literal)
                    .expect("Literal must succeed when `all` was not requested");
                assert_eq!(
                    r,
                    l,
                    "policies diverged on requested={requested:?} umbrella={:?}",
                    current.and_then(|c| c.umbrella.as_deref())
                );
            }
        }
    }

    /// DRY gate: the scope-resolution fallback arm must appear exactly once in the
    /// tree — inside `resolve_scope`.
    ///
    /// Before the extraction (SD-10) this arm sat verbatim in three handlers, two
    /// of which carried the umbrella block above it while the third did not.
    /// Nothing distinguished that third case from a truncated copy, and settling it
    /// took a live A/B against the running server plus an owner ruling. Naming the
    /// difference as `UmbrellaPolicy` is what makes it declarable; this gate is
    /// what stops a fourth handler re-inlining the arm and re-creating the
    /// ambiguity.
    ///
    /// The needle is assembled character-wise so this test's own source does not
    /// match it.
    #[test]
    fn scope_fallback_arm_is_not_inlined_outside_resolve_scope() {
        let needle: String = [
            'S', 'c', 'o', 'p', 'e', ':', ':', 'P', 'r', 'o', 'j', 'e', 'c', 't', ' ', '|', ' ',
            'S', 'c', 'o', 'p', 'e', ':', ':', 'R', 'e', 'p', 'o', ',', ' ', 'f', 'a', 'l', 's',
            'e', ')',
        ]
        .into_iter()
        .collect();
        let root = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
        let mut hits: Vec<String> = Vec::new();
        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            let count = content.matches(needle.as_str()).count();
            if count > 0 {
                let rel = path.strip_prefix(&root).unwrap_or(path);
                hits.push(format!(
                    "{} ({count})",
                    rel.display().to_string().replace('\\', "/")
                ));
            }
        }
        assert_eq!(
            hits,
            vec!["librarian/tools/scope.rs (1)".to_string()],
            "the scope fallback arm must live only in resolve_scope; new handlers \
         should call resolve_scope(requested, current, UmbrellaPolicy::_) rather \
         than re-inlining the match — found: {hits:?}"
        );
    }

    #[test]
    fn no_exclusion_clause_when_list_empty() {
        let ws = ws(vec![], vec![]);
        let current = cp("/repo", "/repo", None);
        let (f, _) = apply_scope(None, Scope::Project, &ws, Some(&current), &[]).unwrap();
        assert!(!serde_json::to_string(&f.unwrap())
            .unwrap()
            .contains(r#""not""#));
    }
}
