//! Standing guard: a type that `impl Tool` but that no agent can reach.
//!
//! `ListFunctions`, `ListDocs` and `GetUsageStats` each implemented `Tool` — name,
//! description, JSON schema, `call` — and were registered nowhere. Between them they
//! carried 21 passing tests. Nothing was visibly broken, because every one of those
//! tests constructed the tool directly: `ListFunctions.call(json!({…}), &ctx)` works
//! perfectly, and it is the *registration* that was absent. No test that builds a tool
//! itself can observe that, and `dead_code` is exempt for `pub` items in a lib crate,
//! so the compiler could not see it either.
//!
//! This is `issue-clusters:IC-3` (`cluster/declared-not-wired`), promoted to `OB-7`.
//! The class is only closed by a check that runs when nobody is worried, which is what
//! this file is.
//!
//! Patterned on `tests/issue_clusters.rs`: walk the corpus, parse, assert a repo-wide
//! property, and carry self-tests proving the scanner is not vacuous.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Types reached by an explicit named delegation from a consolidated tool, rather than
/// by sitting in the server's registry.
///
/// These are the consolidated-tool pattern working correctly: `Workspace` dispatches
/// `"activate" => ActivateProject.call(…)`, `Index` dispatches `"verify" =>
/// IndexVerify.call(…)`. They are genuinely reachable, so a guard keyed on registry
/// membership alone would fire on all seven on day one, acquire an allowlist by
/// reflex, and become the check everyone silences.
///
/// `no_stale_tolerance_entries` keeps this list honest in both directions.
const KNOWN_DELEGATION_ONLY: &[&str] = &[
    "ActivateProject",
    "ProjectStatus",
    "ListLibraries",
    "RegisterLibrary",
    "IndexProject",
    "IndexStatus",
    "IndexVerify",
];

/// Every `.rs` file under `src/`, as `(path, contents)`.
///
/// Deliberately reads FILES rather than compiled items, and scans both sides of the
/// diff the same way. That is what makes the guard feature-independent: `src/librarian/**`
/// is behind `#[cfg(feature = "librarian")]`, so a scan that collected declarations from
/// that subtree but not constructions would report every librarian tool unreachable under
/// `cargo test --no-default-features` — gate command three, where it would look like a
/// real regression.
fn rust_sources() -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    collect_rs(&repo_root().join("src"), &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn collect_rs(dir: &std::path::Path, out: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("tests") {
                continue;
            }
            collect_rs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            // Test-fixture modules declare `Tool` impls that exist only to be driven by
            // their own tests (`EchoTool`, `AlwaysTool`, …). Excluding them from BOTH
            // sides is deliberate: a production tool constructed only from test code is
            // exactly the defect this file exists to catch, so test files must not be
            // able to confer reachability either.
            if path.file_name().and_then(|n| n.to_str()) == Some("tests.rs") {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                out.push((path, strip_cfg_test_modules(&text)));
            }
        }
    }
}

