//! The retrieval-engine registry — Layer 1 of
//! `docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md`.
//!
//! Several independent engines deliver guidance into one agent's context window
//! and stamp one shared [`GuideLedger`](crate::tools::guide_ledger::GuideLedger).
//! Until this module existed they were inlined branches: nothing enumerated
//! them, nothing could say which engine owned a given ledger key, and the only
//! thing keeping two of them from colliding was a single hand-written pairwise
//! test.
//!
//! # What the scout found, and how it changed the design
//!
//! The spec's Layer 1 proposed a `ledger_prefix` per engine and a gate reading
//! *"no registered prefix is a prefix of another"*. **That gate is wrong**, and
//! enumerating the real write sites before writing it is what caught it.
//!
//! Production has **seven** ledger writers, not two:
//!
//! | site | key written | engine |
//! |---|---|---|
//! | `guide_emit::guide_blocks_for` (non-declaring) | `<topic>` | `guide-sections` |
//! | `guide_emit::guide_blocks_for` (non-declaring, body over the auto-inject bound) | `<topic>#<pointer>` | `guide-sections` |
//! | `guide_emit::guide_blocks_for` (fallback) | `<topic>#<preamble>` | `guide-sections` |
//! | `guide_emit::guide_blocks_for` (matched) | `<topic>#<heading>` | `guide-sections` |
//! | `tools::guide` (explicit fetch) | `<topic>` | `guide-sections`, **pull** |
//! | `engines::emitters::emit_session_opener` | `project-activation-bootstrap` | `session-opener` |
//! | `engines::emitters::emit_operator_rules` | `op:OP-N` | `operator-rules` |
//!
//! `session-opener` writes a **bare topic name**, which `guide-sections` also
//! owns. That overlap is deliberate: keying the opener finer would desync the
//! trigger from what `GuideLedger::re_arm` re-arms, and no delivery assertion
//! can see that — which is why
//! `emitters::tests::the_opener_stamps_the_bare_topic_name` pins the key shape
//! separately from the trigger. (This paragraph quoted the prose comment at
//! the original site until Plan 3 deleted it with `call_content`'s inlined
//! branch; the reason now lives in that test's doc comment, so it is restated
//! here rather than quoted.) A global prefix-disjointness gate would fail on a
//! correct, load-bearing arrangement.
//!
//! So disjointness is conditioned on [`Corpus`]: **two engines drawing from
//! different corpora must own disjoint key spaces**; two engines sharing one
//! corpus may share a namespace, because a collision there re-delivers the same
//! bytes rather than confusing two different bodies of knowledge. That is the
//! property [`tests::engines_over_different_corpora_own_disjoint_key_spaces`]
//! enforces, and it subsumes `operator_rules::route`'s pairwise
//! `op_keys_collide_with_no_guide_key`.
//!
//! # What this module is NOT
//!
//! It **does** drive delivery in production as of Plan 3 — `call_content`
//! calls [`coordinator::run_post`], which fans out through the `emit_post`
//! pointers below. What it is still not is *complete*: [`Mode::Unmanaged`]
//! is the honest state for an engine that ships and participates in nothing,
//! which is where `craft-skills` sits today.
//!
//! Three of the four rows carry a wired `emit_post`; the fourth carries
//! `None` and is reached by no phase, which
//! `registry_order_is_delivery_precedence` pins in both directions —
//! `take(3).all(|e| e.emit_post.is_some())` and `ENGINES[3].emit_post
//! .is_none()`. **Wired is not the same as invoked on every call**, and the
//! gap is corpus exclusivity rather than the engine's own trigger:
//! `run_post_in` skips a row whose [`Corpus`] an earlier row already claimed,
//! `continue`-ing *before* it calls `emit`. So when `session-opener` claims
//! [`Corpus::CompiledGuides`], `guide-sections`' emitter is never entered at
//! all. Of the rows that are entered, each may still decline on its own terms
//! — `session-opener` when its topic is already stamped (the most frequent
//! decline of the three), `guide-sections` when the tool names no topic,
//! `operator-rules` when the tool opted out of `selector_key`.

