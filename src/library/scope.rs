/// Scope for library-aware tool queries (`symbols`, `references`,
/// `semantic_search`, `index`). A different axis from `doc`/`librarian`'s
/// `scope` (`project|repo|umbrella|all`) — same word, unrelated vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Only project code (default)
    Project,
    /// Only registered libraries
    Libraries,
    /// Project + all libraries
    All,
    /// A specific library by name
    Library(String),
}

/// Rendered accepted-set for a `RecoverableError` hint at the three call sites that
/// share the full vocabulary (`symbols`, `references`, `semantic_search`). `index`
/// accepts a narrower set (no `libraries`/`all`) and builds its own hint instead.
pub const SCOPE_ACCEPTED_HINT: &str = "'project' (default), 'libraries', 'all', or \
    'lib:<name>' (e.g. 'lib:serde') — an open-ended prefix naming a registered \
    library, not a fixed member of the list. This `scope` is a different axis from \
    `doc`/`librarian`'s `scope` (`project|repo|umbrella|all`).";

/// Rendered accepted-set for `index`'s `RecoverableError` hint. `index` shares
/// `Scope::parse` with the three tools above but accepts a NARROWER set: no
/// `libraries`/`all` — there is no "index everything" operation, only a single
/// project or a single library — so `Scope::parse` succeeding is not enough;
/// `index`'s call site rejects `Scope::Libraries`/`Scope::All` itself using
/// this hint instead of `SCOPE_ACCEPTED_HINT`.
pub const INDEX_SCOPE_ACCEPTED_HINT: &str = "'project' (default) or 'lib:<name>' (e.g. \
    'lib:serde') — index has no 'libraries'/'all' scope (there is no \"index \
    everything\" operation). This `scope` is a different axis from `doc`/`librarian`'s \
    `scope` (`project|repo|umbrella|all`).";

impl Scope {
    /// Parse a tool-argument `scope` string against the caller-facing
    /// vocabulary (`project|libraries|all|lib:<name>`). Every current caller
    /// (`symbols`, `references`, `semantic_search`, `index` — `list_overview` is
    /// `symbols`' own overview path, not a distinct tool) reads `scope` straight
    /// off a tool's `input`, so there is no remaining internal/lenient caller to
    /// preserve a silent default-to-`Project` for; an unrecognized value is a
    /// caller error, not a repairable one. Returns `Err(<the raw string>)` — not a
    /// formatted message — so each call site can build a `RecoverableError`
    /// naming its own accepted set (`index`'s is narrower: no `libraries`/`all`,
    /// rejected post-parse at its own call site since `Scope::parse` itself
    /// accepts them for the other three callers).
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("project") => Ok(Scope::Project),
            Some("libraries") => Ok(Scope::Libraries),
            Some("all") => Ok(Scope::All),
            Some(s) if s.len() > 4 && s.starts_with("lib:") => {
                Ok(Scope::Library(s[4..].to_string()))
            }
            Some(s) => Err(s.to_string()),
        }
    }

    pub fn includes_project(&self) -> bool {
        matches!(self, Scope::Project | Scope::All)
    }

    pub fn includes_library(&self, name: &str) -> bool {
        match self {
            Scope::Libraries | Scope::All => true,
            Scope::Library(n) => n == name,
            Scope::Project => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_is_project() {
        // The one deliberately retained leniency: an OMITTED argument still
        // defaults to Project (matches every schema's `"default": "project"`).
        // This is not the bug — the bug was an unrecognized *string* being
        // silently coerced to Project, which the next test pins as rejected.
        assert_eq!(Scope::parse(None), Ok(Scope::Project));
    }

    #[test]
    fn parse_explicit_values() {
        assert_eq!(Scope::parse(Some("project")), Ok(Scope::Project));
        assert_eq!(Scope::parse(Some("libraries")), Ok(Scope::Libraries));
        assert_eq!(Scope::parse(Some("all")), Ok(Scope::All));
        assert_eq!(
            Scope::parse(Some("lib:serde")),
            Ok(Scope::Library("serde".into()))
        );
    }

    #[test]
    fn parse_rejects_unrecognized_value() {
        // Pins the fixed contract: a mistyped or foreign-vocabulary value (e.g.
        // "umbrella" from doc/librarian's *different* scope axis) is refused by
        // name, not silently folded into Project. This is the exact case the
        // lenient version used to swallow — see the superseded `parse_explicit_values`
        // assertion this replaces: `assert_eq!(Scope::parse(Some("unknown")),
        // Scope::Project)`.
        assert_eq!(Scope::parse(Some("unknown")), Err("unknown".to_string()));
        assert_eq!(Scope::parse(Some("umbrella")), Err("umbrella".to_string()));
        assert_eq!(Scope::parse(Some("libary")), Err("libary".to_string()));
    }

    #[test]
    fn parse_rejects_bare_lib_prefix_with_no_name() {
        // "lib:" alone starts with the prefix but names no library — must not
        // silently parse as Scope::Library("").
        assert_eq!(Scope::parse(Some("lib:")), Err("lib:".to_string()));
    }

    #[test]
    fn includes_project_logic() {
        assert!(Scope::Project.includes_project());
        assert!(!Scope::Libraries.includes_project());
        assert!(Scope::All.includes_project());
        assert!(!Scope::Library("x".into()).includes_project());
    }

    #[test]
    fn includes_library_logic() {
        assert!(!Scope::Project.includes_library("serde"));
        assert!(Scope::Libraries.includes_library("serde"));
        assert!(Scope::All.includes_library("serde"));
        assert!(Scope::Library("serde".into()).includes_library("serde"));
        assert!(!Scope::Library("tokio".into()).includes_library("serde"));
    }
}
