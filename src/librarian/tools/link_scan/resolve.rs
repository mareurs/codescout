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
    /// The tokens actually defined by more than one active artifact — the intersection.
    ///
    /// **Empty means LATENT, never "fine".** Two allocators sharing a namespace whose token
    /// spaces happen not to overlap today is exactly the 2026-08-18 founding case, disjoint
    /// only because one ledger zero-padded its first thirteen entries — "an accident nothing
    /// recorded or enforced", which stopped holding on 2026-09-01 when a third ledger began
    /// at `T-1` and ran past the padding boundary. So this field separates *broken now* from
    /// *not broken yet*; it cannot separate *disjoint by design* from *disjoint by luck*, and
    /// nothing in this payload can, because both are one declarer with several definers. Read
    /// a non-empty list as work, an empty one as a claim to re-check when either side
    /// allocates.
    /// docs/issues/2026-09-02-prefix-t-collides-again-with-the-zero-padding-protection-gone.md
    pub colliding_tokens: Vec<TokenCollision>,
}

/// One token defined by two or more active artifacts, with what the collision costs today.
///
/// The two counts are deliberately different units, because reading one as the other is a
/// measured failure rather than a hypothetical: `link_scan` emits one `Citation` per
/// `(kind, raw)` per document, so `citing_sources` answers *how much is broken* while
/// `citing_mentions` answers *how many places need editing*. `tracker-hygiene-log:HY-21`
/// predicted −3 from three mentions and measured −2, because one citer's two mentions had
/// always been a single finding. Both ship so no reader has to know that to read either.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TokenCollision {
    pub token: String,
    /// The active artifacts defining it — a subset of the conflict's `defined_by`.
    pub defined_by: Vec<String>,
    /// Artifacts holding at least one unresolvable citation of this token.
    pub citing_sources: usize,
    /// Total mentions across those artifacts, counting repeats within one document.
    pub citing_mentions: usize,
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
    ///
    ///   **That sentence describes a corpus that no longer exists, and the drift is the
    ///   reason `colliding_tokens` was added rather than a second exemption.** Measured
    ///   2026-09-02: six session logs now DO declare `entry_prefix`, because
    ///   `get_guide("tracker-conventions")` later instructed every ledger author to — so
    ///   `F` and `W` are reported today, on a rule whose justification was keyed to
    ///   authors declining to declare. An exemption encodes a judgement made at one
    ///   moment and has to be maintained against a corpus that moves; the intersection
    ///   below re-derives the question every run. Left standing rather than deleted
    ///   because it is the clearest instance of why this check reports evidence instead
    ///   of verdicts.
    /// - **Active.** `R` and `U` each keep an archive companion that legitimately defines
    ///   their tokens. Counting those would fire twice more, both false.
    ///
    /// Measured 2026-08-18: exactly one conflict on this corpus — `T`, declared by
    /// `fable-tuning-tasks.md` and also defined by `tool-usage-patterns.md`. Their token
    /// spaces stayed disjoint only because the latter spells its first thirteen entries
    /// zero-padded (`T-001`…`T-013`) while its later ones are `T-14`…`T-24`, and the
    /// resolver matches token strings — an accident nothing recorded or enforced.
    ///
    /// **That accident stopped holding on 2026-09-01**, when a third artifact began
    /// defining `T-1`…`T-17` and ran past the padding boundary: `T-14`…`T-17` are now
    /// defined twice and 17 citations of them resolve to nothing. The founding record
    /// predicted this in the sentence above, which is what a class with no mechanism looks
    /// like from the inside.
    /// docs/issues/2026-09-02-prefix-t-collides-again-with-the-zero-padding-protection-gone.md
    ///
    /// The rename also settled what no count could: `T-1`…`T-12` had been binding ~65
    /// citations that were never about fable tasks — three retired `T-N` ledgers plus
    /// `artifact-augmentation-followups`, whose 14 row-only tasks resolved into fable's
    /// namespace as silent wrong edges. Freeing the prefix turned those back into honest
    /// danglings and `dangling` rose 477 → 542. **A falling `dangling` is not evidence of
    /// repair when a namespace gains a definer**: "citations repaired" and "citations
    /// mis-bound" move that number in the same direction.
    /// docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md
    ///
    /// A declared prefix that defines nothing is deliberately silent here:
    /// `ledger_defines_nothing` owns that case, and its entries are uncitable regardless of
    /// who else shares the namespace.
    ///
    /// `citations` maps a token to `(citing_sources, citing_mentions)` and annotates the
    /// intersection; it never decides membership. **A conflict with no citations and no
    /// overlapping token is still reported**, because the finding is a fact about the
    /// INDEX — two allocators in one namespace — and the founding case was exactly that
    /// state on the day it was filed. Passing an empty map yields the pre-2026-09-02
    /// payload with every count zero, which is what the pure-index tests below assert.
    ///
    /// Ordered by prefix — `BTreeMap`/`BTreeSet` throughout — so a report diffs cleanly
    /// against a prior run, the same reason `doctor` orders by `abs_path`.
    pub fn prefix_conflicts(
        &self,
        citations: &BTreeMap<String, (usize, usize)>,
    ) -> Vec<PrefixConflict> {
        self.declared_by
            .iter()
            .filter_map(|(prefix, declarers)| {
                let definers = self.active_definers.get(prefix)?;
                if definers.len() < 2 {
                    return None;
                }
                // The intersection, derived from `by_token` rather than from the citation
                // stream: a token defined twice is defined twice whether or not anyone has
                // cited it yet, and the latent case is the one the founding instance was.
                let colliding_tokens = self
                    .by_token
                    .iter()
                    .filter(|(token, _)| token.split('-').next() == Some(prefix.as_str()))
                    .filter_map(|(token, refs)| {
                        let mut active: Vec<String> = refs
                            .iter()
                            .filter(|d| d.active)
                            .map(|d| d.artifact_id.clone())
                            .collect();
                        active.sort();
                        active.dedup();
                        if active.len() < 2 {
                            return None;
                        }
                        let (citing_sources, citing_mentions) =
                            citations.get(token).copied().unwrap_or((0, 0));
                        Some(TokenCollision {
                            token: token.clone(),
                            defined_by: active,
                            citing_sources,
                            citing_mentions,
                        })
                    })
                    .collect();
                Some(PrefixConflict {
                    prefix: prefix.clone(),
                    declared_by: declarers.iter().cloned().collect(),
                    defined_by: definers.iter().cloned().collect(),
                    colliding_tokens,
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
    /// A `CitationKind::MalformedQualifier` citation — 2+ qualifier segments before
    /// the entry token. Always report-only, unconditionally: unlike every other arm,
    /// this one never inspects `corpus` or `index`, because the citation is malformed
    /// at the SHAPE level and no lookup could make it a valid edge target.
    MalformedQualifier,
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
        CitationKind::MalformedQualifier => {
            // Shape-level defect, not a lookup failure: no corpus contents could make
            // a double-qualified citation a valid edge target, so this never resolves
            // — see `Outcome::MalformedQualifier` and the bug it fixes.
            Some(Outcome::MalformedQualifier)
        }
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
    /// docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md
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

        let got = idx.prefix_conflicts(&BTreeMap::new());
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].prefix, "T");
        assert_eq!(got[0].declared_by, vec!["fable-tasks".to_string()]);
        assert_eq!(
            got[0].defined_by,
            vec!["fable-tasks".to_string(), "tool-usage".to_string()],
            "both active definers must be named, so the reader knows who to talk to"
        );
        // LOAD-BEARING FIXTURE DETAIL: `T-1` and `T-14` are disjoint, which is what the
        // real corpus looked like on 2026-08-18 and the whole reason that instance was
        // survivable. Change either token to match the other and this becomes the 2026-09-02
        // state instead, silently converting the founding regression guard into a duplicate
        // of `a_conflict_naming_a_twice_defined_token_reports_it`.
        assert!(
            got[0].colliding_tokens.is_empty(),
            "disjoint token spaces: two allocators, nothing broken YET — an empty \
             intersection is the latent state, not a clean bill: {:?}",
            got[0].colliding_tokens
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
            idx.prefix_conflicts(&BTreeMap::new()).is_empty(),
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
            idx.prefix_conflicts(&BTreeMap::new()).is_empty(),
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
        assert!(idx.prefix_conflicts(&BTreeMap::new()).is_empty());
    }

    /// The live case, and the half `prefix_conflicts` could not express before 2026-09-02:
    /// two active artifacts defining the SAME token, which is what actually breaks citations.
    ///
    /// Paired deliberately with `a_declared_prefix_with_a_second_active_definer_is_a_conflict`
    /// above, whose fixture is disjoint. Same conflict shape, opposite intersection — so a
    /// mutation collapsing the two states fails one of them whichever way it goes. Neither
    /// test alone discriminates: the older one passes on an implementation that always
    /// returns an empty intersection, this one on an implementation that never checks
    /// `active`.
    #[test]
    fn a_conflict_naming_a_twice_defined_token_reports_it() {
        let owner = DocExtract {
            declared_prefixes: vec!["T".to_string()],
            ..ex_with_defs(&["T-14", "T-32"])
        };
        let squatter = ex_with_defs(&["T-14", "T-1"]);
        let idx = DefinitionIndex::build([
            ("tool-usage", "active", &owner),
            ("system-retro", "active", &squatter),
        ]);

        let citations = BTreeMap::from([("T-14".to_string(), (12usize, 19usize))]);
        let got = idx.prefix_conflicts(&citations);

        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(
            got[0]
                .colliding_tokens
                .iter()
                .map(|c| c.token.as_str())
                .collect::<Vec<_>>(),
            vec!["T-14"],
            "only the token defined by BOTH may be named — T-1 and T-32 are each single-\
             definer and naming them would send a reader to files that are not in conflict"
        );
        assert_eq!(
            got[0].colliding_tokens[0].defined_by,
            vec!["system-retro".to_string(), "tool-usage".to_string()],
            "the reader needs the two files to reconcile, not just the token"
        );
        assert_eq!(got[0].colliding_tokens[0].citing_sources, 12);
        assert_eq!(got[0].colliding_tokens[0].citing_mentions, 19);
    }

    /// The two counts must stay distinguishable, because the whole reason they both exist is
    /// that one is routinely read as the other.
    ///
    /// A test asserting only that *some* count arrives would pass on an implementation that
    /// wrote the same number into both fields — which is the exact defect
    /// `tracker-hygiene-log:HY-21` measured from the other side, predicting −3 mentions and
    /// observing −2 findings. So the fixture makes them differ and asserts each separately.
    #[test]
    fn the_two_citation_counts_are_different_units() {
        let owner = DocExtract {
            declared_prefixes: vec!["T".to_string()],
            ..ex_with_defs(&["T-17"])
        };
        let squatter = ex_with_defs(&["T-17"]);
        let idx = DefinitionIndex::build([
            ("tool-usage", "active", &owner),
            ("system-retro", "active", &squatter),
        ]);

        // One citing artifact, three mentions inside it — the shape of a guide that
        // demonstrates a token and then uses it again in a worked example.
        let citations = BTreeMap::from([("T-17".to_string(), (1usize, 3usize))]);
        let got = idx.prefix_conflicts(&citations);

        let c = &got[0].colliding_tokens[0];
        assert_eq!(c.citing_sources, 1, "one artifact holds every mention");
        assert_eq!(c.citing_mentions, 3, "three places actually need editing");
        assert_ne!(
            c.citing_sources, c.citing_mentions,
            "if these are ever equal by construction the second field is decoration"
        );
    }

    /// `active` is filtered at TOKEN grain too, not only when building `defined_by`.
    ///
    /// A separate site from `an_archived_co_definer_is_not_a_conflict`: that one guards the
    /// conflict's membership, this one guards the intersection, and a kill at either says
    /// nothing about the other. Without it, an archived companion defining the same token as
    /// its live ledger — the documented endpoint of the compaction ladder — would be reported
    /// as a live collision on every ledger that has ever been compacted.
    #[test]
    fn an_archived_definer_does_not_make_a_token_collide() {
        let live = DocExtract {
            declared_prefixes: vec!["R".to_string()],
            ..ex_with_defs(&["R-4", "R-98"])
        };
        let other_live = ex_with_defs(&["R-98"]);
        let archive = ex_with_defs(&["R-4"]);
        let idx = DefinitionIndex::build([
            ("recon-live", "active", &live),
            ("recon-rival", "active", &other_live),
            ("recon-archive", "archived", &archive),
        ]);

        let got = idx.prefix_conflicts(&BTreeMap::new());
        assert_eq!(
            got[0]
                .colliding_tokens
                .iter()
                .map(|c| c.token.as_str())
                .collect::<Vec<_>>(),
            vec!["R-98"],
            "R-4 is defined by the live ledger and its own archive — the compaction ladder, \
             not a collision. Only R-98 has two ACTIVE definers."
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