pub mod coordinator;
pub mod emitters;

use crate::prompts::SESSION_OPENING_GUIDE;

/// The key an engine retrieves on. This is the family's discriminator: it
/// determines the corpus, the retrieval mechanism, and what the ledger must
/// remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalKey {
    /// The shape of the call being made — `doc.update`, `edit_file`.
    CallShape,
    /// Where the session is, not what it asked. The opener fires on the first
    /// eligible call regardless of shape.
    ///
    /// **This variant is the scout's find.** The spec enumerated six engines by
    /// retrieval key and this was not among them, because the opener's key is
    /// invisible in the key space it writes into — it stamps a bare topic name
    /// exactly like `guide-sections` does.
    SessionPhase,
    /// The human operating the session.
    Operator,
    /// What the agent is trying to do, as opposed to the call it just made.
    TaskIntent,
}

/// Where an engine's guidance comes from. Two engines sharing a corpus may
/// share a ledger namespace; two drawing from different corpora may not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Corpus {
    /// The `include_str!`'d guides under `src/prompts/guides/`.
    CompiledGuides,
    /// `docs/trackers/operator-rules.md`, compiled in via `operator_rules::corpus`.
    OperatorLedger,
    /// `SKILL.md` files and buddy specialists, loaded by the harness.
    SkillFiles,
}

/// How an engine delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Emitted alongside a tool result the agent did not ask for it on.
    Push,
    /// Delivered only when explicitly requested.
    Pull,
    /// Both paths exist and share one ledger namespace.
    Both,
    /// Ships, spends the same context window, and participates in no ledger
    /// and no budget. Not a TODO marker — a fact, and the one this registry
    /// exists to make visible rather than to hide.
    Unmanaged,
}

/// One registered engine.
pub struct EngineDecl {
    /// Stable identifier, kebab-case.
    pub id: &'static str,
    pub key: RetrievalKey,
    pub corpus: Corpus,
    pub mode: Mode,
    /// Module paths that write this engine's keys into the shared ledger.
    /// Empty for [`Mode::Unmanaged`]. Documentation for a reader tracing a
    /// stamp back to its author; not consulted at run time.
    ///
    /// **Nothing enforces this field**, and it has already been wrong: the
    /// opener and rule rows named `tools::core::types` until Plan 3 moved
    /// both writes into `engines::emitters`, and the review that caught it
    /// found the prose table above had been repaired in the same commit that
    /// left these two rows stale. The only read is
    /// `tests::an_unmanaged_engine_is_registered_and_owns_nothing`'s
    /// `assert!(e.writes_at.is_empty())`, which exercises the one row that
    /// cannot be stale — `craft-skills`, whose list is empty by definition.
    /// So when you move a `ledger.insert` call, this field is the thing no
    /// compiler and no test will remind you about.
    pub writes_at: &'static [&'static str],
    /// Whether a ledger key belongs to this engine.
    ///
    /// A function pointer rather than a prefix string because two of the three
    /// ledger-writing engines do not use a prefix at all — see the module docs.
    pub owns_key: fn(&str) -> bool,
    /// This engine's post-phase emitter, or `None` for an engine no phase
    /// reaches — [`Mode::Unmanaged`]'s honest state. A function pointer
    /// rather than a trait object
    /// because an engine is data, not behaviour with state — and because a
    /// `&'static EngineDecl` must stay `Sync` without a `Box`.
    pub(crate) emit_post: Option<
        fn(
            &crate::engines::coordinator::PostCtx<'_>,
            &mut crate::tools::guide_ledger::GuideLedger,
        ) -> crate::engines::coordinator::Emitted,
    >,
}

impl EngineDecl {
    /// Whether this engine participates in the shared ledger at all.
    pub fn is_ledger_participant(&self) -> bool {
        !matches!(self.mode, Mode::Unmanaged)
    }
}

fn owns_operator_key(key: &str) -> bool {
    key.starts_with("op:")
}

fn owns_session_opener_key(key: &str) -> bool {
    key == SESSION_OPENING_GUIDE
}

