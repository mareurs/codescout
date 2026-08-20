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

/// One id namespace claimed by a ledger and also defined by another active artifact.
///
/// Reported, never repaired: choosing between renaming a prefix, normalising a spelling,
/// and committing to qualified citations is a content decision with different costs in each
/// direction. See [`DefinitionIndex::prefix_conflicts`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PrefixConflict {
    pub prefix: String,
    /// Artifacts declaring `entry_prefix: <prefix>` — the claim.
    pub declared_by: Vec<String>,
    /// Every ACTIVE artifact defining at least one `<prefix>-N` token, claimants included.
    pub defined_by: Vec<String>,
}

/// token → definers, plus the per-prefix ownership state the resolver needs.
///
/// `known_prefixes` is the dangling gate: every alpha prefix with ≥1 definition anywhere
/// in the corpus, plus every prefix a ledger DECLARES via `entry_prefix` (BL-41).
///
/// `declared_by` and `active_definers` keep exactly what that flattening throws away —
/// **who** claimed a namespace, and who actually defines inside it. That pairing is the
/// only way to tell a contradicted claim from the blessed per-work-stream convention, so
/// it cannot be recovered from `known_prefixes` afterwards. See
/// [`DefinitionIndex::prefix_conflicts`].
#[derive(Debug, Default)]
pub struct DefinitionIndex {
    by_token: BTreeMap<String, Vec<DefinerRef>>,
    known_prefixes: BTreeSet<String>,
    declared_by: BTreeMap<String, BTreeSet<String>>,
    active_definers: BTreeMap<String, BTreeSet<String>>,
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
                    // Only ACTIVE definers, because an archive companion is the compaction
                    // ladder's endpoint rather than a rival claimant — the resolver already
                    // binds a token to its sole active definer. Counting archives here would
                    // make `prefix_conflicts` fire on `R` and `U`, both of which keep one.
                    if active {
                        idx.active_definers
                            .entry(prefix.to_string())
                            .or_default()
                            .insert(artifact_id.to_string());
                    }
                }
            }
            // A DECLARED namespace is a known one whether or not anything defines an entry
            // in it. Learning prefixes from definitions alone cannot distinguish "not a
            // namespace" (`UTF-8`, `SHA-256`, `CI-2` in prose — the noise the gate exists
            // to suppress) from "a namespace that is wholly broken", and it resolved both
            // to silence: a ledger whose every entry lived in an index row reported zero
            // dangling against 129 live citations, none of which resolved.
            //
            // Regardless of `active`: an archived ledger's namespace is still a namespace,
            // and archived definers already resolve (`single_archived_definer_still_resolves`).
            // docs/issues/archive/2026-08-18-link-scan-dangling-count-is-prefix-gated-so-a-whole-namespace-reads-as-healthy.md
            for prefix in &ex.declared_prefixes {
                idx.known_prefixes.insert(prefix.clone());
                idx.declared_by
                    .entry(prefix.clone())
                    .or_default()
                    .insert(artifact_id.to_string());
            }
        }
        idx
    }

    /// Prefixes where a **declared** namespace has more than one **active** definer.
    ///
    /// Both halves of that pairing are the discriminator, and dropping either makes the
    /// check useless in a different way:
    ///
    /// - **Declared.** Eight session logs define `F-N` and none declares `entry_prefix`.
    ///   That is the documented per-work-stream convention — each log owns its own counter
    ///   and citations are qualified by file stem — so reporting it would flag a blessed
    ///   pattern, and `ambiguous` already quantifies its real cost (~400 citations, 49 of
    ///   50 sampled being F/W). A declaration is an author claiming exclusivity; declining
    ///   to declare is declining to claim it.
    /// - **Active.** `R` and `U` each keep an archive companion that legitimately defines
    ///   their tokens. Counting those would fire twice more, both false.
    ///
    /// Measured 2026-08-18: exactly one conflict on this corpus — `T`, declared by
    /// `fable-tuning-tasks.md` and also defined by `tool-usage-patterns.md`. Their token
    /// spaces stayed disjoint only because the latter spells its first thirteen entries
    /// zero-padded (`T-001`…`T-013`) while its later ones are `T-14`…`T-24`, and the
    /// resolver matches token strings — an accident nothing recorded or enforced.
    ///
    /// **That conflict was fixed the same day, so this now reports 0 on the corpus:**
    /// `fable-tuning-tasks` took `FT`; `tool-usage-patterns` kept `T` as the wider-cited
    /// claimant. A 0 here is the healthy state, not dead wiring — the founding case lives
    /// in the tests below, which is where a regression guard belongs anyway.
    ///
    /// The rename also settled what no count could: `T-1`…`T-12` had been binding ~65
    /// citations that were never about fable tasks — three retired `T-N` ledgers plus
    /// `artifact-augmentation-followups`, whose 14 row-only tasks resolved into fable's
    /// namespace as silent wrong edges. Freeing the prefix turned those back into honest
    /// danglings and `dangling` rose 477 → 542. **A falling `dangling` is not evidence of
    /// repair when a namespace gains a definer**: "citations repaired" and "citations
    /// mis-bound" move that number in the same direction.
    /// docs/issues/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md
    ///
    /// A declared prefix that defines nothing is deliberately silent here:
    /// `ledger_defines_nothing` owns that case, and its entries are uncitable regardless of
    /// who else shares the namespace.
    ///
    /// Ordered by prefix — `BTreeMap`/`BTreeSet` throughout — so a report diffs cleanly
    /// against a prior run, the same reason `doctor` orders by `abs_path`.
    pub fn prefix_conflicts(&self) -> Vec<PrefixConflict> {
        self.declared_by
            .iter()
            .filter_map(|(prefix, declarers)| {
                let definers = self.active_definers.get(prefix)?;
                if definers.len() < 2 {
                    return None;
                }
                Some(PrefixConflict {
                    prefix: prefix.clone(),
                    declared_by: declarers.iter().cloned().collect(),
                    defined_by: definers.iter().cloned().collect(),
                })
            })
            .collect()
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
    /// File stem → the artifact ids sharing it. This is the qualifier vocabulary
    /// for `<stem>:<TOKEN>` citations. A `Vec` because stems are NOT unique
    /// across directories (two `index.md` files), and a collision has to be
    /// reported rather than guessed.
    ///
    /// The stem, not `artifact.slug`: slugs are lazily minted from
    /// `slugify(title)` with `-2` dedup, so they neither exist for most rows nor
    /// can be predicted from the filename — and a citation an author cannot
    /// predict is not a citation.
    pub by_stem: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Edge {
        dst_id: String,
    },
    /// The citation's target is the CITING artifact itself.
    ///
    /// Carries `dst_id` — which is `src_id` — because self-citation is a file-grain
    /// verdict serving two consumers with different grains. At file grain it is not an
    /// edge: a `cites` row from an artifact to itself is a self-loop. At ENTRY grain two
    /// entries in one ledger are two distinct nodes, so `**Kin:** R-3` written inside
    /// `## R-41` is a real edge, and the caller needs the target to build it. Without the
    /// payload the entry layer cannot recover what the file layer already resolved, and
    /// every intra-ledger edge is lost before attribution runs
    /// (`docs/issues/archive/2026-08-21-selfcite-is-file-grain-so-intra-ledger-entry-edges-never-materialize.md`).
    SelfCite {
        dst_id: String,
    },
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
        CitationKind::CrossRepoToken => {
            // `<qualifier>:<TOKEN>` carries two different intents, told apart by
            // whether the qualifier names a file in THIS repo:
            //
            //   codescout:A-11            cross-REPO — cannot become an edge,
            //                             because edges do not span workspaces
            //   bug-fix-session-log:F-33  within-repo QUALIFIED entry citation
            //
            // The second exists because per-work-stream namespaces reuse low
            // numbers: F-1..F-5 and W-1..W-2 are each defined in all eight live
            // session logs, so a bare `F-33` has eight definers and resolves to
            // nothing usable. Measured 2026-08-17: ~400 ambiguous citations, 49 of
            // 50 sampled were F/W, and the citers were the DURABLE ledgers (R-N
            // alone 27 of 50) losing their links to their own evidence.
            let Some((qualifier, token)) = citation.raw.split_once(':') else {
                return Some(Outcome::CrossRepo);
            };
            let Some(candidates) = corpus.by_stem.get(qualifier) else {
                // Unknown qualifier — a real cross-repo ref. Unchanged behaviour.
                return Some(Outcome::CrossRepo);
            };
            match candidates.as_slice() {
                [only] => {
                    if only.as_str() == src_id {
                        return Some(Outcome::SelfCite {
                            dst_id: only.clone(),
                        });
                    }
                    if index.definers(token).iter().any(|d| &d.artifact_id == only) {
                        Some(Outcome::Edge {
                            dst_id: only.clone(),
                        })
                    } else {
                        // The qualifier resolved but that file defines no such
                        // entry: a genuinely broken citation, not an ambiguity.
                        // Worth distinguishing — the fixes differ.
                        Some(Outcome::Dangling)
                    }
                }
                // Two files share a stem. Report rather than guess: precision-first,
                // exactly as the bare-token path does.
                many => Some(Outcome::Ambiguous {
                    total: many.len(),
                    candidates: many.iter().take(AMBIGUOUS_CANDIDATE_CAP).cloned().collect(),
                }),
            }
        }
        CitationKind::ArtifactId => {
            if citation.raw == src_id {
                Some(Outcome::SelfCite {
                    dst_id: src_id.to_string(),
                })
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
                        Outcome::SelfCite { dst_id: id.clone() }
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
                // The local definition wins when a token is defined here AND elsewhere:
                // an entry citing a sibling in its own ledger means the sibling it can
                // see. `dst_id` is therefore this artifact, and the entry layer narrows
                // it to the specific entry.
                return Some(Outcome::SelfCite {
                    dst_id: src_id.to_string(),
                });
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
            ..Default::default()
        }
    }

    fn cite(raw: &str, kind: CitationKind) -> Citation {
        Citation {
            raw: raw.to_string(),
            kind,
            repeat_lines: Vec::new(),
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
        // The local definer wins, and the payload names it — `art-a`, not `art-b`.
        // The entry layer needs that to build an intra-ledger edge.
        assert_eq!(
            got,
            Some(Outcome::SelfCite {
                dst_id: "art-a".into()
            })
        );
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

    /// The fix. A ledger that DECLARES `entry_prefix: WIN` and defines nothing must
    /// still have its citations reported. `known_prefixes` previously learned prefixes
    /// only from definitions, so a wholly-undefined namespace was indistinguishable
    /// from `UTF-8` in prose and every citation of it was suppressed — 129 live `WIN-N`
    /// citations reporting as zero dangling, on a ledger where none of them resolved.
    /// docs/issues/archive/2026-08-18-link-scan-dangling-count-is-prefix-gated-so-a-whole-namespace-reads-as-healthy.md
    #[test]
    fn a_declared_prefix_dangles_even_though_nothing_defines_it() {
        let ledger = DocExtract {
            declared_prefixes: vec!["WIN".to_string()],
            ..Default::default()
        };
        let idx = DefinitionIndex::build([("winledger", "active", &ledger)]);
        assert_eq!(
            resolve(
                &cite("WIN-5", CitationKind::EntryToken),
                "doc",
                "",
                &idx,
                &Corpus::default()
            ),
            Some(Outcome::Dangling),
            "the namespace is declared, so its citations are breakage — not noise"
        );
    }

    /// The other side of the same condition: a declaration must not un-suppress
    /// everything. Extraction is deliberately dumb about `UTF-8` / `SHA-256`
    /// (`prose_acronyms_are_extracted_dumbly`) and the gate is the only thing keeping
    /// them quiet, so widening it per-prefix must stay per-prefix.
    #[test]
    fn declaring_one_prefix_does_not_un_suppress_the_others() {
        let ledger = DocExtract {
            declared_prefixes: vec!["WIN".to_string()],
            ..Default::default()
        };
        let idx = DefinitionIndex::build([("winledger", "active", &ledger)]);
        assert_eq!(
            resolve(
                &cite("UTF-8", CitationKind::EntryToken),
                "doc",
                "",
                &idx,
                &Corpus::default()
            ),
            None,
            "declaring WIN says nothing about UTF"
        );
    }

    /// The wire, not the halves — a real body through the real extractor. This ledger's
    /// entries live in index ROWS, which define nothing, so the declaration travelling
    /// on `DocExtract` is the only thing that can make its citations reportable. Fails
    /// if `extract` stops populating `declared_prefixes` or `build` stops reading it,
    /// neither of which a unit test on either half alone can see.
    #[test]
    fn a_row_only_declared_ledger_is_reportable_end_to_end() {
        let ledger = crate::librarian::tools::link_scan::extract::extract(
            "---\nkind: tracker\nentry_prefix: WIN\n---\n\n| ID | title |\n| WIN-1 | a |\n",
        );
        assert!(
            ledger.definitions.is_empty(),
            "precondition: rows define nothing, so the gate has only the declaration \
                 to go on — got {:?}",
            ledger.definitions
        );
        let idx = DefinitionIndex::build([("winledger", "active", &ledger)]);
        assert_eq!(
            resolve(
                &cite("WIN-1", CitationKind::EntryToken),
                "doc",
                "",
                &idx,
                &Corpus::default()
            ),
            Some(Outcome::Dangling)
        );
    }

    /// A **declared** namespace with a second **active** definer is a claim being
    /// contradicted, and that pairing is the whole discriminator.
    ///
    /// Measured 2026-08-18: fired exactly once on the real corpus — `T`, owned by
    /// `fable-tuning-tasks.md` and squatted by `tool-usage-patterns.md`, whose token
    /// spaces stayed disjoint only because the latter spells its first thirteen entries
    /// zero-padded. Nothing recorded that invariant, and two ordinary edits broke it.
    /// Fixed the same day by renaming the fable ledger to `FT`, so the corpus reports 0
    /// now and this fixture is the only surviving record of the shape.
    /// docs/issues/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md
    #[test]
    fn a_declared_prefix_with_a_second_active_definer_is_a_conflict() {
        let owner = DocExtract {
            declared_prefixes: vec!["T".to_string()],
            ..ex_with_defs(&["T-1"])
        };
        let squatter = ex_with_defs(&["T-14"]);
        let idx = DefinitionIndex::build([
            ("fable-tasks", "active", &owner),
            ("tool-usage", "active", &squatter),
        ]);

        let got = idx.prefix_conflicts();
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].prefix, "T");
        assert_eq!(got[0].declared_by, vec!["fable-tasks".to_string()]);
        assert_eq!(
            got[0].defined_by,
            vec!["fable-tasks".to_string(), "tool-usage".to_string()],
            "both active definers must be named, so the reader knows who to talk to"
        );
    }

    /// The false positive this check exists to avoid. Eight session logs define `F-N`
    /// and **none declares `entry_prefix`** — that is the documented per-work-stream
    /// convention, whose remedy is a file-stem-qualified citation rather than a rename.
    /// Firing here would report a blessed pattern, and `ambiguous` already quantifies
    /// its real cost (~400 citations, 49 of 50 sampled being F/W).
    ///
    /// Verified against the corpus, not assumed: of the nine prefixes declared in this
    /// repo, none is `F` or `W`.
    #[test]
    fn undeclared_co_definers_are_not_a_conflict() {
        let a = ex_with_defs(&["F-1", "F-2"]);
        let b = ex_with_defs(&["F-1", "F-3"]);
        let idx =
            DefinitionIndex::build([("bugfix-log", "active", &a), ("release-log", "active", &b)]);
        assert!(
            idx.prefix_conflicts().is_empty(),
            "nobody claimed F, so two definers of it is a convention, not a conflict"
        );
    }

    /// An archive companion is the compaction ladder's endpoint, not a squatter: the
    /// resolver already binds a token to its sole ACTIVE definer, and archiving is the
    /// documented way to compact a ledger without destroying its definitions.
    ///
    /// This exclusion is load-bearing on real data rather than defensive — it is what
    /// keeps `R` (`reconnaissance-patterns` + its archive) and `U`
    /// (`codescout-usage-frictions` + its archive) quiet. Without it the check would
    /// fire three times on this repo, two of them false.
    #[test]
    fn an_archived_co_definer_is_not_a_conflict() {
        let live = DocExtract {
            declared_prefixes: vec!["R".to_string()],
            ..ex_with_defs(&["R-98"])
        };
        let archived = ex_with_defs(&["R-4"]);
        let idx = DefinitionIndex::build([
            ("recon-live", "active", &live),
            ("recon-archive", "archived", &archived),
        ]);
        assert!(
            idx.prefix_conflicts().is_empty(),
            "the sole ACTIVE definer owns the namespace; its archive is not a rival"
        );
    }

    /// A declared prefix nobody else touches is the healthy case, and asserting it keeps
    /// the three tests above from passing on a method that returns empty unconditionally.
    #[test]
    fn a_declared_prefix_with_one_definer_is_not_a_conflict() {
        let only = DocExtract {
            declared_prefixes: vec!["SD".to_string()],
            ..ex_with_defs(&["SD-1", "SD-2"])
        };
        let idx = DefinitionIndex::build([("structural-debt", "active", &only)]);
        assert!(idx.prefix_conflicts().is_empty());
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
            Some(Outcome::SelfCite {
                dst_id: "59ebeebb6ed05c89".into()
            })
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

    /// The pair that states the whole problem and the whole fix. `F-3` is defined
    /// in two session logs — the real shape, where F-1..F-5 are defined in all
    /// eight — so bare it is Ambiguous. Qualified by file stem it resolves.
    #[test]
    fn a_qualified_citation_resolves_where_the_bare_token_cannot() {
        let a = ex_with_defs(&["F-3"]);
        let b = ex_with_defs(&["F-3"]);
        let idx =
            DefinitionIndex::build([("bugfix-id", "active", &a), ("release-id", "active", &b)]);
        let mut corpus = Corpus::default();
        corpus
            .by_stem
            .insert("bug-fix-session-log".into(), vec!["bugfix-id".into()]);
        corpus.by_stem.insert(
            "release-promotion-session-log".into(),
            vec!["release-id".into()],
        );

        assert!(
            matches!(
                resolve(
                    &cite("F-3", CitationKind::EntryToken),
                    "doc",
                    "",
                    &idx,
                    &corpus
                ),
                Some(Outcome::Ambiguous { .. })
            ),
            "the bare token must stay ambiguous — that is the problem being solved"
        );
        assert_eq!(
            resolve(
                &cite("bug-fix-session-log:F-3", CitationKind::CrossRepoToken),
                "doc",
                "",
                &idx,
                &corpus
            ),
            Some(Outcome::Edge {
                dst_id: "bugfix-id".into()
            }),
        );
    }

    /// A qualifier naming no file in this repo is a genuine cross-repo reference.
    /// This is what keeps `codescout:A-11` behaving as before.
    #[test]
    fn an_unknown_qualifier_stays_cross_repo() {
        let a = ex_with_defs(&["A-11"]);
        let idx = DefinitionIndex::build([("local", "active", &a)]);
        let mut corpus = Corpus::default();
        corpus
            .by_stem
            .insert("something-else".into(), vec!["local".into()]);

        assert_eq!(
            resolve(
                &cite("codescout:A-11", CitationKind::CrossRepoToken),
                "doc",
                "",
                &idx,
                &corpus
            ),
            Some(Outcome::CrossRepo)
        );
    }

    /// Qualifier resolves, entry does not exist there. That is a broken citation,
    /// not an ambiguity — and the distinction matters because the fixes differ:
    /// a dangling one needs the entry or the citation corrected, an ambiguous one
    /// needs a namespace decision.
    #[test]
    fn a_qualified_citation_to_a_file_without_that_entry_dangles() {
        let a = ex_with_defs(&["F-3"]);
        let idx = DefinitionIndex::build([("bugfix-id", "active", &a)]);
        let mut corpus = Corpus::default();
        corpus
            .by_stem
            .insert("bug-fix-session-log".into(), vec!["bugfix-id".into()]);

        assert_eq!(
            resolve(
                &cite("bug-fix-session-log:F-99", CitationKind::CrossRepoToken),
                "doc",
                "",
                &idx,
                &corpus
            ),
            Some(Outcome::Dangling)
        );
    }

    /// Stems are not unique across directories (`bistriceanu/index.md` and any
    /// other `index.md`). Report, never guess — a wrong edge pollutes
    /// `context(anchor_id)` packing, which consumes rels unfiltered.
    #[test]
    fn a_colliding_stem_is_reported_not_guessed() {
        let a = ex_with_defs(&["B-1"]);
        let b = ex_with_defs(&["B-1"]);
        let idx = DefinitionIndex::build([("one", "active", &a), ("two", "active", &b)]);
        let mut corpus = Corpus::default();
        corpus
            .by_stem
            .insert("index".into(), vec!["one".into(), "two".into()]);

        assert!(matches!(
            resolve(
                &cite("index:B-1", CitationKind::CrossRepoToken),
                "doc",
                "",
                &idx,
                &corpus
            ),
            Some(Outcome::Ambiguous { total: 2, .. })
        ));
    }

    #[test]
    fn a_qualified_citation_of_your_own_file_is_a_self_cite() {
        let a = ex_with_defs(&["F-3"]);
        let idx = DefinitionIndex::build([("bugfix-id", "active", &a)]);
        let mut corpus = Corpus::default();
        corpus
            .by_stem
            .insert("bug-fix-session-log".into(), vec!["bugfix-id".into()]);

        assert_eq!(
            resolve(
                &cite("bug-fix-session-log:F-3", CitationKind::CrossRepoToken),
                "bugfix-id",
                "",
                &idx,
                &corpus
            ),
            Some(Outcome::SelfCite {
                dst_id: "bugfix-id".into()
            })
        );
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
