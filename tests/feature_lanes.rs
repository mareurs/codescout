//! Every feature declared in `Cargo.toml` must have a **decision**: a CI lane that
//! builds it, arrival via `default`, or an explicit exemption with a reason.
//!
//! Why this exists. `server-stack` sat in `[features]` with no lane for an unknown
//! length of time while being the configuration `cargo rb` actually ships (see
//! `.cargo/config.toml`). Every `#[cfg(feature = "server-stack")]` test was therefore
//! never compiled — indistinguishable from a test that does not exist, and worse than
//! an obvious hole, because the files *looked* covered. When the lane was finally run
//! it failed immediately, on a real defect. Full account:
//! `docs/issues/archive/2026-08-08-server-stack-gated-tests-never-compiled-by-any-lane.md`.
//!
//! The point is not that every feature must be tested. Some genuinely cannot be, in
//! CI. The point is that *forgetting* must not be silent: adding a feature without
//! either laning it or exempting it fails here, which forces the decision to be made
//! once rather than skipped forever.

use std::collections::BTreeSet;
use std::path::PathBuf;

/// Features intentionally not **run** in CI, each with the reason.
///
/// Adding an entry here is a real decision — it says "this cannot be *executed* on a CI
/// runner", not "I have not got round to it". Everything currently listed needs external
/// services or language servers installed on the machine; the reasons are Cargo.toml's
/// own, not invented here.
///
/// It does **not** exempt a feature from being *compiled*. `cargo check` needs none of
/// those services, and `every_exempt_feature_is_still_compiled_somewhere` enforces that
/// every entry here keeps a compile lane.
///
/// That distinction is the whole of F-62. This comment used to say an exemption meant
/// "cannot be built on a CI runner"; the five `e2e-*` entries were taken at their word,
/// nothing ever compiled them, and all five stopped compiling at once while `cargo test`
/// reported green — the same bit-rot this file exists to prevent, let in through the
/// escape hatch rather than around it.
const EXEMPT: &[(&str, &str)] = &[
    ("e2e", "meta-feature enabling the e2e-* family below"),
    ("e2e-rust", "needs rust-analyzer installed on the runner"),
    (
        "e2e-python",
        "needs pyright-langserver installed on the runner",
    ),
    (
        "e2e-typescript",
        "needs typescript-language-server installed on the runner",
    ),
    ("e2e-kotlin", "needs kotlin-lsp installed on the runner"),
    ("e2e-java", "needs jdtls installed on the runner"),
    (
        "retrieval-e2e",
        "needs a live Qdrant + TEI/Ollama stack (docker-compose.yml)",
    ),
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every feature named in a `--features <list>` flag anywhere under
/// `.github/workflows/`.
///
/// Deliberately scoped to `--features` rather than a substring search of the whole
/// file: "librarian" appears in a step *name* ("Seed empty librarian workspace"), so a
/// plain `contains` would mark it covered for a reason that has nothing to do with
/// building it. A guard that passes by accident is the failure mode this whole test
/// exists to prevent.
fn features_named_in_workflows() -> BTreeSet<String> {
    let dir = repo_root().join(".github/workflows");
    let mut found = BTreeSet::new();

    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yml") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let mut rest = text.as_str();
        while let Some(idx) = rest.find("--features") {
            rest = &rest[idx + "--features".len()..];
            // The next whitespace-delimited token is the (possibly comma-separated)
            // feature list.
            if let Some(tok) = rest.split_whitespace().next() {
                for f in tok.split(',') {
                    let f = f.trim().trim_matches('"');
                    if !f.is_empty() {
                        found.insert(f.to_string());
                    }
                }
            }
        }
    }
    found
}

/// Transitive closure over `[features]`, from an arbitrary starting set.
fn closure_from(
    features: &toml::Table,
    start: impl IntoIterator<Item = String>,
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack: Vec<String> = start.into_iter().collect();

    while let Some(name) = stack.pop() {
        if !out.insert(name.clone()) {
            continue;
        }
        if let Some(toml::Value::Array(members)) = features.get(&name) {
            for m in members {
                if let Some(s) = m.as_str() {
                    // `dep:foo` and `crate/feat` enable dependencies, not features of
                    // this crate — they can never appear as a `[features]` key.
                    if !s.starts_with("dep:") && !s.contains('/') {
                        stack.push(s.to_string());
                    }
                }
            }
        }
    }
    out
}

/// Everything CI actually **compiles**: the closure of every feature named in a
/// `--features` flag. A lane passing `--features e2e` builds `e2e-python` too, so the
/// raw token set understates coverage wherever a meta-feature is used.
fn compiled_in_ci(features: &toml::Table) -> BTreeSet<String> {
    closure_from(features, features_named_in_workflows())
}

