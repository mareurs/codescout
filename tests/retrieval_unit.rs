use codescout::retrieval::config::RetrievalConfig;

/// Run `f` with the whole retrieval env surface neutralised and `overrides` layered on
/// top.
///
/// **The embedding names are DERIVED from `config::embedding_env::all_names()`, never
/// re-typed.** A hand-written list stops covering the resolver the moment a name is
/// added, and that is not hypothetical: the deprecated-alias chain introduced on
/// 2026-09-17 widened what `EmbedEnv::from_real_env` reads, and every test in this file
/// that named its own variables silently began reading an ambient alias instead of the
/// value it had just set. `all_names`' own doc comment records that for the one test
/// that was repaired then; this helper is what stops the ninth name un-isolating the
/// others. A list kept in step by hand is the same shape as the scattered `env::var`
/// calls `config::embedding_env` exists to replace.
///
/// `temp_env::with_vars` documents "if the variable with the same name is set multiple
/// times, the last one wins", so `overrides` reliably beats the `None` entries it
/// follows — that ordering is what lets one helper both clear and set.
///
/// `XDG_CONFIG_HOME` is pointed at an **empty directory** rather than unset: unset falls
/// back to `$HOME/.config/codescout/config.toml`, the global BASE layer that
/// `ProjectConfig::load_or_default` merges under every project.toml, which is exactly
/// the machine-local file these assertions must not read.
///
/// `temp_env::with_vars` also serializes on a global `ReentrantMutex`, which is what
/// makes these tests safe against each other — `#[serial_test::serial]` would not, since
/// it only orders tests within one binary's scheduler and a full `cargo test` run has
/// other binaries writing `CODESCOUT_*` concurrently
/// (BUG: 2026-05-24-ci-retrieval-env-test-cross-binary-flake).
fn with_isolated_retrieval_env<R>(overrides: &[(&str, Option<&str>)], f: impl FnOnce() -> R) -> R {
    let empty_config_home = tempfile::tempdir().expect("tempdir for an empty global config dir");
    let mut vars: Vec<(String, Option<String>)> = vec![(
        "XDG_CONFIG_HOME".to_string(),
        Some(
            empty_config_home
                .path()
                .to_str()
                .expect("tempdir path is utf-8")
                .to_string(),
        ),
    )];
    for name in [
        // Read directly by `RetrievalConfig::from_env_and_project`; outside the
        // embedding family `all_names()` declares.
        "CODESCOUT_QDRANT_URL",
        "CODESCOUT_SPARSE_EMBEDDER_URL",
        "CODESCOUT_RERANKER_URL",
        "CODESCOUT_RETRIEVAL_PROFILE",
        // Not part of the embedding family either: a separate, deprecated override for
        // the model name put on the wire.
        "CODESCOUT_EMBEDDER_MODEL_NAME",
    ] {
        vars.push((name.to_string(), None));
    }
    for name in codescout::config::embedding_env::all_names() {
        vars.push((name.to_string(), None));
    }
    for (key, value) in overrides {
        vars.push(((*key).to_string(), value.map(|v| v.to_string())));
    }
    temp_env::with_vars(vars, f)
}

#[test]
fn config_from_env_uses_defaults_when_unset() {
    // Isolation rationale — derived names, the global-config base layer, and why
    // `temp_env` rather than `#[serial_test::serial]` — lives on
    // `with_isolated_retrieval_env`. Nothing is overridden here: "unset" IS the subject.
    with_isolated_retrieval_env(&[], || {
        let cfg = RetrievalConfig::from_env().expect("defaults");
        assert_eq!(cfg.qdrant_url, "http://127.0.0.1:6334");
        assert_eq!(
            cfg.embedder_url, None,
            "an unset url must mean 'resolve from the model', not 'assume 8081'"
        );
        // 48084/48083 are the HOST ports docker-compose.yml publishes. These
        // read as literals because the constants behind them are pub(crate) and
        // this is an integration test; the in-crate guard
        // `config::default_port_tests::retrieval_default_ports_match_published_compose_ports`
        // is what proves they still match the compose file. Until 2026-08-14
        // these asserted 8084/8083 — container-internal ports nothing listens on
        // from the host — so this test pinned the bug in place rather than
        // catching it.
        assert_eq!(cfg.sparse_embedder_url, "http://127.0.0.1:48084");
        assert_eq!(cfg.reranker_url, "http://127.0.0.1:48083");
        assert_eq!(
            cfg.model_dim, None,
            "an unpinned dim must let the model decide"
        );
        assert_eq!(cfg.model, "local:AllMiniLML6V2Q");
        assert_eq!(cfg.api_key, None);
        assert_eq!(cfg.profile, "cpu");
    });
}

