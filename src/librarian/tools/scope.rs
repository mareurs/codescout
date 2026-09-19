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

use super::LibrarianRecoverableError;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::librarian::current_project::CurrentProject;
use crate::librarian::filter::FilterNode;
use crate::librarian::workspace::WorkspaceConfig;

/// The breadth a librarian query runs at.
///
/// **Deliberately has no `Default`.** The default is not a property of the
/// enum — it is a property of each SURFACE, and the two disagreed silently for
/// months: `find`/`context` took `Repo` from a `#[default]` attribute while
/// `link_scan` hardcoded `Project` and every documentation surface said
/// "project". Removing the derive forces each call site to name its own answer,
/// the same reason [`UmbrellaPolicy`] exists — see its doc comment. A missing
/// `#[default]` is a compile error at the call site; a wrong one is a silent
/// contract violation. See `docs/issues/archive/2026-08-27-scope-default-is-repo-not-project-across-four-doc-surfaces.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Project,
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
    /// `docs/issues/archive/2026-08-15-context-scope-all-crosses-umbrella-boundary.md`.
    Literal,
}

/// Resolve the caller's requested scope into the one the query actually runs
/// under, applying `policy` to an explicit `all`.
///
/// `default` is the scope to use when the caller passed none. It is a REQUIRED
/// argument rather than a trait-derived default: the answer differs per surface
/// and, when it lived in a `#[derive(Default)]`, drifted from every documented
/// claim about it without a single test noticing. Naming it here puts it in the
/// signature at each call site, where a reviewer reads it — the same argument
/// [`UmbrellaPolicy`] makes for the `all` policy.
///
/// Returns `(effective_scope, scope_fallback)`. `scope_fallback` is set when a
/// `project`/`repo` request was widened to `all` because no project is active;
/// callers surface it so a response can explain why the result set came back
/// broader than what was asked for.
///
/// Note the asymmetry preserved from the original: the umbrella GUARD keys off
/// the RAW `requested` (`Some(All)`), while the ALIAS keys off the DEFAULTED
/// value. That distinction is why changing `default` cannot change either —
/// `scope == All` after defaulting still implies the caller asked for `all`,
/// for any `default` that is not itself `All`.
pub fn resolve_scope(
    requested: Option<Scope>,
    current: Option<&CurrentProject>,
    policy: UmbrellaPolicy,
    default: Scope,
) -> Result<(Scope, bool)> {
    let scope = requested.unwrap_or(default);
    if policy == UmbrellaPolicy::Require && requested == Some(Scope::All) {
        if let Some(cp) = current {
            if cp.umbrella.is_none() {
                return Err(LibrarianRecoverableError::new(
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
            LibrarianRecoverableError::new(format!(
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
                // checkout's rows. This clause deliberately over-selects; the
                // CALLER owes the shadow-vs-main dedup, via
                // `worktree::shadowed_main_ids`. No caller may opt out
                // silently — an unlabelled duplicate is worse than either
                // dropping or labelling it.
                Some(main) => FilterNode::Or {
                    or: vec![path_prefix_clause(&cp.abs_path), path_prefix_clause(main)],
                },
                None => path_prefix_clause(&cp.abs_path),
            })
        }
        Scope::Repo => {
            let cp = require(current, "repo")?;
            Some(match &cp.main_root {
                // Same over-selection as Scope::Project above, same caller
                // obligation: dedup via `worktree::shadowed_main_ids`.
                Some(main) => FilterNode::Or {
                    or: vec![path_prefix_clause(&cp.git_root), path_prefix_clause(main)],
                },
                None => path_prefix_clause(&cp.git_root),
            })
        }
        Scope::Umbrella => {
            let cp = require(current, "umbrella")?;
            let umbrella_name = cp.umbrella.as_deref().ok_or_else(|| {
                LibrarianRecoverableError::new(format!(
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
                    LibrarianRecoverableError::new(format!("umbrella `{umbrella_name}` not found"))
                })?;
            if umb.members.is_empty() {
                return Err(LibrarianRecoverableError::new(format!(
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
    // verbatim in src/librarian/tools/find.rs and
    // src/librarian/tools/workspace_state_at.rs, and `Literal` existed only as
    // the ABSENCE of that block in src/librarian/tools/context.rs — which is
    // exactly why a deliberate choice was indistinguishable from a truncated
    // copy.

    #[test]
    fn require_policy_refuses_all_when_the_project_has_no_umbrella() {
        let c = cp("/w/p", "/w/p", None);
        let err = resolve_scope(
            Some(Scope::All),
            Some(&c),
            UmbrellaPolicy::Require,
            Scope::Project,
        )
        .unwrap_err();
        assert!(err.to_string().contains("umbrella"), "got: {err}");
    }

    #[test]
    fn require_policy_aliases_all_to_umbrella_when_one_is_configured() {
        let c = cp("/w/p", "/w/p", Some("main"));
        let (scope, fallback) = resolve_scope(
            Some(Scope::All),
            Some(&c),
            UmbrellaPolicy::Require,
            Scope::Project,
        )
        .unwrap();
        assert_eq!(scope, Scope::Umbrella);
        assert!(!fallback);
    }

    #[test]
    fn literal_policy_keeps_all_as_all_even_with_an_umbrella() {
        // The behaviour `librarian(action="context")` is built on: an explicit
        // `all` reaches every project, umbrella or not. Intentional — confirmed by
        // a live A/B against the running server, then by the owner. See
        // docs/issues/archive/2026-08-15-context-scope-all-crosses-umbrella-boundary.md.
        let c = cp("/w/p", "/w/p", Some("main"));
        let (scope, fallback) = resolve_scope(
            Some(Scope::All),
            Some(&c),
            UmbrellaPolicy::Literal,
            Scope::Project,
        )
        .unwrap();
        assert_eq!(scope, Scope::All);
        assert!(!fallback);
    }

    #[test]
    fn the_two_policies_differ_on_exactly_one_input() {
        // The discriminating pair: identical inputs, opposite outcomes, and the
        // only difference is the policy named at the call site.
        let c = cp("/w/p", "/w/p", None);
        assert!(resolve_scope(
            Some(Scope::All),
            Some(&c),
            UmbrellaPolicy::Require,
            Scope::Project
        )
        .is_err());
        let (scope, _) = resolve_scope(
            Some(Scope::All),
            Some(&c),
            UmbrellaPolicy::Literal,
            Scope::Project,
        )
        .unwrap();
        assert_eq!(scope, Scope::All);
    }

    #[test]
    fn project_and_repo_fall_back_to_all_without_a_current_project() {
        for policy in [UmbrellaPolicy::Require, UmbrellaPolicy::Literal] {
            for requested in [Scope::Project, Scope::Repo] {
                let (scope, fallback) =
                    resolve_scope(Some(requested), None, policy, Scope::Project).unwrap();
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
                let r = resolve_scope(requested, current, UmbrellaPolicy::Require, Scope::Project)
                    .expect("Require must succeed when `all` was not requested");
                let l = resolve_scope(requested, current, UmbrellaPolicy::Literal, Scope::Project)
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
             should call resolve_scope(requested, current, UmbrellaPolicy::_, Scope::Project) \
             rather than re-inlining the match — found: {hits:?}"
        );
    }

    /// Assemble the `resolve_scope(` call needle character-wise so this
    /// test's own source never contains the substring literally -- see the
    /// doc comment on `every_resolve_scope_call_names_project_as_its_default`
    /// for why that matters (the guard would otherwise find itself).
    fn call_needle() -> String {
        [
            'r', 'e', 's', 'o', 'l', 'v', 'e', '_', 's', 'c', 'o', 'p', 'e', '(',
        ]
        .into_iter()
        .collect()
    }

    /// Assemble the `Scope::Project` needle character-wise for the same
    /// reason as `call_needle`.
    fn want_needle() -> String {
        [
            'S', 'c', 'o', 'p', 'e', ':', ':', 'P', 'r', 'o', 'j', 'e', 'c', 't',
        ]
        .into_iter()
        .collect()
    }

    /// Scan `content` for occurrences of `call` (expected to be
    /// `resolve_scope(`), other than its own definition or a commented-out
    /// mention, and report the 1-based line number of any whose default
    /// argument does not name `want` within a **fixed 240-byte window**
    /// after the call token.
    ///
    /// This is the ORIGINAL predicate from
    /// `docs/issues/2026-09-09-a-fixed-byte-window-source-guard-reports-a-correct-call-site.md`
    /// (bug `82b3c70313fb754b`), kept ONLY so the two direction tests below
    /// can demonstrate the defect against a deterministic fixture instead of
    /// mutating a production call site. It is deliberately not wired into
    /// `every_resolve_scope_call_names_project_as_its_default` any more.
    fn offending_resolve_scope_lines_by_byte_window(
        content: &str,
        call: &str,
        want: &str,
    ) -> (usize, Vec<u32>) {
        let mut offenders = Vec::new();
        let mut checked = 0usize;
        for (idx, _) in content.match_indices(call) {
            let line_start = content[..idx].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line_prefix = &content[line_start..idx];
            if line_prefix.contains("fn ") || line_prefix.trim_start().starts_with("//") {
                continue;
            }
            checked += 1;
            let mut end = (idx + 240).min(content.len());
            while !content.is_char_boundary(end) {
                end -= 1;
            }
            if !content[idx..end].contains(want) {
                let line_no = content[..idx].matches('\n').count() as u32 + 1;
                offenders.push(line_no);
            }
        }
        (checked, offenders)
    }

    /// Extract the trimmed source text of a call's **last** positional
    /// argument, given `content` and the byte index of the call's opening
    /// `(`. `//` and `/* */` comments and string-literal contents are
    /// treated as opaque and never contribute to the returned text, so a
    /// comment next to the real argument cannot be mistaken for it in
    /// either direction. Returns `None` if the parens never balance (or a
    /// stray `]`/`}` closes at the call's own depth) before the content
    /// ends -- malformed input, which should not occur in a real `.rs` file.
    fn last_call_arg(content: &str, open_paren: usize) -> Option<String> {
        debug_assert_eq!(content.as_bytes().get(open_paren), Some(&b'('));
        let mut depth: i32 = 0;
        let mut current = String::new();
        let mut args: Vec<String> = Vec::new();
        let mut j = open_paren + 1;
        while j < content.len() {
            let rest = &content[j..];
            if let Some(after) = rest.strip_prefix("//") {
                return match after.find('\n') {
                    Some(nl) => {
                        j += 2 + nl;
                        continue;
                    }
                    None => None, // line comment runs to EOF: unterminated call
                };
            }
            if let Some(after) = rest.strip_prefix("/*") {
                // Unterminated block comment => the call has no closing delimiter, so
                // `?` propagates the same `None` the old `return None` arm did.
                let end = after.find("*/")?;
                j += 2 + end + 2;
                continue;
            }
            let c = rest.chars().next()?;
            let clen = c.len_utf8();
            if c == '"' {
                j += clen;
                while j < content.len() {
                    let c2 = content[j..].chars().next()?;
                    let c2len = c2.len_utf8();
                    j += c2len;
                    if c2 == '\\' {
                        if j < content.len() {
                            j += content[j..]
                                .chars()
                                .next()
                                .map(|c| c.len_utf8())
                                .unwrap_or(0);
                        }
                        continue;
                    }
                    if c2 == '"' {
                        break;
                    }
                }
                continue;
            }
            match c {
                '(' | '[' | '{' => {
                    depth += 1;
                    current.push(c);
                    j += clen;
                }
                ')' | ']' | '}' => {
                    if depth == 0 {
                        if c != ')' {
                            return None; // mismatched delimiter at the call's own depth
                        }
                        let last = current.trim();
                        if !last.is_empty() {
                            args.push(last.to_string());
                        }
                        return args.pop();
                    }
                    depth -= 1;
                    current.push(c);
                    j += clen;
                }
                ',' if depth == 0 => {
                    args.push(current.trim().to_string());
                    current.clear();
                    j += clen;
                }
                _ => {
                    current.push(c);
                    j += clen;
                }
            }
        }
        None
    }

    /// Scan `content` for occurrences of `call` (expected to be
    /// `resolve_scope(`), other than its own definition or a commented-out
    /// mention, and report the 1-based line number of any whose **last
    /// argument** -- parsed to the call's closing delimiter, comments and
    /// string contents excluded -- is not `want`, bare (`Scope::Project`) or
    /// path-qualified (`super::scope::Scope::Project`). Returns
    /// `(checked, offenders)`; `checked` backs the guard's positive control.
    fn offending_resolve_scope_lines(content: &str, call: &str, want: &str) -> (usize, Vec<u32>) {
        let mut offenders = Vec::new();
        let mut checked = 0usize;
        for (idx, _) in content.match_indices(call) {
            let line_start = content[..idx].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line_prefix = &content[line_start..idx];
            if line_prefix.contains("fn ") || line_prefix.trim_start().starts_with("//") {
                continue;
            }
            checked += 1;
            let open_paren = idx + call.len() - 1;
            let names_default = match last_call_arg(content, open_paren) {
                Some(arg) => {
                    arg == want
                        || (arg.ends_with(want) && arg[..arg.len() - want.len()].ends_with("::"))
                }
                None => false,
            };
            if !names_default {
                let line_no = content[..idx].matches('\n').count() as u32 + 1;
                offenders.push(line_no);
            }
        }
        (checked, offenders)
    }

    /// Gate: every `resolve_scope` CALL in the tree names `Scope::Project` as its
    /// default.
    ///
    /// `Scope` deliberately has no `Default` (see its doc comment), so the default
    /// is chosen per call site — which is what makes it reviewable, and also what
    /// makes a fourth handler free to pick a different one. Before the derive was
    /// removed, three surfaces disagreed in exactly this way and nothing failed:
    /// `find`/`context`/`state_at` took `Repo` from the attribute, `link_scan`
    /// hardcoded `Project`, `reindex` picked `Project`-or-`All`, and all four
    /// documentation surfaces said "project".
    ///
    /// Scans source text rather than behaviour on purpose: a test that calls
    /// `resolve_scope(None, .., Scope::Project)` and asserts it returns `Project`
    /// is computed from the thing it judges and cannot fail.
    ///
    /// The needles are assembled character-wise so this test's own source does not
    /// match them.
    ///
    /// The predicate itself parses each call's argument list to its closing
    /// delimiter and checks the **last argument** (`offending_resolve_scope_lines`
    /// / `last_call_arg`), rather than asking whether `Scope::Project` merely
    /// *appears* within a fixed byte window of the call token — see
    /// `docs/issues/2026-09-09-a-fixed-byte-window-source-guard-reports-a-correct-call-site.md`
    /// (bug `82b3c70313fb754b`). That bug's two failure directions are exercised
    /// directly, without mutating a production call site, by
    /// `a_correct_default_survives_a_long_comment_inside_the_argument_list` and
    /// `a_wrong_default_is_reported_despite_a_mentioning_comment_nearby` below.
    #[test]
    fn every_resolve_scope_call_names_project_as_its_default() {
        let call = call_needle();
        let want = want_needle();
        let root = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
        let mut offenders: Vec<String> = Vec::new();
        let mut checked = 0usize;
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
            let (file_checked, file_offenders) =
                offending_resolve_scope_lines(&content, &call, &want);
            checked += file_checked;
            if !file_offenders.is_empty() {
                let rel = path.strip_prefix(&root).unwrap_or(path);
                let rel_str = rel.display().to_string().replace('\\', "/");
                offenders.extend(
                    file_offenders
                        .into_iter()
                        .map(|line_no| format!("{rel_str}:{line_no}")),
                );
            }
        }
        // Positive control: a scan that matched nothing must not read as a pass.
        // There are at least three production call sites (find, context,
        // workspace_state_at); a lower count means the needle stopped matching,
        // not that the tree got cleaner.
        assert!(
            checked >= 3,
            "found only {checked} resolve_scope call site(s) — the SCAN is broken, \
             not the code"
        );
        assert!(
            offenders.is_empty(),
            "every resolve_scope call must name Scope::Project as its default — the \
             librarian's documented default on every user-facing surface. A handler \
             that wants a different breadth should say so in its own doc comment and \
             this gate should be widened deliberately. Offenders: {offenders:?}"
        );
    }

    /// Direction 1 of bug `82b3c70313fb754b`: a correct call whose default
    /// argument is preceded by a multi-line explanatory comment inside the
    /// argument list must not be reported. Under the old 240-byte-window
    /// predicate this comment alone pushes `Scope::Project` past the window
    /// (measured at offset 400 in the bug file); the argument-parsing
    /// predicate does not care how long the comment is, because it excludes
    /// comments before looking at the argument at all.
    #[test]
    fn a_correct_default_survives_a_long_comment_inside_the_argument_list() {
        let call = call_needle();
        let want = want_needle();
        let content = format!(
            "let (effective_scope, scope_fallback) = {call}\n    \
             None,\n    \
             current,\n    \
             UmbrellaPolicy::Require,\n    \
             // Explaining, at some length, exactly why this call opts into\n    \
             // the project default rather than something wider -- the kind\n    \
             // of comment a reviewer asks an author to leave at a call site,\n    \
             // and exactly the shape that pushed a correct call past a fixed\n    \
             // 240-byte window in the bug this test guards against.\n    \
             {want},\n\
             )?;\n"
        );
        let (checked, offenders) = offending_resolve_scope_lines(&content, &call, &want);
        assert_eq!(checked, 1, "fixture must contain exactly one call site");
        assert!(
            offenders.is_empty(),
            "a correct call must not be reported merely because a comment in its \
             argument list is long: {offenders:?}"
        );
    }

    /// Direction 2 of bug `82b3c70313fb754b` -- the expensive one: a call
    /// site with the WRONG default and a comment merely *mentioning*
    /// `Scope::Project` nearby must still be reported. This is the missing
    /// negative case the bug file names under § Fix: "a fixture call site
    /// with a wrong default plus a nearby mention, asserted to be reported."
    #[test]
    fn a_wrong_default_is_reported_despite_a_mentioning_comment_nearby() {
        let call = call_needle();
        let want = want_needle();
        let content = format!(
            "let (effective_scope, scope_fallback) = {call}\n    \
             None,\n    \
             current,\n    \
             UmbrellaPolicy::Require,\n    \
             // MUTATION PROBE -- unlike {want}, this surface wants the repo.\n    \
             Scope::Repo,\n\
             )?;\n"
        );
        let (checked, offenders) = offending_resolve_scope_lines(&content, &call, &want);
        assert_eq!(checked, 1, "fixture must contain exactly one call site");
        assert_eq!(
            offenders.len(),
            1,
            "a wrong default must be reported even when a comment nearby mentions \
             the right one: {offenders:?}"
        );
    }

    /// Reproduces bug `82b3c70313fb754b` against the ORIGINAL byte-window
    /// predicate (kept only as `offending_resolve_scope_lines_by_byte_window`,
    /// not wired into the live guard), so the fix above has a red to point at
    /// without ever mutating a production call site. Both fixtures are shared
    /// with the two tests above; only the predicate under test differs.
    #[test]
    fn the_byte_window_predicate_is_wrong_in_both_directions() {
        let call = call_needle();
        let want = want_needle();

        let correct_call_long_comment = format!(
            "let (effective_scope, scope_fallback) = {call}\n    \
             None,\n    \
             current,\n    \
             UmbrellaPolicy::Require,\n    \
             // Explaining, at some length, exactly why this call opts into\n    \
             // the project default rather than something wider -- the kind\n    \
             // of comment a reviewer asks an author to leave at a call site,\n    \
             // and exactly the shape that pushed a correct call past a fixed\n    \
             // 240-byte window in the bug this test guards against.\n    \
             {want},\n\
             )?;\n"
        );
        let (_, byte_window_offenders) =
            offending_resolve_scope_lines_by_byte_window(&correct_call_long_comment, &call, &want);
        assert!(
            !byte_window_offenders.is_empty(),
            "direction 1: the byte-window predicate was expected to (wrongly) flag \
             this correct call -- if it no longer does, the fixture stopped \
             reproducing the bug and this test should be revisited, not deleted"
        );

        let wrong_default_mentioning_comment = format!(
            "let (effective_scope, scope_fallback) = {call}\n    \
             None,\n    \
             current,\n    \
             UmbrellaPolicy::Require,\n    \
             // MUTATION PROBE -- unlike {want}, this surface wants the repo.\n    \
             Scope::Repo,\n\
             )?;\n"
        );
        let (_, byte_window_offenders_2) = offending_resolve_scope_lines_by_byte_window(
            &wrong_default_mentioning_comment,
            &call,
            &want,
        );
        assert!(
            byte_window_offenders_2.is_empty(),
            "direction 2: the byte-window predicate was expected to (wrongly) admit \
             this wrong default -- if it no longer does, the fixture stopped \
             reproducing the bug and this test should be revisited, not deleted"
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
