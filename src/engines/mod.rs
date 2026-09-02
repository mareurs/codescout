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
//! Production has **six** ledger writers, not two:
//!
//! | site | key written | engine |
//! |---|---|---|
//! | `guide_emit::guide_blocks_for` (non-declaring) | `<topic>` | `guide-sections` |
//! | `guide_emit::guide_blocks_for` (fallback) | `<topic>#<preamble>` | `guide-sections` |
//! | `guide_emit::guide_blocks_for` (matched) | `<topic>#<heading>` | `guide-sections` |
//! | `tools::guide` (explicit fetch) | `<topic>` | `guide-sections`, **pull** |
//! | `core::types::call_content` (opener) | `project-activation-bootstrap` | `session-opener` |
//! | `core::types::call_content` (rules) | `op:OP-N` | `operator-rules` |
//!
//! `session-opener` writes a **bare topic name**, which `guide-sections` also
//! owns. That overlap is deliberate and documented at the site: keying the
//! opener finer "would desync this trigger from what `GuideLedger::re_arm`
//! actually re-arms". A global prefix-disjointness gate would fail on a
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
//! It does not yet *drive* delivery in production — `call_content` still
//! fans out by hand, running its own inlined copy of each engine's logic
//! rather than calling through `emit_post`. Making it call through is
//! `docs/superpowers/plans/2026-09-02-layer-2a-3-wiring-and-one-budget.md`
//! (Plan 3). Three of the four rows below now carry a wired `emit_post` —
//! `run_post_in` can call through it, and the tests do — but that is
//! **necessary, not sufficient**: `run_post`, the only thing that would call
//! `run_post_in` against this live registry, itself has no caller yet, so
//! none of it runs in production until Plan 3 lands. [`Mode::Unmanaged`] is
//! the honest state for an engine that ships and participates in nothing,
//! which is where `craft-skills` sits today — the other three now
//! participate in the registry, but not yet in production.

pub mod coordinator;
pub mod emitters;

use crate::prompts::SESSION_OPENING_GUIDE;

/// The key an engine retrieves on. This is the family's discriminator: it
/// determines the corpus, the retrieval mechanism, and what the ledger must
/// remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalKey {
    /// The shape of the call being made — `artifact.update`, `edit_file`.
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
    pub writes_at: &'static [&'static str],
    /// Whether a ledger key belongs to this engine.
    ///
    /// A function pointer rather than a prefix string because two of the three
    /// ledger-writing engines do not use a prefix at all — see the module docs.
    pub owns_key: fn(&str) -> bool,
    /// This engine's post-phase emitter, or `None` while it is still inlined
    /// in `call_content`. A function pointer rather than a trait object
    /// because an engine is data, not behaviour with state — and because a
    /// `&'static EngineDecl` must stay `Sync` without a `Box`.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "read by engines::coordinator::run_post_in, which Plan 3 of \
                      the Layer 2a sequence wires into call_content; tests call \
                      run_post_in directly today, so it is live under cfg(test)"
        )
    )]
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
/// (`<topic>#<heading>`, `<topic>#<preamble>`).
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
        writes_at: &["tools::core::types"],
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
        writes_at: &["tools::core::types"],
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
    /// `route::ledger_key` is what `call_content`'s rule branch writes.
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
    /// of librarian guide (`types.rs:968`). Both draw `Corpus::CompiledGuides`,
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
}
