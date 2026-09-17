//! The embedding environment surface, declared in one place.
//!
//! # Why this module exists
//!
//! Eight environment variables named the same three settings, across three
//! independently-grown consumers. Nothing declared the set, so nothing could tell you
//! it had grown — each reader spelled its own name at its own call site, and the
//! duplicates were only visible to somebody grepping the whole tree at once:
//!
//! | setting | names that reached it |
//! |---|---|
//! | model | `CODESCOUT_EMBEDDER_MODEL`, `CODESCOUT_EMBED_MODEL`, `CODESCOUT_EMBEDDER_MODEL_NAME` |
//! | url | `CODESCOUT_EMBEDDER_URL`, `CODESCOUT_EMBED_URL` |
//! | api key | `EMBED_API_KEY` |
//! | dim | `CODESCOUT_MODEL_DIM` |
//! | query prefix | `CODESCOUT_QUERY_PREFIX` |
//!
//! Worse than the count: the `CODESCOUT_EMBED_*` pair applied at a **different layer**
//! from the `CODESCOUT_EMBEDDER_*` pair — the first inside
//! `ProjectConfig::load_or_default`, the second in `merge_embed_config` — so two
//! independently-named variables reached one effective setting at two points in the
//! same resolution, and which won depended on where you looked.
//!
//! # The shape of the fix
//!
//! One canonical name per setting, `CODESCOUT_EMBEDDING_*`, with every old name kept
//! as a **deprecated alias that still works** and warns once naming its replacement.
//! Nothing breaks on upgrade; the surface shrinks as people migrate.
//!
//! The declaration is the point, not the rename. A table of names cannot silently gain
//! a ninth entry the way eight scattered `env::var` calls did — adding one here is a
//! diff a reviewer sees.
//!
//! # Testability
//!
//! [`pick`] is pure over an injected lookup, so precedence and
//! which-alias-was-used are ordinary unit tests. Reading process env inside a resolver
//! is what made the model-discard defect untestable and let it ship for a release;
//! `docs/conventions/test-env-isolation.md` bans the `set_var` that would be needed to
//! test it otherwise.

use std::collections::BTreeSet;
use std::sync::Mutex;

/// A setting's canonical environment variable, plus the deprecated names that still
/// reach it.
///
/// `deprecated` is in **precedence order**: earlier entries win over later ones, which
/// preserves the pre-consolidation behaviour where `CODESCOUT_EMBEDDER_*` was applied
/// closer to the resolver than `CODESCOUT_EMBED_*`.
pub(crate) struct EnvName {
    pub(crate) canonical: &'static str,
    pub(crate) deprecated: &'static [&'static str],
}

/// The model spec — a `local:` / `local-dir:` / `ollama:` / `openai:` prefix, or a bare
/// name sent verbatim when a url is configured.
pub(crate) const MODEL: EnvName = EnvName {
    canonical: "CODESCOUT_EMBEDDING_MODEL",
    deprecated: &["CODESCOUT_EMBEDDER_MODEL", "CODESCOUT_EMBED_MODEL"],
};

/// The embedder endpoint. Any OpenAI-compatible `/v1/embeddings` base.
pub(crate) const URL: EnvName = EnvName {
    canonical: "CODESCOUT_EMBEDDING_URL",
    deprecated: &["CODESCOUT_EMBEDDER_URL", "CODESCOUT_EMBED_URL"],
};

/// Bearer token for the embedder endpoint. Dropped unless https or loopback.
pub(crate) const API_KEY: EnvName = EnvName {
    canonical: "CODESCOUT_EMBEDDING_API_KEY",
    deprecated: &["EMBED_API_KEY"],
};

/// Operator pin for the embedding dimension. Unset means "the model is the authority".
pub(crate) const DIM: EnvName = EnvName {
    canonical: "CODESCOUT_EMBEDDING_DIM",
    deprecated: &["CODESCOUT_MODEL_DIM"],
};

/// Query-side prefix for asymmetric models. Doc-side embedding is unaffected.
pub(crate) const QUERY_PREFIX: EnvName = EnvName {
    canonical: "CODESCOUT_EMBEDDING_QUERY_PREFIX",
    deprecated: &["CODESCOUT_QUERY_PREFIX"],
};

/// Pure: resolve one setting from an injected lookup.
///
/// Returns the value and, when a **deprecated** name supplied it, that name — so the
/// caller can warn without this function knowing what a warning is.
///
/// Blank is absent at every position, matching `non_empty`'s policy elsewhere: an
/// exported-but-empty canonical name must not mask a real value under a deprecated one,
/// which would be a silent loss of exactly the kind this whole effort was about.
pub(crate) fn pick(
    name: &EnvName,
    lookup: impl Fn(&str) -> Option<String>,
) -> (Option<String>, Option<&'static str>) {
    let non_blank = |v: Option<String>| v.filter(|s| !s.trim().is_empty());

    if let Some(v) = non_blank(lookup(name.canonical)) {
        return (Some(v), None);
    }
    for old in name.deprecated {
        if let Some(v) = non_blank(lookup(old)) {
            return (Some(v), Some(old));
        }
    }
    (None, None)
}

