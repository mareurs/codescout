//! Standing guard: a present-tense document naming a tool parameter that does not exist.
//!
//! This is `issue-clusters:IC-11` (`cluster/doc-contradicted-by-code`) — documentation that
//! denies or misstates what the code does, because the prose was **true when written**. That
//! entry's spread adjudication (2026-09-01) split its four members three ways by what the prose
//! is a claim *about*, and found exactly one sub-shape mechanizable:
//!
//! - **behavioural** — "the code does X". Every path and symbol it cites is correct. No
//!   reference check can reach it, by construction.
//! - **decision** — "use remedy B", after another *document* recorded B as non-viable. The
//!   falsifying artifact is not code at all.
//! - **named-entity** — the doc names four things that do not exist. **This file.**
//!
//! `audit_doc_refs` cannot cover the third, and the reason is not the one that entry originally
//! gave. Two hypotheses were refuted before the right one: fenced blocks are *not* skipped
//! (`parser.rs` walks them and severity-caps to `code_block`), and the refs are not found-and-
//! downgraded either. `RefKind` has five variants — `FilePath`, `FileLine`, `FileSymbol`,
//! `ModulePath`, `Link` — and **all five are locations**. A tool parameter is not a location, so
//! the instrument never sees it. This guard supplies the missing candidate kind as a test rather
//! than as a sixth `RefKind`, because a test needs no wiring to be reachable and the registry has
//! no enumerable form outside `server.rs`'s own private test module.
//!
//! **Why this stops at the anchored call form, measured 2026-09-01.** A prose *mention* of
//! a dead tool is not checkable here, and two guards for it were tried and rejected rather
//! than skipped. (a) Extending `DEPRECATED_TOOL_NAMES`' `!contains` denylist to
//! `docs/manual/`: of ~25 retired-name occurrences there, roughly **20 are legitimately
//! historical** — migration tables, a changelog, a `> Removed 2026-09-01.` banner, and a
//! rename note — so the check would force deleting correct history, which is `IC-6`, a
//! parser over a namespace with no escape hatch. (b) A heading-level guard, "no page is
//! *titled* after a dead tool": **1 real defect in 4 hits**, because `## render_template`
//! and `## params_schema` are augmentation *fields* and `# tracker_design` is a `librarian`
//! action — indistinguishable from a tool name by shape. The anchored call form works
//! precisely because it separates *calls it* from *mentions it* by construction, and prose
//! offers no equivalent discriminator. Filed instead:
//! `docs/issues/2026-09-01-librarian-mcp-page-describes-a-separate-server-that-was-collapsed.md`.
//!
//! **Scope is principled, not an allowlist.** Only surfaces where naming a parameter is a claim
//! about *current* code are scanned: the manual, the served guides, and the root documents.
//! `docs/issues/`, `docs/plans/`, `docs/superpowers/` and archives are excluded because a bug
//! file describing a past state, or a spec proposing a future parameter, is not asserting the
//! parameter exists today — which is precisely `IC-11`'s own inclusion test.
//!
//! Measured 2026-09-01 when this landed: 131 present-tense files carrying **313** anchored
//! citations, out of 3207 across the whole corpus. **6** parameter violations here and **4**
//! retired tool names, against 32 hits on historical surfaces that are correctly out of scope.
//! All ten were genuine: `symbols(pattern=)` twice in `docs/PROGRESSIVE_DISCOVERABILITY.md` —
//! the page `CLAUDE.md` tells you to read before touching any tool — plus the manual's
//! `read_file`/`edit_file` heading parameters, `edit_section` (renamed to `edit_markdown` in
//! v0.11, by the same page's own changelog), and `artifact_refresh_stale`.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

/// An anchored call in prose: `tool_name(… param= …)`.
///
/// The anchor is what makes this checkable at all. A bare backticked identifier could be a
/// parameter, a field, a local or an English word; `symbols` and `grep` *are* English words. The
/// tool name plus the open paren is what pins the token to a specific schema, and it is the form
/// this project's docs actually use — 3207 occurrences corpus-wide, against 5 anchored calls to
/// a name that is not a tool.
///
/// **Every named argument on the line is read, not just the first.** An earlier draft anchored
/// on `name\(\s*param\s*=`, which sees only the leading argument — and so reported green over
/// `read_file(path, headings=[...])`, a live violation sitting in the manual's own *Recommended
/// Workflow* table. A guard that checks the first argument and passes is worse than no guard,
/// because the green tick is read as coverage of the whole call.
const CALL_OPEN: &str = r"\b([a-z][a-z0-9]*(?:_[a-z0-9]+)*)\(";
const NAMED_ARG: &str = r"\b([a-z_][a-z0-9_]*)\s*=";

