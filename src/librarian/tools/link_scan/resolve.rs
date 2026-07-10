// src/librarian/tools/link_scan/resolve.rs
//! Pure resolution stage: map extracted citations to outcomes against a
//! definition index + catalog corpus. Policy lives HERE (extraction is
//! deliberately dumb): self-cite absorption, the archived-definer tie-break,
//! the prefix gate that silences prose-acronym noise, and precision-first
//! ambiguity handling (a wrong edge pollutes `context(anchor_id)` packing,
//! which consumes all rels unfiltered).

use std::collections::{BTreeMap, BTreeSet};

use super::extract::{Citation, CitationKind, DocExtract};

/// One artifact that defines a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinerRef {
    pub artifact_id: String,
    /// `false` when the defining artifact is archived/superseded — the
    /// tie-break: archive-flow residue must not turn every archived tracker
    /// into an ambiguity generator.
    pub active: bool,
}

/// token → definers, plus the set of alpha prefixes with ≥1 definition
/// anywhere in the corpus (the dangling gate).
#[derive(Debug, Default)]
pub struct DefinitionIndex {
    by_token: BTreeMap<String, Vec<DefinerRef>>,
    known_prefixes: BTreeSet<String>,
}

impl DefinitionIndex {
    pub fn build<'a>(
        extracts: impl IntoIterator<Item = (&'a str, &'a str, &'a DocExtract)>,
    ) -> Self {
        let mut idx = DefinitionIndex::default();
        for (artifact_id, status, ex) in extracts {
            let active = !matches!(status, "archived" | "superseded");
            for def in &ex.definitions {
                idx.by_token
                    .entry(def.token.clone())
                    .or_default()
                    .push(DefinerRef {
                        artifact_id: artifact_id.to_string(),
                        active,
                    });
                if let Some(prefix) = def.token.split('-').next() {
                    idx.known_prefixes.insert(prefix.to_string());
                }
            }
        }
        idx
    }

    fn definers(&self, token: &str) -> &[DefinerRef] {
        self.by_token.get(token).map(Vec::as_slice).unwrap_or(&[])
    }

    fn prefix_is_known(&self, token: &str) -> bool {
        token
            .split('-')
            .next()
            .is_some_and(|p| self.known_prefixes.contains(p))
    }
}

/// Catalog corpus view needed for id / rel-path resolution.
#[derive(Debug, Default)]
pub struct Corpus {
    pub ids: BTreeSet<String>,
    /// forward-slash repo-relative path → artifact id.
    pub by_rel_path: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Edge {
        dst_id: String,
    },
    SelfCite,
    Ambiguous {
        candidates: Vec<String>,
        total: usize,
    },
    Dangling,
    CrossRepo,
}

/// Cap on candidate ids carried in an Ambiguous finding.
const AMBIGUOUS_CANDIDATE_CAP: usize = 5;

/// Resolve one citation from the artifact `src_id` whose repo-relative file
/// sits in directory `src_rel_dir` (forward-slash, "" for repo root).
/// `None` = suppressed (prose-acronym noise, or unresolved rel-path links —
/// broken links are audit_doc_refs's jurisdiction, not link_scan's).
pub fn resolve(
    citation: &Citation,
    src_id: &str,
    src_rel_dir: &str,
    index: &DefinitionIndex,
    corpus: &Corpus,
) -> Option<Outcome> {
    match citation.kind {
        CitationKind::CrossRepoToken => Some(Outcome::CrossRepo),
        CitationKind::ArtifactId => {
            if citation.raw == src_id {
                Some(Outcome::SelfCite)
            } else if corpus.ids.contains(&citation.raw) {
                Some(Outcome::Edge {
                    dst_id: citation.raw.clone(),
                })
            } else {
                // Always reported: a 16-hex token is high-signal, and stale
                // ids are the documented cost of the v6 migration — this is
                // the detector for them.
                Some(Outcome::Dangling)
            }
        }
        CitationKind::RelPathLink => {
            for candidate in normalize_link_target(src_rel_dir, &citation.raw) {
                if let Some(id) = corpus.by_rel_path.get(&candidate) {
                    return Some(if id == src_id {
                        Outcome::SelfCite
                    } else {
                        Outcome::Edge { dst_id: id.clone() }
                    });
                }
            }
            None
        }
        CitationKind::EntryToken => {
            let definers = index.definers(&citation.raw);
            if definers.iter().any(|d| d.artifact_id == src_id) {
                return Some(Outcome::SelfCite);
            }
            match definers.len() {
                0 => {
                    if index.prefix_is_known(&citation.raw) {
                        Some(Outcome::Dangling)
                    } else {
                        None // UTF-8 / SHA-256 / GPT-4 prose noise
                    }
                }
                1 => Some(Outcome::Edge {
                    dst_id: definers[0].artifact_id.clone(),
                }),
                _ => {
                    let active: Vec<_> = definers.iter().filter(|d| d.active).collect();
                    if active.len() == 1 {
                        Some(Outcome::Edge {
                            dst_id: active[0].artifact_id.clone(),
                        })
                    } else {
                        Some(Outcome::Ambiguous {
                            total: definers.len(),
                            candidates: definers
                                .iter()
                                .map(|d| d.artifact_id.clone())
                                .take(AMBIGUOUS_CANDIDATE_CAP)
                                .collect(),
                        })
                    }
                }
            }
        }
    }
}