/// Remove every `#[cfg(test)]`-attributed block, by brace matching from the attribute.
///
/// `collect_rs` excludes `tests/` directories and `tests.rs` files, and its comment states
/// the principle those two carriers implement: *"test files must not be able to confer
/// reachability either."* The third carrier is the one this repo uses most — an inline
/// `#[cfg(test)] mod tests { … }` at the foot of a production file — and it was not covered.
///
/// Measured 2026-09-01: `src/mcp_resources/tool_guide.rs` constructs `Arc::new(EditMarkdown)`
/// and `Arc::new(SymbolAt)` inside exactly such a module, putting both types into the
/// reachable set on the strength of test code. Both were genuinely registered on that date,
/// so nothing was misreported — but the path is live, and it is the half of the scan that had
/// no discipline on it. `reachable_tool_types` guards its `.call(` side with the
/// load-bearing `=>`; its `Arc::new(` side had no counterpart at all.
///
/// **`EditMarkdown` no longer exists** — Task 8 of the tool-surface collapse folded it into
/// `edit_file`, and that call site now constructs `EditFile`. The measurement is kept in its
/// 2026-09-01 tense rather than rewritten, because it is evidence about the SCAN, not about
/// either type: the hazard it demonstrates is that a `#[cfg(test)]` block can confer
/// reachability, and that is unchanged by which tool happens to sit in the example.
///
/// **Both error directions are acceptable here and one is loud**, which is why brace
/// matching suffices without a Rust parser. Over-deleting (a `{` inside a string or comment
/// ending the span early) drops production code, so a real tool loses its registration and
/// `every_impl_tool_type_is_reachable` fails *by name*. Under-deleting leaves today's
/// behaviour. There is no silently-wrong direction.
fn strip_cfg_test_modules(text: &str) -> String {
    const ATTR: &str = "#[cfg(test)]";
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while let Some(rel) = text[i..].find(ATTR) {
        let at = i + rel;
        let Some(open_rel) = text[at..].find('{') else {
            break;
        };
        let open = at + open_rel;

        let mut depth = 0usize;
        let mut end = None;
        for (j, b) in bytes.iter().enumerate().skip(open) {
            match b {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(j + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else { break };

        out.push_str(&text[i..at]);
        i = end;
    }

    out.push_str(&text[i..]);
    out
}

/// The last `::`-separated segment that starts with an uppercase letter.
///
/// `crate::tools::guide::GetGuide::new` -> `GetGuide`, `artifact::Artifact` -> `Artifact`,
/// `ReadFile` -> `ReadFile`. Taking the plain last segment would yield `new`.
fn type_name(path_expr: &str) -> Option<String> {
    path_expr
        .split("::")
        .filter(|seg| seg.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
        .last()
        .map(str::to_owned)
}

fn read_ident_path_forward(bytes: &[u8], mut i: usize) -> String {
    let start = i;
    while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b':')
    {
        i += 1;
    }
    String::from_utf8_lossy(&bytes[start..i]).into_owned()
}

fn read_ident_path_backward(bytes: &[u8], end: usize) -> String {
    let mut i = end;
    while i > 0
        && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_' || bytes[i - 1] == b':')
    {
        i -= 1;
    }
    String::from_utf8_lossy(&bytes[i..end]).into_owned()
}

/// Every type with an `impl … Tool for X` block, mapped to the file declaring it.
///
/// The trait path is matched on its LAST segment, so `impl Tool for X`,
/// `impl crate::tools::Tool for X` and `impl super::Tool for X` are all found.
/// That tolerance is load-bearing rather than defensive: `IndexVerify` is declared
/// `impl crate::tools::Tool for IndexVerify`, and a scan for the bare spelling misses
/// it. `the_scan_finds_qualified_impls` fails the moment someone simplifies this back.
fn declared_tool_types(sources: &[(PathBuf, String)]) -> BTreeMap<String, PathBuf> {
    let mut out = BTreeMap::new();
    for (path, text) in sources {
        for line in text.lines() {
            let Some(rest) = line.trim_start().strip_prefix("impl ") else {
                continue;
            };
            let Some((trait_part, type_part)) = rest.split_once(" for ") else {
                continue;
            };
            // Last whitespace-separated token handles `impl<T> Tool for X`.
            let Some(trait_tok) = trait_part.split_whitespace().next_back() else {
                continue;
            };
            if trait_tok.rsplit("::").next() != Some("Tool") {
                continue;
            }
            let name = read_ident_path_forward(type_part.as_bytes(), 0);
            if let Some(name) = type_name(&name) {
                out.entry(name).or_insert_with(|| path.clone());
            }
        }
    }
    out
}

/// Every type an agent can actually reach: constructed into a registry via `Arc::new(X)`,
/// or dispatched to by name from a match arm via `=> X.call(`.
///
/// **The `=>` is load-bearing, and without it this whole file is decoration.** The tools
/// that motivated this guard were each exercised by their own tests as
/// `ListFunctions.call(json!({…}), &ctx)` — textually identical to a delegation except for
/// the arrow. A scan that accepted any `X.call(` would have marked all three reachable and
/// passed green on the exact corpus that contained the defect. `the_scan_discriminates`
/// pins that case.
fn reachable_tool_types(sources: &[(PathBuf, String)]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (_, text) in sources {
        let bytes = text.as_bytes();

        let mut from = 0;
        while let Some(rel) = text[from..].find("Arc::new(") {
            let at = from + rel + "Arc::new(".len();
            if let Some(n) = type_name(&read_ident_path_forward(bytes, at)) {
                out.insert(n);
            }
            from = at;
        }

        let mut from = 0;
        while let Some(rel) = text[from..].find(".call(") {
            let at = from + rel;
            let path_str = read_ident_path_backward(bytes, at);
            if !path_str.is_empty() && text[..at - path_str.len()].trim_end().ends_with("=>") {
                if let Some(n) = type_name(&path_str) {
                    out.insert(n);
                }
            }
            from = at + 1;
        }
    }
    out
}

/// Types that declare `Tool` but that nothing constructs or dispatches to.
fn unreachable(sources: &[(PathBuf, String)]) -> Vec<(String, PathBuf)> {
    let declared = declared_tool_types(sources);
    let reachable = reachable_tool_types(sources);
    declared
        .into_iter()
        .filter(|(name, _)| !reachable.contains(name))
        .filter(|(name, _)| !KNOWN_DELEGATION_ONLY.contains(&name.as_str()))
        .collect()
}

#[test]
fn every_impl_tool_type_is_reachable() {
    let sources = rust_sources();
    let orphans = unreachable(&sources);
    assert!(
        orphans.is_empty(),
        "these types implement `Tool` but no agent can reach them — they are neither \
         constructed into a registry nor dispatched to by name.\n{}\n\n\
         Either register the type, delegate to it from a consolidated tool, or delete it. \
         Do NOT add it to KNOWN_DELEGATION_ONLY unless a `<Type>.call(` dispatch arm \
         genuinely exists — `no_stale_tolerance_entries` checks that.",
        orphans
            .iter()
            .map(|(n, p)| format!("  {n}  ({})", p.display()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_scan_finds_qualified_impls() {
    let declared = declared_tool_types(&rust_sources());
    assert!(
        declared.contains_key("IndexVerify"),
        "IndexVerify is declared `impl crate::tools::Tool for IndexVerify` — a qualified \
         trait path. If this fails, the scan has been narrowed to the bare `impl Tool for` \
         spelling and is now blind to every qualified impl, which is exactly how an \
         unreachable tool goes unnoticed."
    );
}

#[test]
fn the_scan_actually_reads_files() {
    let sources = rust_sources();
    assert!(
        sources.len() > 100,
        "expected to walk the whole src/ tree, saw {} files",
        sources.len()
    );
    let declared = declared_tool_types(&sources);
    assert!(
        declared.len() > 20,
        "expected a substantial set of `impl Tool` types, found {} — a scan that silently \
         matches nothing passes `every_impl_tool_type_is_reachable` vacuously",
        declared.len()
    );
}

#[test]
fn the_scan_discriminates() {
    // Three shapes in one synthetic corpus: registered, delegated, and the shape that
    // actually shipped — a tool with passing tests and no caller.
    let corpus = vec![(
        PathBuf::from("synthetic.rs"),
        r#"
            impl Tool for RegisteredThing {}
            impl crate::tools::Tool for DelegatedThing {}
            impl Tool for OrphanThing {}

            fn build() { let v = vec![Arc::new(RegisteredThing)]; }

            async fn dispatch(v: &str) {
                match v { "go" => DelegatedThing.call(input, ctx).await, _ => todo!() }
            }

            // The historical shape, verbatim in form: exercised only by its own test.
            // `OrphanThing.call(` differs from the delegation above by one arrow.
            #[tokio::test]
            async fn orphan_works() {
                let r = OrphanThing.call(json!({}), &ctx).await.unwrap();
                assert_eq!(r["total"], 2);
            }
        "#
        .to_string(),
    )];

    let declared = declared_tool_types(&corpus);
    for n in ["RegisteredThing", "DelegatedThing", "OrphanThing"] {
        assert!(declared.contains_key(n), "{n} should be declared");
    }

    let reachable = reachable_tool_types(&corpus);
    assert!(
        reachable.contains("RegisteredThing"),
        "Arc::new registration"
    );
    assert!(reachable.contains("DelegatedThing"), "=> delegation arm");
    assert!(
        !reachable.contains("OrphanThing"),
        "a tool called only by its own test is NOT reachable — if this passes, the scan \
         accepts any `X.call(` and would have declared the three deleted tools healthy"
    );

    let orphans: Vec<String> = unreachable(&corpus).into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        orphans,
        vec!["OrphanThing".to_string()],
        "the scan must flag the untethered type and only that one"
    );
}

#[test]
fn an_inline_cfg_test_module_cannot_confer_reachability() {
    // The one carrier `collect_rs`'s file/dir exclusions cannot reach. Both directions are
    // asserted, because a strip that removed everything would pass the first half alone.
    let orphan = strip_cfg_test_modules(
        r#"
            impl Tool for TestOnlyThing {}

            #[cfg(test)]
            mod tests {
                fn build() { let v: Vec<Arc<dyn Tool>> = vec![Arc::new(TestOnlyThing)]; }
                async fn go() { match k { "x" => TestOnlyThing.call(i, c).await, _ => () } }
            }
        "#,
    );
    let corpus = vec![(PathBuf::from("synthetic.rs"), orphan)];
    assert!(
        declared_tool_types(&corpus).contains_key("TestOnlyThing"),
        "the impl block sits OUTSIDE the test module and must survive the strip — if it \
         does not, the type vanishes from both sides and the orphan is hidden rather than \
         caught"
    );
    assert!(
        !reachable_tool_types(&corpus).contains("TestOnlyThing"),
        "a type constructed or dispatched to only from an inline #[cfg(test)] module is \
         NOT reachable; this is the shape tool_guide.rs exhibits in production code"
    );

    // Negative control: the identical construction outside a test module still confers it.
    let prod = vec![(
        PathBuf::from("prod.rs"),
        strip_cfg_test_modules("impl Tool for ProdThing {}\nfn b() { Arc::new(ProdThing); }"),
    )];
    assert!(
        reachable_tool_types(&prod).contains("ProdThing"),
        "the strip must not eat production registrations — without this the test above \
         passes for a strip that deletes the whole file"
    );
}

#[test]
fn the_inline_test_strip_is_not_filtering_an_empty_set() {
    // Pins a real occurrence, mirroring `test_fixture_modules_are_excluded_from_both_sides`:
    // if no production file carries an inline #[cfg(test)] module that constructs a tool,
    // the strip is filtering nothing and the guard above is untested against reality.
    let path = repo_root().join("src/mcp_resources/tool_guide.rs");
    let raw = std::fs::read_to_string(&path).expect("tool_guide.rs must exist");
    assert!(
        raw.contains("#[cfg(test)]") && raw.contains("Arc::new(EditFile)"),
        "expected an inline #[cfg(test)] module constructing a real tool. If this moved, \
         find another occurrence rather than deleting the test — its job is to prove the \
         strip has something to strip."
    );
    assert!(
        !strip_cfg_test_modules(&raw).contains("Arc::new(EditFile)"),
        "the strip left a test-module construction behind"
    );
}

#[test]
fn test_fixture_modules_are_excluded_from_both_sides() {
    let scanned = rust_sources();
    assert!(
        !scanned
            .iter()
            .any(|(p, _)| p.file_name().and_then(|n| n.to_str()) == Some("tests.rs")),
        "tests.rs files must not be scanned"
    );
    // Non-vacuous: such a file must actually exist, or the exclusion above is filtering
    // an empty set and this test asserts nothing.
    assert!(
        repo_root().join("src/tools/core/tests.rs").exists(),
        "expected at least one tests.rs fixture module to exist, or this exclusion is \
         untested"
    );
}

#[test]
fn no_stale_tolerance_entries() {
    let sources = rust_sources();
    let declared = declared_tool_types(&sources);
    let all_text: String = sources.iter().map(|(_, t)| t.as_str()).collect();

    for name in KNOWN_DELEGATION_ONLY {
        assert!(
            declared.contains_key(*name),
            "KNOWN_DELEGATION_ONLY lists `{name}`, which no longer implements `Tool`. \
             Remove it — a stale tolerance entry masks the next real orphan."
        );
        assert!(
            all_text.contains(&format!("{name}.call(")),
            "KNOWN_DELEGATION_ONLY claims `{name}` is reached by a named delegation, but no \
             `{name}.call(` dispatch exists. The delegation arm was removed and the type is \
             now genuinely unreachable — this entry is hiding that."
        );
    }
}
