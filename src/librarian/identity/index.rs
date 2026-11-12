//! The key index: every document's key, who owns it, and what a reference resolves to.
//!
//! Pure: documents arrive as [`DocInput`] and nothing is read from disk. `build` computes
//! its maps once (key to document, path to document, owner and token to owned documents).
//! `resolve` is then hash lookups plus a sort of the candidates; it never scans the
//! document set, however many documents one owner holds.
//!
//! A document whose key is invalid is excluded and reported. A key held by two or more
//! documents is reported and resolves to `KeyCollision`; those documents are left out of
//! the key map and of every candidate set, but they stay in the path map, so a bare token
//! defined in their own text still resolves to `Local` there.
//!
//! Ownership: the NEAREST ancestor folder that holds a folder-index document (`readme.md`)
//! owns the document; a folder index starts at its parent's parent. When that nearest index
//! has an invalid or colliding key the document has no owner: the walk stops there and never
//! passes the broken index to a farther ancestor. A folder with no index of its own is
//! skipped, and the walk goes on to the next ancestor.

use std::collections::{HashMap, HashSet};

use super::grammar::Reference;
use super::key::{components, folder_of, is_folder_index, key_of, Key, KeyError};
use super::repo_name::RepoName;
use crate::librarian::tools::link_scan::extract::entry_sections;

/// One document handed to [`KeyIndex::build`].
///
/// Precondition: `rel_path` is a canonical file path relative to the repo root, and each
/// file is listed once. Two spellings of one file (`./a/x.md` and `a/x.md`) are two
/// documents with one key, and are reported as a `DuplicateKey`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocInput {
    pub rel_path: String,
    pub text: String,
}

/// The ids of the entry sections `text` defines, in document order (a repeated id repeats).
pub fn defined_tokens(text: &str) -> Vec<String> {
    entry_sections(text).into_iter().map(|s| s.id).collect()
}

/// Something wrong with the document set itself, found while building the index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexProblem {
    /// A document whose path gives no valid key; it is not in the index.
    InvalidKey(KeyError),
    /// Two or more documents with one key; `rel_paths` lists them all, sorted.
    DuplicateKey { key: Key, rel_paths: Vec<String> },
}

/// What a reference points at, or why it points at nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// `[[key]]` names this document.
    File { dst: Key },
    /// `[[key:TOKEN]]` names the one document under `key` that defines `token`.
    Entry { dst: Key, token: String },
    /// A bare token the source document defines itself.
    Local { token: String },
    /// A bare token the source document does not define: not a citation, ignored by callers.
    NotLocal,
    /// No document has this key.
    UnknownKey { key: Key },
    /// Two or more documents have this key.
    KeyCollision { key: Key, rel_paths: Vec<String> },
    /// The key is known but no document under it defines the token.
    UndefinedToken { key: Key, token: String },
    /// Several documents under `key` define `token`; `candidates` is sorted.
    AmbiguousToken {
        key: Key,
        token: String,
        candidates: Vec<Key>,
    },
    /// The reference names another repo; this index cannot resolve it.
    CrossRepo { repo: RepoName },
    /// The wiki link is outside the grammar.
    Malformed { raw: String },
}

/// A document with a valid key.
#[derive(Debug)]
struct Doc {
    rel_path: String,
    key: Key,
    /// Ids this document defines; a set, so a token defined twice is one candidate.
    tokens: HashSet<String>,
}

#[derive(Debug)]
pub struct KeyIndex {
    this_repo: RepoName,
    docs: Vec<Doc>,
    /// Keys held by exactly one document.
    by_key: HashMap<Key, usize>,
    /// Normalised path of every valid-key document, colliding ones included: a document
    /// with a colliding key still resolves the bare tokens its own text defines.
    by_path: HashMap<String, usize>,
    /// Keys held by two or more documents, with their sorted paths.
    collisions: HashMap<Key, Vec<String>>,
    owners: HashMap<Key, Key>,
    /// Owner key, then token, to the owned documents that define it (unambiguous keys only).
    owned_tokens: HashMap<Key, HashMap<String, Vec<usize>>>,
    problems: Vec<IndexProblem>,
}

/// The path as `key_of` reads it: components joined by `/`.
fn normalised(rel_path: &str) -> String {
    components(rel_path).join("/")
}

