/// Scope for library-aware tool queries.
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

impl Scope {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            None | Some("project") => Scope::Project,
            Some("libraries") => Scope::Libraries,
            Some("all") => Scope::All,
            Some(s) if s.starts_with("lib:") => Scope::Library(s[4..].to_string()),
            Some(_) => Scope::Project,
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
        assert_eq!(Scope::parse(None), Scope::Project);
    }

    #[test]
    fn parse_explicit_values() {
        assert_eq!(Scope::parse(Some("project")), Scope::Project);
        assert_eq!(Scope::parse(Some("libraries")), Scope::Libraries);
        assert_eq!(Scope::parse(Some("all")), Scope::All);
        assert_eq!(
            Scope::parse(Some("lib:serde")),
            Scope::Library("serde".into())
        );
        assert_eq!(Scope::parse(Some("unknown")), Scope::Project);
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