/// `default` plus everything it pulls in, transitively. A feature reached this way is
/// built by any lane that does not pass `--no-default-features`, and the test matrix
/// has such a lane.
fn default_closure(features: &toml::Table) -> BTreeSet<String> {
    closure_from(features, ["default".to_string()])
}

fn declared_features() -> toml::Table {
    let manifest = std::fs::read_to_string(repo_root().join("Cargo.toml")).unwrap();
    let parsed: toml::Table = manifest.parse().expect("Cargo.toml parses as TOML");
    match parsed.get("features") {
        Some(toml::Value::Table(t)) => t.clone(),
        _ => panic!("Cargo.toml has no [features] table"),
    }
}

#[test]
fn every_declared_feature_has_a_lane_or_a_reason() {
    let features = declared_features();
    let via_default = default_closure(&features);
    let via_workflow = features_named_in_workflows();
    let exempt: BTreeSet<&str> = EXEMPT.iter().map(|(f, _)| *f).collect();

    let uncovered: Vec<&String> = features
        .keys()
        .filter(|f| {
            !via_default.contains(*f) && !via_workflow.contains(*f) && !exempt.contains(f.as_str())
        })
        .collect();

    assert!(
        uncovered.is_empty(),
        "these features are declared in Cargo.toml but no CI lane builds them, and they \
         are not exempt: {uncovered:?}\n\n\
         Pick one:\n  \
         - add a lane in .github/workflows/ci.yml (a `cargo check --features <name> \
         --all-targets` step is enough to catch bit-rot), or\n  \
         - add the feature to EXEMPT in tests/feature_lanes.rs with the reason it cannot \
         be built on a runner.\n\n\
         Silently leaving it unbuilt is the option this test removes: `server-stack` was \
         unbuilt while being the configuration `cargo rb` ships, and the first run of its \
         lane failed on a real defect."
    );
}

/// The guard above can only be trusted if its inputs are non-empty. A typo in the
/// workflow directory, an extension change, or a `[features]` rename would make it
/// pass by finding nothing to check — vacuous green, which is exactly the failure this
/// file was written to prevent, reproduced one level up.
#[test]
fn the_guard_is_not_vacuous() {
    let features = declared_features();
    assert!(
        features.len() > 5,
        "expected a populated [features] table, got {}",
        features.len()
    );

    let via_workflow = features_named_in_workflows();
    assert!(
        via_workflow.contains("server-stack"),
        "expected the server-stack lane to be discoverable via --features; \
         found only {via_workflow:?}"
    );

    let via_default = default_closure(&features);
    assert!(
        via_default.contains("librarian"),
        "expected `librarian` to be reached transitively through `default`; \
         closure was {via_default:?}"
    );
    // ...and specifically NOT because the string appears somewhere in a workflow.
    assert!(
        !via_workflow.contains("librarian"),
        "`librarian` was matched as a --features argument; the workflow scan has \
         widened and can now mark features covered by accident"
    );
}

/// An exemption says a feature cannot be **run** on a CI runner. It does not say the
/// feature cannot be **compiled** there, and those are different claims: every reason
/// in `EXEMPT` names a missing service or language server, which is a reason not to
/// execute the tests — `cargo check` needs none of them.
///
/// The project already learned this once. `retrieval-e2e` stays exempt from running
/// and is compiled by the `feature-check` lane anyway, after 4 call sites rotted under
/// a signature change with nothing to catch it. The lesson was written into that job's
/// comment and not applied to the five `e2e-*` lanes — which then stopped compiling
/// **entirely**: `ToolContext` grew a field, `tests/e2e/harness.rs` never got it, and
/// `cargo test` stayed green throughout, because nothing builds a feature-gated target
/// that no lane names. Found by accident on 2026-08-26, not by any gate.
/// `docs/trackers/bug-fix-session-log.md` F-62.
#[test]
fn every_exempt_feature_is_still_compiled_somewhere() {
    let features = declared_features();
    let compiled = compiled_in_ci(&features);

    let uncompiled: Vec<&str> = EXEMPT
        .iter()
        .map(|(f, _)| *f)
        .filter(|f| !compiled.contains(*f))
        .collect();

    assert!(
        uncompiled.is_empty(),
        "these features are exempt from RUNNING in CI, but nothing compiles them \
         either: {uncompiled:?}\n\n\
         A reason like \"needs <language server> on the runner\" justifies not \
         executing the tests. It does not justify never building them: a feature-gated \
         target that no lane names is indistinguishable from a target that does not \
         exist, and it rots silently while `cargo test` reports green.\n\n\
         Add a `cargo check --features <name> --all-targets` step to the \
         `feature-check` job in .github/workflows/ci.yml — naming a meta-feature \
         covers the whole family it enables."
    );
}