impl KeyIndex {
    pub fn build(this_repo: RepoName, docs: impl IntoIterator<Item = DocInput>) -> KeyIndex {
        let mut invalid: Vec<(String, KeyError)> = Vec::new();
        let mut valid: Vec<Doc> = Vec::new();
        for d in docs {
            match key_of(&d.rel_path) {
                Ok(key) => valid.push(Doc {
                    tokens: defined_tokens(&d.text).into_iter().collect(),
                    rel_path: d.rel_path,
                    key,
                }),
                Err(e) => invalid.push((d.rel_path, e)),
            }
        }
        // Sorted up front so every map below is built in one order whatever the input order.
        valid.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

        let mut holders: HashMap<&Key, Vec<usize>> = HashMap::new();
        for (i, d) in valid.iter().enumerate() {
            holders.entry(&d.key).or_default().push(i);
        }
        let mut by_key = HashMap::new();
        let mut collisions = HashMap::new();
        for (key, idxs) in holders {
            if let [only] = idxs[..] {
                by_key.insert(key.clone(), only);
            } else {
                let paths: Vec<String> = idxs.iter().map(|&i| valid[i].rel_path.clone()).collect();
                collisions.insert(key.clone(), paths);
            }
        }
        let by_path = valid
            .iter()
            .enumerate()
            .map(|(i, d)| (normalised(&d.rel_path), i))
            .collect();

        // Folder (normalised; "" is the repo root) to what its folder-index document offers:
        // `Some(key)` for a usable one, `None` for one whose key is invalid or collides.
        let usable = |d: &Doc| by_key.contains_key(&d.key);
        let mut folder_index: HashMap<String, Option<&Key>> = HashMap::new();
        for d in valid.iter().filter(|d| is_folder_index(&d.rel_path)) {
            let offered = usable(d).then_some(&d.key);
            let slot = folder_index
                .entry(folder_of(&d.rel_path))
                .or_insert(offered);
            // Two index documents in one folder (`README.md`, `readme.md`) collide: no owner.
            if offered.is_none() {
                *slot = None;
            }
        }
        for (rel_path, _) in invalid.iter().filter(|(p, _)| is_folder_index(p)) {
            folder_index.insert(folder_of(rel_path), None);
        }
        let mut owners = HashMap::new();
        let mut owned_tokens: HashMap<Key, HashMap<String, Vec<usize>>> = HashMap::new();
        for (i, d) in valid.iter().enumerate().filter(|(_, d)| usable(d)) {
            let parts = components(&d.rel_path);
            let own_folder_depth = parts.len() - 1;
            // A folder index starts at its parent's parent: it never owns itself.
            let start = if is_folder_index(&d.rel_path) {
                own_folder_depth.checked_sub(1)
            } else {
                Some(own_folder_depth)
            };
            // The NEAREST folder with an index document decides. When that index is unusable
            // the walk stops with no owner: it must not hand the document to a farther index.
            let nearest = start.and_then(|s| {
                (0..=s)
                    .rev()
                    .find_map(|depth| folder_index.get(&parts[..depth].join("/")))
            });
            if let Some(&Some(owner)) = nearest {
                owners.insert(d.key.clone(), owner.clone());
                let by_token = owned_tokens.entry(owner.clone()).or_default();
                for token in &d.tokens {
                    by_token.entry(token.clone()).or_default().push(i);
                }
            }
        }

        let mut problems: Vec<IndexProblem> = Vec::new();
        invalid.sort_by(|a, b| a.0.cmp(&b.0));
        problems.extend(
            invalid
                .into_iter()
                .map(|(_, e)| IndexProblem::InvalidKey(e)),
        );
        let mut dups: Vec<_> = collisions.iter().collect();
        dups.sort_by(|a, b| a.0.cmp(b.0));
        problems.extend(
            dups.into_iter()
                .map(|(key, rel_paths)| IndexProblem::DuplicateKey {
                    key: key.clone(),
                    rel_paths: rel_paths.clone(),
                }),
        );

        KeyIndex {
            this_repo,
            docs: valid,
            by_key,
            by_path,
            collisions,
            owners,
            owned_tokens,
            problems,
        }
    }

    pub fn problems(&self) -> &[IndexProblem] {
        &self.problems
    }

    /// The path of the one document that holds `key`; `None` when none or several do.
    pub fn rel_path(&self, key: &Key) -> Option<&str> {
        self.by_key
            .get(key)
            .map(|&i| self.docs[i].rel_path.as_str())
    }

