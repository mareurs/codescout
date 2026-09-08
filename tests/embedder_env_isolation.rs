//! Guards the one env-isolation rule this repo has already paid for in CI time.
//!
//! `docs/conventions/test-env-isolation.md` states the principle; this file pins the
//! single instance that broke `experiments` for 36 hours with no owner
//! (`docs/issues/2026-09-08-embedder-tests-read-an-ambient-env-var-ci-does-not-have.md`).
//!
//! **Why a source scan rather than a behavioural test.** The failure mode is a test that
//! reads `std::env` and therefore asserts about the developer's shell. You cannot catch
//! that with another test *in the same environment* — it passes there by construction,
//! which is exactly why every local gate run was green while CI was red. The only
//! observer that can see it is one reading the source, so that is what this is.
//!
//! **Deliberately narrow, and named for what it actually checks.** It pins two files and
//! one constructor, not "no test reads env". A guard whose name is wider than its trigger
//! is its own defect class here (`cluster/guard-narrower-than-its-name`), so the name says
//! `embedder`. Extend the LIST when a second constructor earns it.

use std::path::Path;

/// Files that must never construct the embedder through its env-reading path, with the
/// floor of `with_config` calls each currently holds. The floor is the non-vacuity
/// control — see the assertion below for why it is load-bearing rather than decoration.
const SCANNED: &[(&str, usize)] = &[
    ("src/retrieval/embedder.rs", 20),
    ("src/tools/semantic/semantic_search.rs", 1),
];

/// `EmbedderHttp::new` reads `CODESCOUT_EMBEDDER_MODEL_NAME`, `CODESCOUT_QUERY_PREFIX`,
/// `EMBED_API_KEY`, `CODESCOUT_EMBED_BATCH` and `CODESCOUT_EMBED_INFLIGHT` from the
/// process environment. Every shell on a developer machine supplies the first from a
/// gitignored `.env`; CI supplies none of them.
///
/// That divergence was latent and harmless for weeks. `9c03b32f` — a correct hardening
/// that refuses a blank model at the constructor — turned the empty default into a hard
/// failure, and from that commit the identical suite was green for every developer and
/// red for CI. Last green `ubuntu/default` was `c9e6cb6b`; the branch stayed red 36 hours
/// with every local gate run passing.
///
/// Neither the test nor the hardening was wrong. The tests never declared their
/// dependency, so tightening the contract reclassified an unstated precondition as a
/// failure in one environment only — there was no line a reviewer could object to. This
/// scan is the remedy that does not depend on anyone noticing.
///
/// **Use `EmbedderHttp::with_config(dense, sparse, dim, "m", "")`** — its own doc comment
/// already says tests must, and it takes the model name and query prefix explicitly.
#[test]
fn no_embedder_test_constructs_through_the_env_reading_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (rel, _) in SCANNED {
        let src = std::fs::read_to_string(root.join(rel))
            .unwrap_or_else(|e| panic!("{rel} must be readable to be guarded: {e}"));
        let hits = src.match_indices("EmbedderHttp::new(").count();
        assert_eq!(
            hits, 0,
            "{rel} calls EmbedderHttp::new(), which reads process env, so the test asserts \
             about the developer's shell and passes locally while failing in CI. Use \
             EmbedderHttp::with_config(dense, sparse, dim, \"m\", \"\") instead. \
             Background: docs/issues/2026-09-08-embedder-tests-read-an-ambient-env-var-ci-does-not-have.md"
        );
    }
}

/// Non-vacuity control for the scan above, and it is the half that makes it worth
/// anything.
///
/// `assert_eq!(hits, 0)` is monotone under removal: rename the type, move the tests to
/// another file, or mistype the needle, and the scan finds nothing and passes while
/// looking at the wrong thing — reporting health where it has simply gone blind. So assert
/// the scanner is standing in front of real content: each file must still hold at least
/// as many `with_config` calls as it did when the guard was written.
///
/// A failure here is not "someone broke the rule" — it is "the guard above stopped
/// being able to see", which needs the needle re-pointed rather than the code fixed.
#[test]
fn the_env_isolation_scan_is_not_vacuous() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (rel, floor) in SCANNED {
        let src = std::fs::read_to_string(root.join(rel))
            .unwrap_or_else(|e| panic!("{rel} must be readable to be guarded: {e}"));
        let found = src.match_indices("EmbedderHttp::with_config(").count();
        assert!(
            found >= *floor,
            "{rel} holds {found} EmbedderHttp::with_config( calls, below the floor of \
             {floor} recorded when this guard was written. The guard above is probably \
             scanning the wrong thing rather than the code being wrong — re-point the \
             needle, or lower the floor deliberately and say why."
        );
    }
}