#[test]
fn config_from_env_reads_overrides() {
    // Every name set here is a DEPRECATED alias, deliberately: it is the arm that
    // reaches the resolver through `embedding_env::pick`'s fallback chain rather than
    // its canonical branch, so "modernising" these to `CODESCOUT_EMBEDDING_*` would
    // silently drop the only end-to-end coverage the aliases have.
    //
    // `with_isolated_retrieval_env` clears the canonical names first — without that,
    // an ambient `CODESCOUT_EMBEDDING_URL` outranks the alias this test sets and the
    // assertion compares the machine's embedder to the fixture's.
    with_isolated_retrieval_env(
        &[
            ("CODESCOUT_QDRANT_URL", Some("http://qd:1")),
            ("CODESCOUT_EMBEDDER_URL", Some("http://eb:2")),
            ("CODESCOUT_EMBEDDER_MODEL", Some("local:BGESmallENV15")),
            ("CODESCOUT_SPARSE_EMBEDDER_URL", Some("http://eb-sparse:5")),
            ("CODESCOUT_RERANKER_URL", Some("http://rr:3")),
            ("CODESCOUT_MODEL_DIM", Some("4096")),
            ("CODESCOUT_RETRIEVAL_PROFILE", Some("gpu")),
            ("EMBED_API_KEY", Some("sk-test")),
        ],
        || {
            let cfg = RetrievalConfig::from_env().expect("overrides");
            assert_eq!(cfg.qdrant_url, "http://qd:1");
            assert_eq!(cfg.embedder_url.as_deref(), Some("http://eb:2"));
            assert_eq!(cfg.sparse_embedder_url, "http://eb-sparse:5");
            assert_eq!(cfg.model_dim, Some(4096));
            assert_eq!(cfg.model, "local:BGESmallENV15");
            assert_eq!(cfg.api_key.as_deref(), Some("sk-test"));
            assert_eq!(cfg.profile, "gpu");
        },
    );
}

#[test]
fn config_from_env_and_project_prefers_project_toml_when_env_silent() {
    // Composition-level regression for review round-1 mutation α: the
    // root -> ProjectConfig::load_or_default -> merge chain inside
    // `RetrievalConfig::from_env_and_project` must actually reach the
    // project's [embeddings] values, not just each merge field in isolation.
    //
    // Needs the WHOLE embedding surface neutralized, not one family: two
    // independently-named variable families reach one effective setting at two
    // different layers — `CODESCOUT_EMBEDDER_*` in `merge_embed_config`, and
    // `CODESCOUT_EMBED_*`/`CODESCOUT_EMBEDDING_*` inside
    // `ProjectConfig::load_or_default`, applied unconditionally before
    // `RetrievalConfig` ever sees the result. `with_isolated_retrieval_env`
    // clears all of them from one declaration; listing them here by hand is what
    // let the canonical `CODESCOUT_EMBEDDING_URL` leak past this test.
    with_isolated_retrieval_env(&[], || {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join(".codescout/project.toml"),
            "[project]\nname = \"proj\"\n\n[embeddings]\nmodel = \"local-dir:/weights\"\nurl = \"http://from-toml:9\"\n",
        )
        .unwrap();
        let cfg = RetrievalConfig::from_env_and_project(Some(dir.path())).expect("loads");
        assert_eq!(cfg.model, "local-dir:/weights");
        assert_eq!(cfg.embedder_url.as_deref(), Some("http://from-toml:9"));
    });
}