/// Any key naming a compiled-in guide topic, whole (`<topic>`) or sliced
/// (`<topic>#<heading>`, `<topic>#<preamble>`, `<topic>#<pointer>`).
///
/// Deliberately checks membership in `GUIDE_TOPICS` rather than merely "has no
/// `op:` prefix": a negative predicate would claim every future engine's
/// namespace by default, so the disjointness gate below could never fail and
/// would be decoration.
fn owns_guide_key(key: &str) -> bool {
    let topic = key.split_once('#').map_or(key, |(t, _)| t);
    crate::prompts::GUIDE_TOPICS.contains(&topic)
}

fn owns_nothing(_key: &str) -> bool {
    false
}

/// Every engine known to this process.
pub static ENGINES: &[EngineDecl] = &[
    EngineDecl {
        id: "session-opener",
        key: RetrievalKey::SessionPhase,
        corpus: Corpus::CompiledGuides,
        mode: Mode::Push,
        writes_at: &["engines::emitters"],
        owns_key: owns_session_opener_key,
        emit_post: Some(crate::engines::emitters::emit_session_opener),
    },
    EngineDecl {
        id: "guide-sections",
        key: RetrievalKey::CallShape,
        corpus: Corpus::CompiledGuides,
        mode: Mode::Both,
        writes_at: &["tools::core::guide_emit", "tools::guide"],
        owns_key: owns_guide_key,
        emit_post: Some(crate::engines::emitters::emit_guide_sections),
    },
    EngineDecl {
        id: "operator-rules",
        key: RetrievalKey::Operator,
        corpus: Corpus::OperatorLedger,
        mode: Mode::Push,
        writes_at: &["engines::emitters"],
        owns_key: owns_operator_key,
        emit_post: Some(crate::engines::emitters::emit_operator_rules),
    },
    EngineDecl {
        id: "craft-skills",
        key: RetrievalKey::TaskIntent,
        corpus: Corpus::SkillFiles,
        mode: Mode::Unmanaged,
        writes_at: &[],
        owns_key: owns_nothing,
        emit_post: None,
    },
];