/// Candidate repo-relative interpretations of a markdown link target, in
/// resolution order: repo-root-relative first (the dominant house style),
/// then relative to the citing file's directory (handles `../` links).
pub fn normalize_link_target(src_rel_dir: &str, target: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(2);
    if let Some(c) = normalize_segments("", target) {
        out.push(c);
    }
    if let Some(c) = normalize_segments(src_rel_dir, target) {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

/// Join `base` (repo-relative dir, "" for root) with `rel` and normalize
/// `.`/`..` segments. Returns None if `..` escapes the repo root.
fn normalize_segments(base: &str, rel: &str) -> Option<String> {
    let mut stack: Vec<&str> = base.split('/').filter(|s| !s.is_empty()).collect();
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                stack.pop()?;
            }
            s => stack.push(s),
        }
    }
    Some(stack.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::tools::link_scan::extract::Definition;

    fn ex_with_defs(tokens: &[&str]) -> DocExtract {
        DocExtract {
            definitions: tokens
                .iter()
                .map(|t| Definition {
                    token: t.to_string(),
                    line: 1,
                })
                .collect(),
            citations: vec![],
        }
    }

    fn cite(raw: &str, kind: CitationKind) -> Citation {
        Citation {
            raw: raw.to_string(),
            kind,
            line: 1,
        }
    }

    #[test]
    fn self_citation_wins_even_with_other_definers() {
        let a = ex_with_defs(&["F-1"]);
        let b = ex_with_defs(&["F-1"]);
        let idx = DefinitionIndex::build([("art-a", "active", &a), ("art-b", "active", &b)]);
        let corpus = Corpus::default();
        let got = resolve(
            &cite("F-1", CitationKind::EntryToken),
            "art-a",
            "docs/trackers",
            &idx,
            &corpus,
        );
        assert_eq!(got, Some(Outcome::SelfCite));
    }

    #[test]
    fn unique_external_definer_yields_edge() {
        let a = ex_with_defs(&["A-11"]);
        let idx = DefinitionIndex::build([("audit-log", "active", &a)]);
        let got = resolve(
            &cite("A-11", CitationKind::EntryToken),
            "findings-doc",
            "docs/research",
            &idx,
            &Corpus::default(),
        );
        assert_eq!(
            got,
            Some(Outcome::Edge {
                dst_id: "audit-log".into()
            })
        );
    }

    #[test]
    fn archived_tie_break_resolves_to_sole_active_definer() {
        let a = ex_with_defs(&["BUG-30"]);
        let b = ex_with_defs(&["BUG-30"]);
        let idx = DefinitionIndex::build([
            ("live-tracker", "active", &a),
            ("old-archive", "archived", &b),
        ]);
        let got = resolve(
            &cite("BUG-30", CitationKind::EntryToken),
            "third-doc",
            "",
            &idx,
            &Corpus::default(),
        );
        assert_eq!(
            got,
            Some(Outcome::Edge {
                dst_id: "live-tracker".into()
            })
        );
    }

    #[test]
    fn multiple_active_definers_yield_ambiguous_no_edge() {
        let a = ex_with_defs(&["F-1"]);
        let b = ex_with_defs(&["F-1"]);
        let idx = DefinitionIndex::build([
            ("session-log-1", "active", &a),
            ("session-log-2", "active", &b),
        ]);
        let got = resolve(
            &cite("F-1", CitationKind::EntryToken),
            "third-doc",
            "",
            &idx,
            &Corpus::default(),
        );
        match got {
            Some(Outcome::Ambiguous { candidates, total }) => {
                assert_eq!(candidates.len(), 2);
                assert_eq!(total, 2);
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn single_archived_definer_still_resolves() {
        // Unique definer wins even if archived — archived is not nonexistent.
        let a = ex_with_defs(&["S-12"]);
        let idx = DefinitionIndex::build([("old-residuals", "archived", &a)]);
        let got = resolve(
            &cite("S-12", CitationKind::EntryToken),
            "doc",
            "",
            &idx,
            &Corpus::default(),
        );
        assert_eq!(
            got,
            Some(Outcome::Edge {
                dst_id: "old-residuals".into()
            })
        );
    }

    #[test]
    fn dangling_is_prefix_gated() {
        let a = ex_with_defs(&["U-1"]);
        let idx = DefinitionIndex::build([("frictions", "active", &a)]);
        let corpus = Corpus::default();
        // U-99: prefix U is defined somewhere → report dangling.
        assert_eq!(
            resolve(
                &cite("U-99", CitationKind::EntryToken),
                "doc",
                "",
                &idx,
                &corpus
            ),
            Some(Outcome::Dangling)
        );
        // UTF-8: prefix UTF has no definitions anywhere → suppressed.
        assert_eq!(
            resolve(
                &cite("UTF-8", CitationKind::EntryToken),
                "doc",
                "",
                &idx,
                &corpus
            ),
            None
        );
    }

    #[test]
    fn artifact_id_resolution_edge_self_dangling() {
        let idx = DefinitionIndex::default();
        let mut corpus = Corpus::default();
        corpus.ids.insert("59ebeebb6ed05c89".into());
        corpus.ids.insert("f2ecdd76a6189efb".into());

        assert_eq!(
            resolve(
                &cite("f2ecdd76a6189efb", CitationKind::ArtifactId),
                "59ebeebb6ed05c89",
                "docs/trackers",
                &idx,
                &corpus
            ),
            Some(Outcome::Edge {
                dst_id: "f2ecdd76a6189efb".into()
            })
        );
        assert_eq!(
            resolve(
                &cite("59ebeebb6ed05c89", CitationKind::ArtifactId),
                "59ebeebb6ed05c89",
                "docs/trackers",
                &idx,
                &corpus
            ),
            Some(Outcome::SelfCite)
        );
        // Stale id (v6-migration cost detector): always reported.
        assert_eq!(
            resolve(
                &cite("aaaaaaaaaaaaaaaa", CitationKind::ArtifactId),
                "59ebeebb6ed05c89",
                "docs/trackers",
                &idx,
                &corpus
            ),
            Some(Outcome::Dangling)
        );
    }

    #[test]
    fn rel_path_link_root_relative_and_file_relative() {
        let idx = DefinitionIndex::default();
        let mut corpus = Corpus::default();
        corpus
            .by_rel_path
            .insert("docs/trackers/x.md".into(), "art-x".into());

        // Root-relative target.
        assert_eq!(
            resolve(
                &cite("docs/trackers/x.md", CitationKind::RelPathLink),
                "src-doc",
                "docs/research",
                &idx,
                &corpus
            ),
            Some(Outcome::Edge {
                dst_id: "art-x".into()
            })
        );
        // File-relative ../ target from docs/research/.
        assert_eq!(
            resolve(
                &cite("../trackers/x.md", CitationKind::RelPathLink),
                "src-doc",
                "docs/research",
                &idx,
                &corpus
            ),
            Some(Outcome::Edge {
                dst_id: "art-x".into()
            })
        );
        // Unresolved → suppressed (audit_doc_refs's jurisdiction).
        assert_eq!(
            resolve(
                &cite("docs/gone.md", CitationKind::RelPathLink),
                "src-doc",
                "docs/research",
                &idx,
                &corpus
            ),
            None
        );
    }

    #[test]
    fn cross_repo_reports_only() {
        let got = resolve(
            &cite("codescout:A-11", CitationKind::CrossRepoToken),
            "doc",
            "",
            &DefinitionIndex::default(),
            &Corpus::default(),
        );
        assert_eq!(got, Some(Outcome::CrossRepo));
    }

    #[test]
    fn normalize_rejects_root_escape() {
        assert_eq!(normalize_segments("", "../../etc/passwd"), None);
        assert_eq!(
            normalize_segments("docs/research", "../trackers/x.md"),
            Some("docs/trackers/x.md".into())
        );
    }
}