#[test]
fn config_from_env_and_project_env_wins_over_project_toml() {
    // Same composition seam as above, but with the env side populated too — a
    // precedence inversion that only manifests once the real `load_or_default`
    // call is wired in (rather than in a pure merge-function-level test alone)
    // would slip past `merge_tests` in src/retrieval/config.rs but not this one.
    //
    // The two names set here are DEPRECATED aliases on purpose (see
    // `config_from_env_reads_overrides`); the canonical ones are cleared by
    // `with_isolated_retrieval_env`, so an ambient `CODESCOUT_EMBEDDING_URL`
    // cannot win the comparison this test is about.
    with_isolated_retrieval_env(
        &[
            ("CODESCOUT_EMBEDDER_URL", Some("http://from-env:8")),
            ("CODESCOUT_EMBEDDER_MODEL", Some("local:BGESmallENV15")),
        ],
        || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
            std::fs::write(
                dir.path().join(".codescout/project.toml"),
                "[project]\nname = \"proj\"\n\n[embeddings]\nmodel = \"local-dir:/weights\"\nurl = \"http://from-toml:9\"\n",
            )
            .unwrap();
            let cfg = RetrievalConfig::from_env_and_project(Some(dir.path())).expect("loads");
            assert_eq!(cfg.model, "local:BGESmallENV15");
            assert_eq!(cfg.embedder_url.as_deref(), Some("http://from-env:8"));
        },
    );
}
/// A `project.toml` that cannot be loaded must REFUSE, not resolve the built-in default
/// embedding model behind the caller's back.
///
/// Regression for `980b98ef4dc66eac`
/// (`docs/issues/2026-09-18-a-malformed-project-toml-silently-resolves-the-default-embedding-model.md`):
/// `resolve_embed_fields_with` discarded the load error with `.ok()`, so a project whose
/// config fails to parse was indistinguishable from one that never configured a model —
/// it got `local:AllMiniLML6V2Q` and no error. Same shape as the archived
/// `f73130523241a666`, one subsystem over.
///
/// **The fixture's malformed shape is load-bearing:** an `[embeddings]` table with no
/// `[project]` table, which `ProjectSection.name` requires — the bug file's own
/// reproduction. Any other load failure is fine to substitute; substituting VALID TOML
/// makes the test vacuous, since `expect_err` is then asserting about a config that
/// loaded.
#[test]
fn a_malformed_project_toml_refuses_instead_of_defaulting_the_embedding_model() {
    with_isolated_retrieval_env(&[], || {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let project_toml = dir.path().join(".codescout").join("project.toml");
        std::fs::write(
            &project_toml,
            "[embeddings]\nmodel = \"local:JinaEmbeddingsV2BaseCode\"\n",
        )
        .unwrap();

        let err = match RetrievalConfig::from_env_and_project(Some(dir.path())) {
            // `RetrievalConfig` has no `Debug` derive on purpose (`api_key` is plaintext),
            // so this cannot be `expect_err`. Naming the resolved model in the panic is
            // the better message anyway: the silent substitution IS the defect.
            Ok(cfg) => panic!(
                "a project.toml that cannot be loaded must not resolve silently — \
                 got model {:?}",
                cfg.model
            ),
            Err(e) => e,
        };

        let rendered = err.to_string();
        assert!(
            rendered.contains(project_toml.to_str().unwrap()),
            "the refusal must name the file the reader has to fix, got: {rendered}"
        );

        // The recoverable arm, not the fatal one: a user's config file is input, so a
        // sibling parallel tool call in the same turn must survive it
        // (`get_guide(\"error-handling\")`). Asserting the TYPE rather than the wording —
        // this reds on a switch back to a bare `anyhow::bail!`, and survives rewording.
        let recoverable = err
            .downcast_ref::<codescout::tools::RecoverableError>()
            .expect("a malformed user config is input-driven -> RecoverableError");
        // Shape, not prose: the guidance must still be THERE. A predicate-only suite
        // leaves the remedy half untested by construction, and this reds exactly on its
        // deletion without pinning a sentence that will be reworded.
        assert!(
            recoverable.hint().is_some_and(|h| !h.trim().is_empty()),
            "a refusal with no remedy tells the reader what is wrong and not what to do"
        );
    });
}