/// Every name in the embedding env surface — canonical and deprecated alike.
///
/// `pub` on purpose, and the reason is a defect this very change caused. Integration
/// tests that assert "nothing is configured" must neutralise the whole surface, and
/// `tests/retrieval_unit.rs` did it by **re-typing the names**. Adding the deprecated
/// chain widened what `EmbedEnv::from_real_env` reads, its hand-written list no longer
/// covered it, and a machine that exports `CODESCOUT_EMBED_URL` started failing a test
/// about defaults — a test whose subject had not changed at all.
///
/// A list that must be kept in step by hand is the same shape as the eight scattered
/// `env::var` calls this module exists to replace. Deriving it here means the ninth
/// name cannot silently un-isolate a test.
pub fn all_names() -> Vec<&'static str> {
    let mut out = Vec::new();
    for n in [&MODEL, &URL, &API_KEY, &DIM, &QUERY_PREFIX] {
        out.push(n.canonical);
        out.extend_from_slice(n.deprecated);
    }
    out
}

/// Names already warned about, so a variable read from several call sites produces one
/// line rather than one per read.
static WARNED: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());

/// Read one setting from the real process environment, warning once per deprecated name
/// actually used.
///
/// The env access is here, at the edge; the decision is in [`pick`].
pub(crate) fn read(name: &EnvName) -> Option<String> {
    let (value, deprecated_used) = pick(name, |k| std::env::var(k).ok());
    if let Some(old) = deprecated_used {
        // A poisoned lock would mean another thread panicked mid-warn. Warning twice is
        // strictly better than propagating that into config resolution, so recover.
        let mut warned = WARNED.lock().unwrap_or_else(|e| e.into_inner());
        if warned.insert(old) {
            tracing::warn!(
                "{old} is deprecated — rename it to {}. It still works and will for at \
                 least one more release. Several names reached the same embedding \
                 setting at different layers, which made the effective value depend on \
                 which one you looked at.",
                name.canonical
            );
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        let map: HashMap<&str, &str> = pairs.iter().copied().collect();
        move |k| map.get(k).map(|v| v.to_string())
    }

    #[test]
    fn the_canonical_name_wins_and_reports_no_deprecation() {
        let (v, old) = pick(
            &MODEL,
            lookup_from(&[
                ("CODESCOUT_EMBEDDING_MODEL", "canonical"),
                ("CODESCOUT_EMBEDDER_MODEL", "deprecated"),
            ]),
        );
        assert_eq!(v.as_deref(), Some("canonical"));
        assert_eq!(old, None, "no warning is owed when the new name was used");
    }

    #[test]
    fn a_deprecated_name_still_works_and_is_named() {
        let (v, old) = pick(
            &MODEL,
            lookup_from(&[("CODESCOUT_EMBEDDER_MODEL", "from-old-name")]),
        );
        assert_eq!(
            v.as_deref(),
            Some("from-old-name"),
            "deprecated must keep WORKING — the whole point of an alias is that \
             nothing breaks on upgrade"
        );
        assert_eq!(old, Some("CODESCOUT_EMBEDDER_MODEL"));
    }

    /// Precedence among the deprecated names is preserved, not reinvented.
    ///
    /// `CODESCOUT_EMBEDDER_MODEL` used to be applied closer to the resolver than
    /// `CODESCOUT_EMBED_MODEL`, so it won. Re-ordering this list would silently change
    /// the effective model on any machine that happens to set both — a config change
    /// disguised as a refactor.
    #[test]
    fn the_older_alias_order_is_preserved() {
        let (v, old) = pick(
            &MODEL,
            lookup_from(&[
                ("CODESCOUT_EMBED_MODEL", "outer-layer"),
                ("CODESCOUT_EMBEDDER_MODEL", "inner-layer"),
            ]),
        );
        assert_eq!(v.as_deref(), Some("inner-layer"));
        assert_eq!(old, Some("CODESCOUT_EMBEDDER_MODEL"));
    }

    /// A blank canonical value must not mask a real deprecated one.
    ///
    /// `FOO=` yields `Some("")`, so a naive first-set-wins would resolve to empty and
    /// discard a working configuration — the same silent-loss shape as the defects this
    /// module was created to end.
    #[test]
    fn a_blank_canonical_value_does_not_mask_a_deprecated_one() {
        let (v, old) = pick(
            &URL,
            lookup_from(&[
                ("CODESCOUT_EMBEDDING_URL", "   "),
                ("CODESCOUT_EMBEDDER_URL", "http://real:1"),
            ]),
        );
        assert_eq!(v.as_deref(), Some("http://real:1"));
        assert_eq!(old, Some("CODESCOUT_EMBEDDER_URL"));
    }

    #[test]
    fn nothing_set_resolves_to_nothing() {
        let (v, old) = pick(&API_KEY, lookup_from(&[]));
        assert_eq!(v, None);
        assert_eq!(old, None);
    }

    /// Every canonical name shares one prefix, and no name is declared twice.
    ///
    /// The prefix is what makes the surface greppable as a set — the property whose
    /// absence let it grow to eight names unnoticed. The duplicate check matters
    /// because an alias listed under two settings would make the effective value depend
    /// on read order, which is the defect class this module closes.
    #[test]
    fn the_declared_surface_is_consistent_and_disjoint() {
        let all = [&MODEL, &URL, &API_KEY, &DIM, &QUERY_PREFIX];
        let mut seen = BTreeSet::new();
        for n in all {
            assert!(
                n.canonical.starts_with("CODESCOUT_EMBEDDING_"),
                "{} breaks the shared prefix",
                n.canonical
            );
            assert!(seen.insert(n.canonical), "{} declared twice", n.canonical);
            for d in n.deprecated {
                assert!(seen.insert(d), "{d} is an alias of more than one setting");
            }
        }
        assert!(
            seen.len() >= 10,
            "non-vacuity: the surface should hold at least the five canonical names \
             and their five-plus aliases, got {}",
            seen.len()
        );
    }
}