/// The engines claiming `key`, in registry order.
///
/// Returns a list rather than an `Option` because a shared-corpus overlap is
/// legitimate (`session-opener` inside `guide-sections`), so "exactly one
/// owner" is not the invariant — "no owner from a foreign corpus" is.
pub fn owners_of(key: &str) -> Vec<&'static EngineDecl> {
    ENGINES.iter().filter(|e| (e.owns_key)(key)).collect()
}
/// Gate 1, post half: every `Tool` impl whose `call_content` override never reaches
/// [`coordinator::run_post`], each with the reason it is allowed to.
///
/// The pre phase is total by construction (one production dispatch, which no tool
/// can override); the post phase runs inside the trait's default `call_content`, so
/// an override silently opts its tool out of every engine — no guide block, no
/// operator rule, no session opener, no ledger stamp. Being on this list is a
/// written-down waiver, on `PULL_ONLY_GUIDE_TOPICS`' convention, not an assumed
/// one. `tests::every_call_content_override_reaches_run_post_or_is_declared`
/// enumerates the overrides by parsing `src/` and fails on an override missing
/// from this list and on an entry that no longer bypasses.
pub const POST_PHASE_BYPASSES: &[(&str, &str)] = &[];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Real ledger keys, drawn from the live corpora rather than invented.
    ///
    /// **Load-bearing:** a hand-written key list would be a claim about what the
    /// engines emit, and the gate below would then test that claim rather than
    /// the engines. Both sources are the ones production actually stamps —
    /// `GUIDE_INDEX.ledger_keys()` is what `guide_blocks_for` writes, and
    /// `route::ledger_key` is what `emitters::emit_operator_rules` writes.
    fn live_keys() -> Vec<String> {
        let mut keys: Vec<String> = crate::prompts::guide_index::GUIDE_INDEX.ledger_keys();
        keys.extend(crate::prompts::GUIDE_TOPICS.iter().map(|t| t.to_string()));
        keys.extend(
            crate::operator_rules::corpus::OPERATOR_RULES
                .iter()
                .map(|r| crate::operator_rules::route::ledger_key(&r.id)),
        );
        keys
    }

    /// Gate 2. Subsumes `operator_rules::route`'s pairwise
    /// `op_keys_collide_with_no_guide_key` at N engines instead of 2.
    #[test]
    fn engines_over_different_corpora_own_disjoint_key_spaces() {
        let keys = live_keys();
        assert!(!keys.is_empty(), "the key corpus must not be empty");

        for key in &keys {
            let corpora: HashSet<Corpus> = owners_of(key).iter().map(|e| e.corpus).collect();
            assert!(
                corpora.len() <= 1,
                "ledger key {key:?} is claimed by engines drawing on different corpora \
                 ({corpora:?}). A collision across corpora means one engine's stamp \
                 silences another's unrelated content; within one corpus it merely \
                 re-delivers the same bytes. Rename the namespace, do not widen a \
                 predicate."
            );
        }
    }

    /// Gate 1, at the resolution this layer can actually hold: every key the
    /// live corpora produce has a registered owner.
    ///
    /// **What this does NOT establish.** It is scoped to keys the *registered*
    /// corpora emit, so a brand-new engine writing a brand-new namespace passes
    /// it trivially — the gate cannot see a writer it was never told about.
    /// Closing that needs key construction to go through the registry, which is
    /// Layer 2's job. Stated here so a green run is not read as totality.
    #[test]
    fn every_live_ledger_key_has_a_registered_owner() {
        for key in live_keys() {
            assert!(
                !owners_of(&key).is_empty(),
                "ledger key {key:?} belongs to no registered engine. Add it to \
                 ENGINES, or widen the owning engine's `owns_key`."
            );
        }
    }

    /// The predicates must be able to say *no*. A registry whose members all
    /// return `true` passes both gates above and means nothing — the failure
    /// mode a negative predicate (`!starts_with(\"op:\")`) would have shipped.
    #[test]
    fn no_engine_claims_a_key_from_outside_every_corpus() {
        let foreign = "definitely-not-a-topic-or-a-rule#nope";
        assert!(
            owners_of(foreign).is_empty(),
            "{foreign:?} is owned by {:?}; a predicate that claims arbitrary keys \
             makes the disjointness gate unfalsifiable",
            owners_of(foreign).iter().map(|e| e.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn engine_ids_are_unique() {
        let ids: HashSet<&str> = ENGINES.iter().map(|e| e.id).collect();
        assert_eq!(ids.len(), ENGINES.len(), "duplicate engine id in ENGINES");
    }

    /// The overlap the module docs describe, pinned so that removing it is a
    /// decision rather than an accident: the opener's key sits *inside*
    /// `guide-sections`' namespace, and both engines legitimately claim it.
    ///
    /// If this ever reds because the opener moved to its own namespace, the
    /// site's own comment must move with it — it argues that keying the opener
    /// finer desyncs it from `GuideLedger::re_arm`.
    #[test]
    fn the_session_opener_deliberately_shares_the_guide_namespace() {
        let owners: Vec<&str> = owners_of(SESSION_OPENING_GUIDE)
            .iter()
            .map(|e| e.id)
            .collect();
        assert!(owners.contains(&"session-opener"), "got {owners:?}");
        assert!(owners.contains(&"guide-sections"), "got {owners:?}");
    }

    /// Registry order IS delivery precedence, and the two must not drift.
    ///
    /// The session opener precedes guide-sections because the pre-refactor
    /// `if/else` in `call_content` tried it first — deliberately, so a
    /// one-shot `artifact` call receives the 2.5 KB opener rather than 18 KB
    /// of librarian guide. That `if/else` is gone as of Plan 3; the trade now
    /// lives in this row order plus [`emitters::emit_session_opener`]'s own
    /// trigger. Both draw `Corpus::CompiledGuides`,
    /// so under `run_post_in` the earlier one claims and the later never runs.
    /// Swapping these two rows silently inverts that trade with no other test
    /// failing, which is why the order is pinned here rather than commented.
    ///
    /// The `emit_post` assertion below is a second, independent property:
    /// order alone says nothing about wiring. Reverting any of the first
    /// three rows' `emit_post` to `None` leaves `ids` exactly as it was — the
    /// same four strings in the same order — so only this second assertion
    /// would catch it.
    #[test]
    fn registry_order_is_delivery_precedence() {
        let ids: Vec<&str> = ENGINES.iter().map(|e| e.id).collect();
        assert_eq!(
            ids,
            vec![
                "session-opener",
                "guide-sections",
                "operator-rules",
                "craft-skills"
            ]
        );
        assert!(
            ENGINES.iter().take(3).all(|e| e.emit_post.is_some()),
            "the first three engines must be wired, or Plan 3 has nothing to call"
        );
        // The other half of the same property: `take(3)` is only the right bound
        // while the 4th row stays unwired. `is_ledger_participant()` reads `mode`
        // alone, so wiring `craft-skills` would red nothing without this.
        assert!(
            ENGINES[3].emit_post.is_none(),
            "wiring craft-skills makes take(3) above the wrong bound"
        );
        // Ceiling, not an omission: this pins that each row HAS an emitter, never
        // which one. Comparing `fn` pointers is lint-flagged in Rust, so swapping
        // two rows' emitters is caught by the emitters' own behavioural tests
        // rather than here.
    }

    /// `craft-skills` ships and is counted by no ledger and no budget. The
    /// registry records that rather than omitting it, because an engine absent
    /// from the roster is indistinguishable from one that does not exist.
    #[test]
    fn an_unmanaged_engine_is_registered_and_owns_nothing() {
        let e = ENGINES
            .iter()
            .find(|e| e.id == "craft-skills")
            .expect("craft-skills must stay registered while it remains uncoordinated");
        assert!(!e.is_ledger_participant());
        assert!(e.writes_at.is_empty());
        assert!(
            !(e.owns_key)(SESSION_OPENING_GUIDE),
            "an Unmanaged engine must own no key, or it would shadow a real owner"
        );
    }
    // ---- Gate 5: the family table has one home -------------------------------

    /// The spec that holds the engine table, relative to `docs/superpowers/specs/`.
    const FAMILY_TABLE_HOME: &str = "2026-09-02-retrieval-engine-coordination-design.md";

    /// Whether `text` carries the engine family table: a Markdown table row whose
    /// first three cells are `Engine`, `Retrieval key`, `Corpus`. Keyed on the
    /// header's leading cells rather than the whole row, so the dated
    /// `Status (verified …)` column can be re-verified without blinding the check.
    fn carries_the_family_table(text: &str) -> bool {
        text.lines().any(|line| {
            let Some(row) = line.trim().strip_prefix('|') else {
                return false;
            };
            let cells: Vec<&str> = row.split('|').map(str::trim).collect();
            cells.len() >= 3
                && cells[0] == "Engine"
                && cells[1] == "Retrieval key"
                && cells[2] == "Corpus"
        })
    }

    /// Every `*.md` under `dir` other than `home` (a path relative to `dir`) that
    /// carries the table, as sorted relative paths.
    fn family_table_copies(dir: &std::path::Path, home: &str) -> Vec<String> {
        let mut copies: Vec<String> = walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .map(|e| e.into_path())
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .filter_map(|p| {
                let rel = p
                    .strip_prefix(dir)
                    .ok()?
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&p).ok()?;
                (rel != home && carries_the_family_table(&text)).then_some(rel)
            })
            .collect();
        copies.sort();
        copies
    }

    /// Gate 5. The table moved here from the operator-rules spec on 2026-09-02
    /// because a second copy had already gone stale (§ *Problem 1*); that spec now
    /// cites this gate as what keeps the move from being undone.
    ///
    /// The home must still match the fingerprint, or a re-worded header would
    /// leave the copy check passing on a corpus where it can see nothing.
    #[test]
    fn the_engine_family_table_lives_only_in_the_coordination_spec() {
        let specs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/superpowers/specs");
        let home = std::fs::read_to_string(specs.join(FAMILY_TABLE_HOME))
            .unwrap_or_else(|e| panic!("cannot read the table's home {FAMILY_TABLE_HOME}: {e}"));
        assert!(
            carries_the_family_table(&home),
            "{FAMILY_TABLE_HOME} no longer carries a row starting `| Engine | Retrieval key | \
         Corpus |`; update the fingerprint with the header, or this gate sees no copy anywhere"
        );
        let copies = family_table_copies(&specs, FAMILY_TABLE_HOME);
        assert!(
            copies.is_empty(),
            "the engine family table is canonical in {FAMILY_TABLE_HOME} § The family; these \
         specs carry a copy that will go stale: {copies:?}. Replace each with a pointer."
        );
    }

    /// The copy check must be able to say yes: a pasted table (re-spaced, with a
    /// different status column) is found, the home is not reported, and an
    /// unrelated table that merely has an `Engine` column is not mistaken for it.
    #[test]
    fn a_pasted_copy_of_the_family_table_is_found() {
        let dir = tempfile::tempdir().unwrap();
        let home = "| Engine | Retrieval key | Corpus | Status (verified 2026-09-02) | On the shared surface? |\n|---|---|---|---|---|\n| 1. codescout guiding | call shape | compiled-in guides | shipped | yes |\n";
        std::fs::write(dir.path().join("home.md"), home).unwrap();
        std::fs::write(
            dir.path().join("pasted.md"),
            "# Other spec\n\n|Engine|Retrieval key|Corpus|Status (verified 2026-10-01)|\n|-|-|-|-|\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("unrelated.md"),
            "| Engine | Corpus |\n|---|---|\n| x | y |\n\nThe Engine, Retrieval key and Corpus columns live elsewhere.\n",
        )
        .unwrap();
        assert_eq!(
            family_table_copies(dir.path(), "home.md"),
            vec!["pasted.md".to_string()]
        );
    }

    // ---- Gate 1, post half: no undeclared bypass of run_post -----------------

    /// One `call_content` body: the type whose `impl Tool` holds it (`None` for the
    /// trait's own default) and whether it calls `run_post`.
    #[derive(Debug)]
    struct CallContentSite {
        owner: Option<String>,
        file: String,
        reaches_run_post: bool,
    }

    /// Every `call_content` with a body in `src` that sits in an `impl Tool for T`
    /// or in `trait Tool` itself, found by parsing rather than by text match, so an
    /// `fn call_content(` split across lines or a `run_post(` inside a comment
    /// cannot fool it: only a call expression whose callee ends in `run_post` counts.
    fn call_content_sites(file: &str, src: &str) -> Vec<CallContentSite> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("rust grammar");
        let Some(tree) = parser.parse(src, None) else {
            return Vec::new();
        };
        let bytes = src.as_bytes();
        let text = |n: tree_sitter::Node| n.utf8_text(bytes).unwrap_or("").to_string();
        let last_segment = |s: String| s.rsplit("::").next().unwrap_or("").to_string();
        let calls_run_post = |body: tree_sitter::Node| {
            let mut stack = vec![body];
            while let Some(n) = stack.pop() {
                if n.kind() == "call_expression"
                    && n.child_by_field_name("function")
                        .is_some_and(|f| last_segment(text(f)) == "run_post")
                {
                    return true;
                }
                let mut c = n.walk();
                stack.extend(n.children(&mut c));
            }
            false
        };
        let mut out = Vec::new();
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            let owner: Option<Option<String>> = match node.kind() {
                "impl_item" => node
                    .child_by_field_name("trait")
                    .filter(|t| last_segment(text(*t)) == "Tool")
                    .and_then(|_| node.child_by_field_name("type"))
                    .map(|t| Some(text(t))),
                "trait_item" => node
                    .child_by_field_name("name")
                    .filter(|n| text(*n) == "Tool")
                    .map(|_| None),
                _ => None,
            };
            if let (Some(owner), Some(body)) = (owner, node.child_by_field_name("body")) {
                let mut c = body.walk();
                for item in body.named_children(&mut c) {
                    if item.kind() != "function_item"
                        || !item
                            .child_by_field_name("name")
                            .is_some_and(|n| text(n) == "call_content")
                    {
                        continue;
                    }
                    if let Some(fn_body) = item.child_by_field_name("body") {
                        out.push(CallContentSite {
                            owner: owner.clone(),
                            file: file.to_string(),
                            reaches_run_post: calls_run_post(fn_body),
                        });
                    }
                }
            }
            let mut c = node.walk();
            stack.extend(node.children(&mut c));
        }
        out
    }

    /// One message per override that bypasses `run_post` undeclared, per declared
    /// entry that no longer bypasses, and per entry whose reason is a placeholder.
    fn post_phase_violations(sites: &[CallContentSite], declared: &[(&str, &str)]) -> Vec<String> {
        let mut problems = Vec::new();
        for site in sites {
            let Some(owner) = &site.owner else { continue };
            if !site.reaches_run_post && !declared.iter().any(|(t, _)| t == owner) {
                problems.push(format!(
                    "{}: `impl Tool for {owner}` overrides call_content and never calls run_post, so \
                 no engine emits on its calls. Route it through run_post, or add it to \
                 POST_PHASE_BYPASSES with the reason.",
                    site.file
                ));
            }
        }
        for (ty, reason) in declared {
            if !sites
                .iter()
                .any(|s| s.owner.as_deref() == Some(*ty) && !s.reaches_run_post)
            {
                problems.push(format!(
                    "POST_PHASE_BYPASSES lists `{ty}`, which no longer overrides call_content \
                 without run_post; remove the entry"
                ));
            }
            if reason.len() <= 40 {
                problems.push(format!(
                    "POST_PHASE_BYPASSES entry `{ty}` must say why it bypasses: {reason:?}"
                ));
            }
        }
        problems
    }

    /// Gate 1, post half, over the real tree. The trait default must be seen and
    /// must reach `run_post` — that is the proof the scanner sees the call at all.
    #[test]
    fn every_call_content_override_reaches_run_post_or_is_declared() {
        let root = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
        let mut sites = Vec::new();
        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(path) else {
                continue;
            };
            if !src.contains("call_content") {
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            sites.extend(call_content_sites(&rel, &src));
        }
        let defaults: Vec<&CallContentSite> = sites.iter().filter(|s| s.owner.is_none()).collect();
        assert!(
            defaults.len() == 1 && defaults[0].reaches_run_post,
            "expected exactly one `trait Tool` default call_content, calling run_post; found {defaults:?}"
        );
        let problems = post_phase_violations(&sites, POST_PHASE_BYPASSES);
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    #[test]
    fn an_undeclared_override_that_skips_run_post_is_reported() {
        // A comment naming `run_post(` is not a call, so this still bypasses.
        let src = "impl Tool for Foo {\n    async fn call_content(&self) -> u8 {\n        // run_post(&ctx, &mut l) is skipped here\n        self.call()\n    }\n}\n";
        let sites = call_content_sites("foo.rs", src);
        assert_eq!(sites.len(), 1, "{sites:?}");
        let problems = post_phase_violations(&sites, &[]);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("Foo"), "{problems:?}");
    }

    #[test]
    fn an_override_that_calls_run_post_needs_no_declaration() {
        let src = "impl crate::tools::Tool for Bar {\n    async fn call_content(&self) -> u8 {\n        let e = crate::engines::coordinator::run_post(&p, &mut l);\n        e\n    }\n}\nimpl Other for Baz {\n    fn call_content(&self) {}\n}\n";
        let sites = call_content_sites("bar.rs", src);
        assert_eq!(sites.len(), 1, "only the `Tool` impl counts: {sites:?}");
        assert!(sites[0].reaches_run_post);
        assert!(post_phase_violations(&sites, &[]).is_empty());
    }

    #[test]
    fn a_declared_bypass_that_no_longer_bypasses_is_reported() {
        let reason = "a reason long enough to pass the placeholder check, honestly";
        let problems = post_phase_violations(&[], &[("Gone", reason)]);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("Gone"), "{problems:?}");
    }
}