/// The distinction the fix above must NOT break: an **absent** `project.toml` is the
/// common case, not a failure. It still falls back to the built-in default silently.
///
/// This is the other direction of the same `and_then` — `.ok()` collapsed "absent" and
/// "malformed" into one `None`, and a fix that propagates every `Err` would be correct
/// here only because `load_or_default` synthesises a default table for a missing file
/// rather than erroring. That is the property this pins.
#[test]
fn an_absent_project_toml_still_resolves_the_built_in_default() {
    with_isolated_retrieval_env(&[], || {
        // Deliberately no `.codescout/` directory at all.
        let dir = tempfile::tempdir().unwrap();

        let cfg = RetrievalConfig::from_env_and_project(Some(dir.path()))
            .expect("an absent project.toml is not an error — it is the common case");

        assert_eq!(
            cfg.model,
            codescout::config::project::default_embed_model(),
            "no project.toml means the built-in default, resolved without complaint"
        );
        assert_eq!(cfg.embedder_url, None);
    });
}
/// The guard the four tests above cannot be: that `with_isolated_retrieval_env` really
/// clears **every declared** embedding name.
///
/// Each test above reads the resolved CONFIG, so a name missing from the helper only
/// surfaces on a machine that happens to export it — which is why the 2026-09-17 alias
/// chain went unnoticed until someone's shell had `CODESCOUT_EMBED_URL` in it, and why
/// CI stayed green throughout. This reads the ENVIRONMENT instead, and poisons every
/// declared name with a sentinel first, so it reds on the missed name **everywhere**,
/// bare CI included.
///
/// That sentinel pass is load-bearing, not belt-and-braces: without it the loop below
/// asserts "unset" against an environment where the names were never set, which is true
/// however the helper behaves — an assertion that cannot fail, in exactly the direction
/// this bug came from.
#[test]
fn the_isolation_helper_clears_every_declared_embedding_name() {
    let declared = codescout::config::embedding_env::all_names();
    assert!(
        !declared.is_empty(),
        "a guard that iterates an empty list passes by finding nothing — \
         embedding_env::all_names() must declare the surface it is named for"
    );

    let poisoned: Vec<(&str, Option<&str>)> = declared
        .iter()
        .map(|name| (*name, Some("SENTINEL-ambient-value")))
        .collect();

    temp_env::with_vars(poisoned, || {
        // Nesting is safe: `temp_env` guards on a ReentrantMutex, and the inner call
        // captures and restores only the names it touches.
        with_isolated_retrieval_env(&[], || {
            for name in codescout::config::embedding_env::all_names() {
                assert!(
                    std::env::var(name).is_err(),
                    "{name} survived the isolation helper. An ambient value of it \
                     outranks whatever a test sets — silently, and only on machines \
                     that export it. Derive the helper's list from all_names(); do not \
                     re-type it."
                );
            }
        });
    });
}

/// Constructing a client from env must go through `temp_env` like every other test in
/// this binary — a bare `set_var` here is not a style nit, it is a live flake.
///
/// `temp_env::with_vars` serializes on a global `ReentrantMutex`, so the guarded tests
/// never see each other's values. A bare `set_var` does not take that lock: it mutates
/// the process env of every test running in parallel in this binary, and the trailing
/// `remove_var` cleanup then unsets the variable underneath them. Measured 2026-08-26 on
/// the `server-stack` lane — this test's own `http://127.0.0.1:8081` surfaced inside
/// `config_from_env_and_project_env_wins_over_project_toml`, which had asked for
/// `http://from-env:8`. A lock only protects its participants.
///
/// It shows up on `server-stack` alone because that is the only lane where the `cfg`
/// below compiles this test into the binary at all.
#[cfg(feature = "server-stack")]
#[test]
fn client_from_env_constructs_when_urls_present() {
    with_isolated_retrieval_env(
        &[
            ("CODESCOUT_QDRANT_URL", Some("http://127.0.0.1:6334")),
            ("CODESCOUT_EMBEDDER_URL", Some("http://127.0.0.1:8081")),
            (
                "CODESCOUT_SPARSE_EMBEDDER_URL",
                Some("http://127.0.0.1:8084"),
            ),
            ("CODESCOUT_RERANKER_URL", Some("http://127.0.0.1:8083")),
        ],
        || {
            let cfg = codescout::retrieval::config::RetrievalConfig::from_env().unwrap();
            // doesn't connect — just constructs
            let _ = codescout::retrieval::client::RetrievalClient::from_config_only(cfg);
        },
    );
}

use codescout::retrieval::drift::{diff_chunks, ChunkRef};

fn cr(id: &str, hash: &str) -> ChunkRef {
    ChunkRef {
        chunk_id: id.into(),
        content_hash: hash.into(),
        // diff_chunks only reads chunk_id; file_path is irrelevant to these tests.
        file_path: String::new(),
    }
}