/// Names that are live tool aliases but are not the primary `fn name()` of any tool.
///
/// Empty on purpose. `activate_project` is *not* here: it is declared as
/// `pub const NAME: &'static str = "activate_project"` (`src/tools/config/mod.rs:117`), and
/// [`tool_names`] reads that form too. It was in an earlier draft of this list, put there
/// because a `fn name()`-only scan reported it missing — the instrument's blind spot arriving
/// as a plausible finding rather than an error. Widening the extractor was the fix; an
/// allowlist entry would have hidden the defect and then silently accepted a genuinely dead
/// name later.
const ALIAS_ALLOWLIST: &[&str] = &[];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file under `src/`, with inline `#[cfg(test)]` modules removed.
///
/// The strip is load-bearing and is the same one `tests/tool_reachability.rs` carries, for the
/// same reason: a test fixture declaring `fn name() { "fake_tool" }` with its own schema would
/// enter the accepted set and make `fake_tool(anything=)` pass in a document. Membership must
/// come from production code only. Duplicated rather than shared because integration tests are
/// separate crates; see that file's `strip_cfg_test_modules` for the original.
fn production_sources() -> Vec<String> {
    fn walk(dir: &std::path::Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                if let Ok(t) = std::fs::read_to_string(&p) {
                    out.push(strip_cfg_test_modules(&t));
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo_root().join("src"), &mut out);
    out
}

/// Remove every `#[cfg(test)] mod … { … }` block by brace matching.
fn strip_cfg_test_modules(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("#[cfg(test)]") {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let Some(open) = tail.find('{') else {
            break;
        };
        let mut depth = 0usize;
        let mut end = None;
        for (i, c) in tail[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        match end {
            Some(e) => rest = &tail[e..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Every name an agent can dispatch, from **both** declaration forms.
///
/// `fn name(&self) -> &str { "grep" }` is the common one; `pub const NAME: &'static str = "…"`
/// is the other, and missing it is what made `activate_project` look retired.
fn tool_names() -> HashSet<String> {
    let by_fn = regex::Regex::new(
        r#"fn name\(&self\)\s*->\s*&(?:'static\s+)?str\s*\{\s*"([a-z_][a-z0-9_]*)""#,
    )
    .unwrap();
    let by_const =
        regex::Regex::new(r#"const NAME:\s*&'static str\s*=\s*"([a-z_][a-z0-9_]*)""#).unwrap();
    let mut names = HashSet::new();
    for text in production_sources() {
        for c in by_fn.captures_iter(&text) {
            names.insert(c[1].to_string());
        }
        for c in by_const.captures_iter(&text) {
            names.insert(c[1].to_string());
        }
    }
    names
}

/// Tool name → the parameter names its JSON schema declares at the top level.
///
/// Only depth-1 keys of the `"properties"` object count. Descending would fold a nested item
/// schema's keys (`edits[].old_string`) into the tool's own parameter set, which loosens the
/// guard in the direction that produces false *negatives* — the direction no assertion here
/// could report.
fn tool_params() -> HashMap<String, HashSet<String>> {
    let name_re = regex::Regex::new(
        r#"fn name\(&self\)\s*->\s*&(?:'static\s+)?str\s*\{\s*"([a-z_][a-z0-9_]*)""#,
    )
    .unwrap();
    let key_re = regex::Regex::new(r#""([a-zA-Z_][a-zA-Z0-9_]*)"\s*:"#).unwrap();
    let mut map: HashMap<String, HashSet<String>> = HashMap::new();

    for text in production_sources() {
        for m in name_re.captures_iter(&text) {
            let name = m[1].to_string();
            let after = m.get(0).unwrap().end();
            let Some(schema_at) = text[after..].find("fn input_schema").map(|i| after + i) else {
                continue;
            };
            let Some(props_at) = text[schema_at..]
                .find("\"properties\"")
                .map(|i| schema_at + i)
            else {
                continue;
            };
            let Some(open) = text[props_at..].find('{').map(|i| props_at + i) else {
                continue;
            };

            // Walk the properties object, collecting only keys sitting at brace depth 1.
            let bytes = &text.as_bytes()[open..];
            let mut depth = 0usize;
            let mut spans: Vec<(usize, usize)> = Vec::new();
            let mut seg_start = 0usize;
            for (i, &b) in bytes.iter().enumerate() {
                match b {
                    b'{' => {
                        depth += 1;
                        if depth == 1 {
                            seg_start = i + 1;
                        } else if depth == 2 {
                            spans.push((seg_start, i));
                        }
                    }
                    b'}' => {
                        depth -= 1;
                        if depth == 1 {
                            seg_start = i + 1;
                        } else if depth == 0 {
                            spans.push((seg_start, i));
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let entry = map.entry(name).or_default();
            for (a, b) in spans {
                for k in key_re.captures_iter(&text[open + a..open + b]) {
                    entry.insert(k[1].to_string());
                }
            }
        }
    }
    map
}

/// Documents that assert what the tools accept **right now**.
///
/// Deliberately excludes `docs/issues/`, `docs/plans/`, `docs/superpowers/` and every archive:
/// a bug file recording that `call_graph(depth=2)` timed out is describing the world as it was,
/// and a spec proposing `references(kind="call")` is describing a world that does not exist yet.
/// Neither asserts the parameter exists, so neither is this class. Measured 2026-09-01: 32 hits
/// sit on those surfaces and every one sampled was a past or proposed state.
fn present_tense_surfaces() -> Vec<PathBuf> {
    let root = repo_root();
    let mut out = Vec::new();

    fn walk_md(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk_md(&p, out);
            } else if p.extension().is_some_and(|x| x == "md") {
                out.push(p);
            }
        }
    }
    walk_md(&root.join("docs/manual"), &mut out);
    walk_md(&root.join("src/prompts/guides"), &mut out);

    for f in [
        "docs/PROGRESSIVE_DISCOVERABILITY.md",
        "docs/PROBES.md",
        "docs/RELEASE.md",
        "docs/TAXONOMY.md",
        "CLAUDE.md",
        "README.md",
        "CONTRIBUTING.md",
    ] {
        let p = root.join(f);
        if p.is_file() {
            out.push(p);
        }
    }
    out
}

/// One anchored citation: where it is, and what it claims.
struct Cite {
    file: String,
    line: usize,
    tool: String,
    param: String,
    text: String,
}

fn anchored_cites() -> Vec<Cite> {
    let open = regex::Regex::new(CALL_OPEN).unwrap();
    let arg = regex::Regex::new(NAMED_ARG).unwrap();
    let quoted = regex::Regex::new(r#""[^"]*"|'[^']*'"#).unwrap();
    let root = repo_root();
    let mut out = Vec::new();
    for path in present_tense_surfaces() {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string();
        for (i, line) in text.lines().enumerate() {
            for c in open.captures_iter(line) {
                let tool = c[1].to_string();
                let after = c.get(0).unwrap().end();
                // The argument list, bounded by BALANCED parens and taken at depth 1 only.
                //
                // Both halves are load-bearing. Running to the last `)` on the line billed a
                // neighbouring call's arguments to this one — `artifact_augment(merge=…)` sitting
                // after `artifact(…)` produced a phantom `artifact(merge=`, and the report was
                // 20 violations of which 14 were that. Taking nested depth would bill an inner
                // call's arguments too. Quoted spans are blanked first, so an `=` inside a string
                // literal (`grep(pattern="a=b")`) cannot be read as a named argument.
                let mut depth = 1usize;
                let mut span = String::new();
                for ch in line[after..].chars() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    if depth == 1 {
                        span.push(ch);
                    }
                }
                let span = quoted.replace_all(&span, "");
                for a in arg.captures_iter(&span) {
                    out.push(Cite {
                        file: rel.clone(),
                        line: i + 1,
                        tool: tool.clone(),
                        param: a[1].to_string(),
                        text: line.trim().chars().take(110).collect(),
                    });
                }
            }
        }
    }
    out
}

/// A present-tense document naming a parameter its tool does not declare.
///
/// The failure this catches is not "a reader is misinformed". A document naming a parameter that
/// does not exist sends an agent to call it, and codescout's tools ignore unknown keys rather
/// than rejecting them (`IC-15`), so the call *succeeds* and silently does something else.
#[test]
fn a_documented_tool_parameter_exists_on_that_tool() {
    let schemas = tool_params();
    let mut bad: Vec<String> = Vec::new();

    for c in anchored_cites() {
        let Some(params) = schemas.get(&c.tool) else {
            continue; // not a tool, or no schema — the sibling test owns that case
        };
        if !params.contains(&c.param) {
            let mut known: Vec<&str> = params.iter().map(String::as_str).collect();
            known.sort_unstable();
            bad.push(format!(
                "  {}:{}\n      `{}({}=` — {} has no such parameter.\n      line: {}\n      declared: {}",
                c.file,
                c.line,
                c.tool,
                c.param,
                c.tool,
                c.text,
                known.join(", ")
            ));
        }
    }

    assert!(
        bad.is_empty(),
        "{} present-tense document(s) name a tool parameter that does not exist.\n\n{}\n\n\
         Fix the document, or — if the parameter is proposed rather than shipped — move the \
         claim to a spec or plan, which this scan deliberately does not read.",
        bad.len(),
        bad.join("\n\n")
    );
}

/// An anchored call to a name no tool answers to.
///
/// Separate from the parameter test because the remedies differ: a wrong parameter is a one-token
/// edit, a retired tool name means the page describes an interface that no longer exists and
/// usually needs rewriting rather than patching.
#[test]
fn a_documented_call_names_a_live_tool() {
    let names = tool_names();
    let allowed: HashSet<&str> = ALIAS_ALLOWLIST.iter().copied().collect();
    let mut bad: Vec<String> = Vec::new();

    for c in anchored_cites() {
        // Only snake_case names — a single-word call in prose is too often ordinary English.
        if !c.tool.contains('_') {
            continue;
        }
        if names.contains(&c.tool) || allowed.contains(c.tool.as_str()) {
            continue;
        }
        bad.push(format!(
            "  {}:{}\n      `{}(…)` names no registered tool.\n      line: {}",
            c.file, c.line, c.tool, c.text
        ));
    }

    assert!(
        bad.is_empty(),
        "{} present-tense document(s) call a tool that does not exist.\n\n{}\n\n\
         If the name is a live alias, add it to ALIAS_ALLOWLIST with the declaration site — \
         but read that constant's comment first: the last candidate for it was a gap in this \
         file's own extractor, not a real alias.",
        bad.len(),
        bad.join("\n\n")
    );
}

/// Non-vacuity. Both assertions above are `is_empty()`, which is **monotone under removal** — a
/// scan that reads nothing, a regex that matches nothing, or a surface list that resolves to no
/// files all produce exactly the silence they assert. None of that is detectable from the
/// passing side, which is `CLAUDE.md` § *Testing Discipline*'s first law and the whole of
/// `IC-16`.
///
/// So this pins the population from below, with a known-good citation that must resolve. If the
/// extractor breaks, the corpus empties, or `doc` loses its `action` parameter, this fails
/// while its two siblings stay green.
#[test]
fn the_scan_is_not_reading_an_empty_corpus() {
    let surfaces = present_tense_surfaces();
    assert!(
        surfaces.len() > 50,
        "present-tense surface list collapsed to {} files — the two siblings would pass \
             vacuously",
        surfaces.len()
    );

    let schemas = tool_params();
    assert!(
        schemas.len() > 20,
        "only {} tool schemas extracted — the parameter test would skip nearly every citation",
        schemas.len()
    );
    assert!(
        tool_names().contains("activate_project"),
        "`activate_project` is declared as `const NAME` (src/tools/config/mod.rs); losing it \
             means the extractor is back to reading only `fn name()`, and every const-declared \
             tool would read as retired"
    );

    let cites = anchored_cites();
    assert!(
        cites.len() > 200,
        "only {} anchored citations found across {} files — measured 313 on 2026-09-01, so a \
             collapse to double digits means the regex or the walk is broken",
        cites.len(),
        surfaces.len()
    );

    // A citation that must resolve: `doc(action=…)` is the single most cited call in this
    // repo's documentation, and `action` is its required discriminator.
    let resolved = cites.iter().any(|c| c.tool == "doc" && c.param == "action");
    assert!(
        resolved,
        "no `doc(action=` citation resolved — the scan is running but reading the wrong \
             thing"
    );

    // And the schema side must actually contain it, or the parameter test accepts anything.
    let params: BTreeSet<String> = schemas
        .get("doc")
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect();
    assert!(
        params.contains("action"),
        "doc's extracted schema has no `action` key — got {params:?}"
    );
}