    /// The key of the nearest ancestor folder index above the document with `key`.
    pub fn owner(&self, key: &Key) -> Option<&Key> {
        self.owners.get(key)
    }

    /// What `reference`, written in the document at `src_rel_path`, points at.
    pub fn resolve(&self, src_rel_path: &str, reference: &Reference) -> Resolution {
        match reference {
            Reference::Malformed { raw } => Resolution::Malformed { raw: raw.clone() },
            Reference::Bare { token } => {
                let defined_here = self
                    .by_path
                    .get(&normalised(src_rel_path))
                    .is_some_and(|&i| self.docs[i].tokens.contains(token));
                if defined_here {
                    Resolution::Local {
                        token: token.clone(),
                    }
                } else {
                    Resolution::NotLocal
                }
            }
            Reference::File { repo, key } => self
                .foreign(repo)
                .or_else(|| self.unusable(key))
                .unwrap_or_else(|| Resolution::File { dst: key.clone() }),
            Reference::Entry { repo, key, token } => {
                if let Some(r) = self.foreign(repo).or_else(|| self.unusable(key)) {
                    return r;
                }
                let mut candidates: Vec<Key> = self
                    .by_key
                    .get(key)
                    .filter(|&&i| self.docs[i].tokens.contains(token))
                    .into_iter()
                    .chain(
                        self.owned_tokens
                            .get(key)
                            .and_then(|by_token| by_token.get(token))
                            .into_iter()
                            .flatten(),
                    )
                    .map(|&i| self.docs[i].key.clone())
                    .collect();
                candidates.sort();
                match candidates.len() {
                    0 => Resolution::UndefinedToken {
                        key: key.clone(),
                        token: token.clone(),
                    },
                    1 => Resolution::Entry {
                        dst: candidates.remove(0),
                        token: token.clone(),
                    },
                    _ => Resolution::AmbiguousToken {
                        key: key.clone(),
                        token: token.clone(),
                        candidates,
                    },
                }
            }
        }
    }

    /// `CrossRepo` when `repo` names a repo other than this one.
    fn foreign(&self, repo: &Option<RepoName>) -> Option<Resolution> {
        repo.as_ref()
            .filter(|r| **r != self.this_repo)
            .map(|r| Resolution::CrossRepo { repo: r.clone() })
    }