#[test]
fn diff_identical_yields_noop() {
    let server = vec![cr("a", "h1"), cr("b", "h2")];
    let local = vec![cr("a", "h1"), cr("b", "h2")];
    let d = diff_chunks(&server, &local);
    assert!(d.to_upsert.is_empty());
    assert!(d.to_delete.is_empty());
}

#[test]
fn diff_added_chunk_yields_upsert() {
    let server = vec![cr("a", "h1")];
    let local = vec![cr("a", "h1"), cr("b", "h2")];
    let d = diff_chunks(&server, &local);
    assert_eq!(d.to_upsert, vec!["b".to_string()]);
    assert!(d.to_delete.is_empty());
}

#[test]
fn diff_deleted_chunk_yields_delete() {
    let server = vec![cr("a", "h1"), cr("b", "h2")];
    let local = vec![cr("a", "h1")];
    let d = diff_chunks(&server, &local);
    assert!(d.to_upsert.is_empty());
    assert_eq!(d.to_delete, vec!["b".to_string()]);
}

#[test]
fn diff_modified_chunk_yields_upsert_for_new_id() {
    let server = vec![cr("a-old", "h1")];
    let local = vec![cr("a-new", "h2")];
    let d = diff_chunks(&server, &local);
    assert_eq!(d.to_upsert, vec!["a-new".to_string()]);
    assert_eq!(d.to_delete, vec!["a-old".to_string()]);
}

#[cfg(feature = "server-stack")]
use codescout::retrieval::payload::{map_to_payload, payload_to_map, CodePayload};

#[cfg(feature = "server-stack")]
#[test]
fn payload_roundtrip_preserves_fields() {
    let p = CodePayload {
        project_id: "codescout".into(),
        file_path: "src/lib.rs".into(),
        language: "rust".into(),
        start_line: 10,
        end_line: 42,
        ast_header: "src/lib.rs :: fn main()".into(),
        content: "fn main() {}".into(),
        content_hash: "h1".into(),
        last_indexed_commit: "abc".into(),
        chunk_id: "id1".into(),
    };
    let map = payload_to_map(&p);
    let back = map_to_payload(&map).expect("decode");

    // Every field, not a sample. This test asserted 4 of 11 while its name claimed
    // all of them, so `ast_header` could round-trip as garbage — or not at all —
    // without failing anything.
    assert_eq!(back.project_id, p.project_id);
    assert_eq!(back.file_path, p.file_path);
    assert_eq!(back.language, p.language);
    assert_eq!(back.start_line, p.start_line);
    assert_eq!(back.end_line, p.end_line);
    assert_eq!(back.ast_header, p.ast_header);
    assert_eq!(back.content, p.content);
    assert_eq!(back.content_hash, p.content_hash);
    assert_eq!(back.last_indexed_commit, p.last_indexed_commit);
    assert_eq!(back.chunk_id, p.chunk_id);
}

/// `embed_text` is the single home for "what does a chunk look like to the embedder".
/// Before it existed, that decision was the residue of a struct literal in another
/// module, which is how the AST header stopped being embedded without a failing test.
///
/// Deliberately NOT gated behind `server-stack`. That feature is in neither `default`
/// nor any CI workflow, so every test carrying the gate — including
/// `payload_roundtrip_preserves_fields` right above — is never compiled and cannot
/// fail. `embed_text` and `CodePayload` are both ungated, so this one runs.
#[test]
fn embed_text_prepends_the_ast_header_and_omits_it_when_absent() {
    use codescout::retrieval::payload::{embed_text, CodePayload};

    let mut p = CodePayload {
        project_id: "codescout".into(),
        file_path: "src/lib.rs".into(),
        language: "rust".into(),
        start_line: 1,
        end_line: 1,
        ast_header: "src/lib.rs :: fn main()".into(),
        content: "fn main() {}".into(),
        content_hash: "h1".into(),
        last_indexed_commit: "abc".into(),
        chunk_id: "id1".into(),
    };

    assert_eq!(
        embed_text(&p),
        "src/lib.rs :: fn main()\nfn main() {}",
        "the header is a newline-separated prefix, and the body is preserved verbatim"
    );

    // Markdown and plain text have no AST header; they must embed as bare content
    // rather than as a leading blank line.
    p.ast_header = String::new();
    assert_eq!(embed_text(&p), "fn main() {}");
}