    /// Why `key` cannot be resolved (unknown or colliding); `None` when one document holds it.
    fn unusable(&self, key: &Key) -> Option<Resolution> {
        if self.by_key.contains_key(key) {
            None
        } else if let Some(rel_paths) = self.collisions.get(key) {
            Some(Resolution::KeyCollision {
                key: key.clone(),
                rel_paths: rel_paths.clone(),
            })
        } else {
            Some(Resolution::UnknownKey { key: key.clone() })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(p: &str, t: &str) -> DocInput {
        DocInput {
            rel_path: p.into(),
            text: t.into(),
        }
    }
    fn k(s: &str) -> Key {
        Key::parse(s).unwrap()
    }
    fn repo() -> RepoName {
        RepoName::parse("codescout").unwrap()
    }
    fn index() -> KeyIndex {
        KeyIndex::build(
            repo(),
            [
                doc("docs/trackers/ledgers/readme.md", "# Ledgers\n"),
                doc(
                    "docs/trackers/ledgers/frictions/readme.md",
                    "# Frictions\n\n## FR-1 — in the index\n",
                ),
                doc(
                    "docs/trackers/ledgers/frictions/2026-11-11-fr-two.md",
                    "## FR-2 — own file\n",
                ),
                doc(
                    "docs/trackers/ledgers/frictions/archive/2026-10-01-fr-three.md",
                    "## FR-3 — archived\n",
                ),
                doc("docs/trackers/ledgers/logs/a-log.md", "## F-1 — a\n"),
                doc("docs/trackers/ledgers/logs/b-log.md", "## F-1 — b\n"),
                doc("docs/one/dup.md", ""),
                doc("docs/two/dup.md", ""),
                doc("docs/Bad Name.md", ""),
            ],
        )
    }
    fn entry(key: &str, token: &str) -> Reference {
        Reference::Entry {
            repo: None,
            key: k(key),
            token: token.into(),
        }
    }
    fn file(key: &str) -> Reference {
        Reference::File {
            repo: None,
            key: k(key),
        }
    }

    #[test]
    fn nearest_folder_index_owns_its_descendants_tokens() {
        let ix = index();
        assert_eq!(ix.owner(&k("2026-10-01-fr-three")), Some(&k("frictions")));
        assert_eq!(ix.owner(&k("frictions")), Some(&k("ledgers")));
        let src = "docs/x.md";
        assert_eq!(
            ix.resolve(src, &entry("frictions", "FR-1")),
            Resolution::Entry {
                dst: k("frictions"),
                token: "FR-1".into()
            }
        );
        assert_eq!(
            ix.resolve(src, &entry("frictions", "FR-2")),
            Resolution::Entry {
                dst: k("2026-11-11-fr-two"),
                token: "FR-2".into()
            }
        );
        assert_eq!(
            ix.resolve(src, &entry("frictions", "FR-3")),
            Resolution::Entry {
                dst: k("2026-10-01-fr-three"),
                token: "FR-3".into()
            }
        );
        // `ledgers` is not the NEAREST index of the friction entries, so it owns none of them.
        assert!(matches!(
            ix.resolve(src, &entry("ledgers", "FR-2")),
            Resolution::UndefinedToken { .. }
        ));
    }

    #[test]
    fn a_token_defined_twice_under_one_owner_is_ambiguous_not_guessed() {
        let ix = KeyIndex::build(
            repo(),
            [
                doc("logs/readme.md", ""),
                doc("logs/a-log.md", "## F-1 — a\n"),
                doc("logs/b-log.md", "## F-1 — b\n"),
            ],
        );
        assert!(matches!(
            ix.resolve("x.md", &entry("logs", "F-1")),
            Resolution::AmbiguousToken { candidates, .. } if candidates.len() == 2
        ));
        assert_eq!(
            ix.resolve("x.md", &entry("a-log", "F-1")),
            Resolution::Entry {
                dst: k("a-log"),
                token: "F-1".into()
            }
        );
    }

    #[test]
    fn bare_tokens_are_local_only() {
        let ix = index();
        let src = "docs/trackers/ledgers/logs/a-log.md";
        assert_eq!(
            ix.resolve(
                src,
                &Reference::Bare {
                    token: "F-1".into()
                }
            ),
            Resolution::Local {
                token: "F-1".into()
            }
        );
        assert_eq!(
            ix.resolve(
                src,
                &Reference::Bare {
                    token: "FR-1".into()
                }
            ),
            Resolution::NotLocal
        );
        assert_eq!(
            ix.resolve(
                src,
                &Reference::Bare {
                    token: "UTF-8".into()
                }
            ),
            Resolution::NotLocal
        );
    }

    #[test]
    fn collisions_invalid_keys_repos_and_unknowns() {
        let ix = index();
        assert!(ix.problems().iter().any(|p| matches!(p, IndexProblem::DuplicateKey { key, rel_paths } if key.as_str() == "dup" && rel_paths.len() == 2)));
        assert!(ix
            .problems()
            .iter()
            .any(|p| matches!(p, IndexProblem::InvalidKey(_))));
        assert!(matches!(
            ix.resolve("x.md", &file("dup")),
            Resolution::KeyCollision { .. }
        ));
        assert_eq!(
            ix.resolve("x.md", &file("nope")),
            Resolution::UnknownKey { key: k("nope") }
        );
        let here = Reference::File {
            repo: Some(RepoName::parse("codescout").unwrap()),
            key: k("frictions"),
        };
        assert_eq!(
            ix.resolve("x.md", &here),
            Resolution::File {
                dst: k("frictions")
            }
        );
        let there = Reference::File {
            repo: Some(RepoName::parse("claude-plugins").unwrap()),
            key: k("frictions"),
        };
        assert!(matches!(
            ix.resolve("x.md", &there),
            Resolution::CrossRepo { .. }
        ));
    }

    // Ruling 2: a colliding key lists every holder (sorted), resolves to KeyCollision for
    // File and Entry alike, and has no path and no owner.
    #[test]
    fn a_colliding_key_lists_all_holders_and_has_no_path_or_owner() {
        let ix = KeyIndex::build(
            repo(),
            [
                doc("m/readme.md", ""),
                doc("m/dup.md", "## T-1 — one\n"),
                doc("b/dup.md", "## T-1 — two\n"),
                doc("a/dup.md", "## T-1 — three\n"),
            ],
        );
        let want = vec!["a/dup.md".to_string(), "b/dup.md".into(), "m/dup.md".into()];
        assert_eq!(
            ix.problems(),
            [IndexProblem::DuplicateKey {
                key: k("dup"),
                rel_paths: want.clone()
            }]
        );
        let collision = Resolution::KeyCollision {
            key: k("dup"),
            rel_paths: want,
        };
        assert_eq!(ix.resolve("x.md", &file("dup")), collision);
        assert_eq!(ix.resolve("x.md", &entry("dup", "T-1")), collision);
        assert_eq!(ix.rel_path(&k("dup")), None);
        assert_eq!(ix.owner(&k("dup")), None);
        // The colliding docs are excluded from the owner's candidate set.
        assert!(matches!(
            ix.resolve("x.md", &entry("m", "T-1")),
            Resolution::UndefinedToken { .. }
        ));
    }

    // Ruling 3: an unknown source is no obstacle to File/Entry, and a Bare token there is NotLocal.
    #[test]
    fn a_source_outside_the_index_resolves_files_normally_and_bare_tokens_not_at_all() {
        let ix = index();
        let bare = Reference::Bare {
            token: "F-1".into(),
        };
        assert_eq!(
            ix.resolve("nowhere/unknown.md", &bare),
            Resolution::NotLocal
        );
        // `docs/Bad Name.md` has an invalid key, so it is not in the index either.
        assert_eq!(ix.resolve("docs/Bad Name.md", &bare), Resolution::NotLocal);
        assert_eq!(
            ix.resolve("nowhere/unknown.md", &file("frictions")),
            Resolution::File {
                dst: k("frictions")
            }
        );
        // A differently spelled path to a known source is the same source.
        let src = r".\docs//trackers\ledgers/logs/a-log.md";
        assert_eq!(
            ix.resolve(src, &bare),
            Resolution::Local {
                token: "F-1".into()
            }
        );
    }

    // Ruling 4: sorted problems, and the same output for any input order.
    #[test]
    fn problems_are_sorted_and_the_input_order_does_not_matter() {
        let docs = vec![
            doc("z/dup.md", ""),
            doc("Zed Bad.md", ""),
            doc("a/dup.md", ""),
            doc("b/other.md", ""),
            doc("c/other.md", ""),
            doc("Alpha Bad.md", ""),
            doc("logs/readme.md", ""),
            doc("logs/a-log.md", "## F-1 — a\n"),
            doc("logs/b-log.md", "## F-1 — b\n"),
        ];
        let forward = KeyIndex::build(repo(), docs.clone());
        let mut shuffled = docs;
        shuffled.reverse();
        shuffled.swap(1, 5);
        let backward = KeyIndex::build(repo(), shuffled);
        let paths_of = |ix: &KeyIndex| -> Vec<String> {
            ix.problems()
                .iter()
                .map(|p| match p {
                    IndexProblem::InvalidKey(KeyError::NonCanonical { rel_path, .. }) => {
                        format!("invalid {rel_path}")
                    }
                    IndexProblem::InvalidKey(other) => format!("invalid {other:?}"),
                    IndexProblem::DuplicateKey { key, rel_paths } => {
                        format!("dup {key} {rel_paths:?}")
                    }
                })
                .collect()
        };
        assert_eq!(
            paths_of(&forward),
            vec![
                "invalid Alpha Bad.md".to_string(),
                "invalid Zed Bad.md".into(),
                r#"dup dup ["a/dup.md", "z/dup.md"]"#.into(),
                r#"dup other ["b/other.md", "c/other.md"]"#.into(),
            ]
        );
        assert_eq!(forward.problems(), backward.problems());
        for r in [entry("logs", "F-1"), file("dup"), entry("a-log", "F-1")] {
            assert_eq!(forward.resolve("x.md", &r), backward.resolve("x.md", &r));
        }
        assert_eq!(forward.owner(&k("a-log")), backward.owner(&k("a-log")));
        assert!(matches!(
            forward.resolve("x.md", &entry("logs", "F-1")),
            Resolution::AmbiguousToken { candidates, .. }
                if candidates == vec![k("a-log"), k("b-log")]
        ));
    }

    // Ruling 5 (corrected): the NEAREST folder index decides. When it has an invalid or a
    // colliding key its descendants have no owner; the walk does not pass it to a farther
    // index. A folder with no index of its own is skipped. A folder index never owns itself.
    #[test]
    fn a_broken_nearest_index_stops_the_walk_and_a_missing_one_does_not() {
        let ix = KeyIndex::build(
            repo(),
            [
                doc("README.md", "## R-1 — root\n"),
                doc("a/readme.md", ""),
                // key `b` collides with `z/b.md`, so this folder's index owns nothing.
                doc("a/b/readme.md", ""),
                doc("z/b.md", ""),
                doc("a/b/c.md", "## T-1 — behind a colliding index\n"),
                // no index in `deeper`: the walk reaches `a/b`, which is broken, and stops.
                doc("a/b/deeper/e.md", ""),
                // a folder index starts at its parent's parent: `a/b`, broken.
                doc("a/b/sub/readme.md", ""),
                // folder name gives an invalid key, so this folder's index owns nothing.
                doc("a/Bad Dir/readme.md", ""),
                doc("a/Bad Dir/d.md", ""),
                // no index in `plain`: the walk goes on to `a`.
                doc("a/plain/f.md", ""),
                doc("top.md", ""),
                doc("q/loose.md", ""),
            ],
        );
        for broken in ["c", "e", "sub", "d"] {
            assert_eq!(ix.owner(&k(broken)), None, "{broken}");
        }
        assert_eq!(ix.owner(&k("f")), Some(&k("a")));
        assert_eq!(ix.owner(&k("a")), Some(&k("readme")));
        assert_eq!(ix.owner(&k("top")), Some(&k("readme")));
        assert_eq!(ix.owner(&k("loose")), Some(&k("readme")));
        assert_eq!(ix.owner(&k("readme")), None);
        assert_eq!(ix.rel_path(&k("readme")), Some("README.md"));
        assert_eq!(ix.rel_path(&k("c")), Some("a/b/c.md"));
        // Failing loudly: no farther index picks up the entry behind the broken one.
        for owner in ["readme", "a"] {
            assert!(
                matches!(
                    ix.resolve("x.md", &entry(owner, "T-1")),
                    Resolution::UndefinedToken { .. }
                ),
                "{owner}"
            );
        }
        assert!(ix
            .problems()
            .iter()
            .any(|p| matches!(p, IndexProblem::DuplicateKey { key, .. } if key.as_str() == "b")));
    }

    // A directory-shaped path (`docs/README.md/`) is an invalid key, but it is not an index
    // document: it must not turn its folder's real index into a broken one.
    #[test]
    fn a_directory_shaped_invalid_path_does_not_break_its_folders_index() {
        let ix = KeyIndex::build(
            repo(),
            [
                doc("docs/readme.md", ""),
                doc("docs/README.md/", ""),
                doc("docs/a.md", "## T-1 — in docs\n"),
            ],
        );
        assert_eq!(ix.owner(&k("a")), Some(&k("docs")));
        assert!(ix
            .problems()
            .iter()
            .any(|p| matches!(p, IndexProblem::InvalidKey(KeyError::NotMarkdown(_)))));
    }

    // Backslash input paths own exactly as the forward-slash spelling does.
    #[test]
    fn backslash_input_paths_give_the_same_owners() {
        let set = |sep: &str| -> Vec<DocInput> {
            [
                ("README.md", "## R-1 — root\n"),
                ("docs/readme.md", ""),
                ("docs/logs/readme.md", "## F-1 — in the index\n"),
                ("docs/logs/archive/a-log.md", "## F-2 — archived\n"),
                ("docs/other/b-doc.md", ""),
            ]
            .into_iter()
            .map(|(p, t)| doc(&p.replace('/', sep), t))
            .collect()
        };
        let forward = KeyIndex::build(repo(), set("/"));
        let mixed = {
            let mut docs = set("/");
            // Every other document spelled with backslashes: a mixed set.
            for d in docs.iter_mut().step_by(2) {
                d.rel_path = d.rel_path.replace('/', "\\");
            }
            KeyIndex::build(repo(), docs)
        };
        let backslashed = KeyIndex::build(repo(), set("\\"));
        for ix in [&mixed, &backslashed] {
            for key in ["readme", "docs", "logs", "a-log", "b-doc"] {
                assert_eq!(ix.owner(&k(key)), forward.owner(&k(key)), "{key}");
            }
            assert_eq!(ix.owner(&k("a-log")), Some(&k("logs")));
            assert_eq!(ix.owner(&k("b-doc")), Some(&k("docs")));
            assert_eq!(
                ix.resolve("x.md", &entry("logs", "F-2")),
                Resolution::Entry {
                    dst: k("a-log"),
                    token: "F-2".into()
                }
            );
        }
    }

    // Documents the DocInput precondition (canonical path, each file once): two spellings of
    // one file are two documents with one key, so they are reported, not merged.
    #[test]
    fn two_spellings_of_one_file_are_reported_as_a_duplicate_key() {
        let ix = KeyIndex::build(repo(), [doc("a/x.md", ""), doc("./a/x.md", "")]);
        assert_eq!(
            ix.problems(),
            [IndexProblem::DuplicateKey {
                key: k("x"),
                rel_paths: vec!["./a/x.md".to_string(), "a/x.md".into()]
            }]
        );
    }

    // Ruling 2 deviation, adjudicated: a colliding-key document stays in the path map, so
    // the bare tokens its own text defines still resolve to Local; its key still collides.
    #[test]
    fn a_colliding_key_document_still_resolves_its_own_bare_tokens() {
        let ix = KeyIndex::build(
            repo(),
            [doc("a/dup.md", "## T-1 — a\n"), doc("b/dup.md", "")],
        );
        let bare = Reference::Bare {
            token: "T-1".into(),
        };
        assert_eq!(
            ix.resolve("a/dup.md", &bare),
            Resolution::Local {
                token: "T-1".into()
            }
        );
        assert_eq!(ix.resolve("b/dup.md", &bare), Resolution::NotLocal);
        assert!(matches!(
            ix.resolve("a/dup.md", &file("dup")),
            Resolution::KeyCollision { .. }
        ));
    }

    // Ruling 6: an entry reference needs a known key first, then the candidates decide.
    #[test]
    fn entry_resolution_order_unknown_key_before_undefined_token() {
        let ix = index();
        let src = "x.md";
        assert_eq!(
            ix.resolve(src, &entry("nope", "FR-1")),
            Resolution::UnknownKey { key: k("nope") }
        );
        assert_eq!(
            ix.resolve(src, &entry("frictions", "FR-99")),
            Resolution::UndefinedToken {
                key: k("frictions"),
                token: "FR-99".into()
            }
        );
        // A `File` reference to a known key names the document.
        assert_eq!(
            ix.resolve(src, &file("2026-11-11-fr-two")),
            Resolution::File {
                dst: k("2026-11-11-fr-two")
            }
        );
        // An owner key that no document carries is unknown, whatever it would own.
        let orphan = KeyIndex::build(repo(), [doc("only/a.md", "## T-1 — a\n")]);
        assert_eq!(
            orphan.resolve(src, &entry("only", "T-1")),
            Resolution::UnknownKey { key: k("only") }
        );
        assert_eq!(
            ix.resolve(src, &Reference::Malformed { raw: " [[".into() }),
            Resolution::Malformed { raw: " [[".into() }
        );
    }

    // Ruling 7: this repo's own name is local; another repo is CrossRepo without a key lookup.
    #[test]
    fn a_foreign_repo_is_cross_repo_even_for_an_unknown_key() {
        let ix = index();
        let other = RepoName::parse("claude-plugins").unwrap();
        let foreign = Reference::Entry {
            repo: Some(other.clone()),
            key: k("nope"),
            token: "X-1".into(),
        };
        assert_eq!(
            ix.resolve("x.md", &foreign),
            Resolution::CrossRepo { repo: other }
        );
        let own = Reference::Entry {
            repo: Some(repo()),
            key: k("frictions"),
            token: "FR-1".into(),
        };
        assert_eq!(
            ix.resolve("x.md", &own),
            Resolution::Entry {
                dst: k("frictions"),
                token: "FR-1".into()
            }
        );
    }

    // Ruling 8: definitions in document order; a token defined twice counts the document once.
    #[test]
    fn a_token_defined_twice_in_one_document_is_one_candidate() {
        let text = "## F-2 — b\n\n## F-1 — a\n\n## F-1 — again\n";
        assert_eq!(
            defined_tokens(text),
            vec!["F-2".to_string(), "F-1".into(), "F-1".into()]
        );
        let ix = KeyIndex::build(repo(), [doc("logs/readme.md", text)]);
        assert_eq!(
            ix.resolve("x.md", &entry("logs", "F-1")),
            Resolution::Entry {
                dst: k("logs"),
                token: "F-1".into()
            }
        );
    }
}
