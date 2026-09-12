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
///
/// `docs/architecture/` was added 2026-09-06. It had been in neither the walk nor the exclusion
/// list above, so a reader auditing coverage met a thoughtful, complete-looking rationale that
/// simply did not mention it — which is the shape that stops anyone asking.
///
/// **It red on its first run, and not on what the bug predicted.** The bug cited 10 dead-tool call
/// sites in `augmented-artifacts.md` as the acceptance RED; by the time it was actioned all 10 had
/// been repaired by the collapse programme's own sweep, and a pre-check for retired tool names over
/// the directory found none. What it actually caught was `augmented-artifacts.md:75` — a malformed
/// `doc(action="augment", …, augment={prompt: ..., params=...)`, whose unclosed brace makes the
/// regex read `params=` as a top-level argument of `doc`, which has no such parameter. Line 210 of
/// the same file already carried the correct form, so the document disagreed with itself.
///
/// Worth the paragraph because the pre-check was *sound and still misleading*: it asked "are there
/// dead tool NAMES here?" and answered no, and that was generalised to "this addition will find
/// nothing". These two tests ask two questions — is the name live, and does the parameter exist —
/// and a survey of one says nothing about the other. Wiring was proved separately by planting a
/// dead call under the directory and watching this gate red, then reverting; the suite passing is
/// not what established it.
///
/// What adding a directory does NOT reach: bare prose that names a dead tool without writing a
/// call. That is [`CALL_OPEN`]'s anchor working as designed, and it is a different defect class
/// with its own bug file. A directory added to the walk is not the same as a file made correct.
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
    walk_md(&root.join("docs/architecture"), &mut out);

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

/// One call found on one line: what it names, what it claims, and whether the scanner could
/// read it at all.
#[derive(Debug)]
struct LineCall {
    tool: String,
    params: Vec<String>,
    /// The argument span closed its `(` with a `{` or `[` still open. The call is malformed
    /// — or the scanner has met a shape it reads wrongly — and either way its `params` are a
    /// guess. See [`a_documented_call_closes_its_own_literals`].
    ///
    /// **Both halves of the condition are load-bearing, and the corpus is what taught it.**
    /// Flagging `nest > 0` alone fired on 8 correct documents, every one a call whose
    /// arguments WRAP across lines: the scan is per-line, so such a line ends with its `(`
    /// still open and its literal still open, which is indistinguishable from malformed by
    /// depth alone. Requiring the parenthesis to have CLOSED separates them — a wrapped call
    /// never reaches `)`, a malformed one does. Wrapped calls are a silent-miss shape, not a
    /// false-RED one, and belong to the scanner's own bug file rather than to this gate.
    unclosed: bool,
}

/// The per-line core of [`anchored_cites`], extracted so that the inputs this scanner reads
/// WRONG can be written as tests at all.
///
/// The extraction is part of the fix, not tidying around it. This defect is `IC-6` — a legal
/// syntax the scanner cannot represent — and that class's signature is *"no test can be
/// written, because the case cannot be expressed"*. Here that was true in a second, concrete
/// way: the only entry point walked the filesystem, so a fixture had nowhere to live except
/// the corpus this gate scans, and planting one there makes the gate's own input a fixture.
/// A pure function over one line gives the case somewhere to exist.
fn calls_on_line(line: &str) -> Vec<LineCall> {
    // Compiled once, not per line. The first cut of this extraction built all three inside
    // the function, so they were rebuilt for every LINE of every surface, taking this suite
    // from 0.32s to 133s. Invisible to every assertion here — all four tests still passed,
    // because a 400x slowdown is not a wrong answer. Recorded rather than quietly fixed: a
    // pure per-line function is the right shape for testability and the wrong shape for regex
    // construction, and the two are reconciled here rather than by giving up either.
    static OPEN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static ARG: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static QUOTED: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let open = OPEN.get_or_init(|| regex::Regex::new(CALL_OPEN).unwrap());
    let arg = ARG.get_or_init(|| regex::Regex::new(NAMED_ARG).unwrap());
    let quoted = QUOTED.get_or_init(|| regex::Regex::new(r#""[^"]*"|'[^']*'"#).unwrap());

    let mut out = Vec::new();
    for c in open.captures_iter(line) {
        let tool = c[1].to_string();
        let after = c.get(0).unwrap().end();
        // The argument list, bounded by BALANCED delimiters and taken at this call's own
        // depth only. Every clause below is load-bearing.
        //
        // Running to the last `)` on the line billed a neighbouring call's arguments to this
        // one — `artifact_augment(merge=…)` sitting after `artifact(…)` produced a phantom
        // `artifact(merge=`, and the report was 20 violations of which 14 were that. Taking
        // nested PAREN depth would bill an inner call's arguments to the outer one.
        //
        // `{}` and `[]` count alongside `()` because a `key=value` inside an object or array
        // literal is an argument of NOTHING — it is a field of that literal. Counting parens
        // alone billed it to the enclosing call and produced a false RED naming a real tool
        // and a real-looking parameter, with nothing pointing at the nesting as the cause.
        // The corpus dodged it by writing `:` inside `{}`, which is the house style anyway —
        // so a parser limitation silently dictated a prose convention.
        //
        // Delimiters inside a double-quoted string are not counted, because `json_path=
        // "$.rows[*].id"` would otherwise open a literal that the rest of the span never
        // closes, dropping every argument after it. Single quotes are deliberately NOT
        // tracked: an apostrophe in prose ("the tool's `doc(action=…)`") is far commoner
        // here than a single-quoted argument, and treating one as a string opener would
        // swallow real calls. `quoted` below still blanks both for the `=` scan, where a
        // stray apostrophe costs nothing.
        let mut paren = 1usize;
        let mut nest = 0usize;
        let mut in_str = false;
        let mut closed = false;
        let mut span = String::new();
        for ch in line[after..].chars() {
            if ch == '"' {
                in_str = !in_str;
            } else if !in_str {
                match ch {
                    '(' => paren += 1,
                    ')' => {
                        paren -= 1;
                        if paren == 0 {
                            closed = true;
                            break;
                        }
                    }
                    '{' | '[' => nest += 1,
                    '}' | ']' => nest = nest.saturating_sub(1),
                    _ => {}
                }
            }
            if paren == 1 && nest == 0 {
                span.push(ch);
            }
        }
        // Blanked so an `=` inside a string literal (`grep(pattern="a=b")`) cannot be read as
        // a named argument.
        let span = quoted.replace_all(&span, "");
        out.push(LineCall {
            tool,
            params: arg.captures_iter(&span).map(|a| a[1].to_string()).collect(),
            unclosed: closed && nest > 0,
        });
    }
    out
}

/// Flat `(tool, param)` view of [`calls_on_line`], for tests that do not care about
/// malformedness.
fn named_args_on_line(line: &str) -> Vec<(String, String)> {
    calls_on_line(line)
        .into_iter()
        .flat_map(|c| {
            c.params
                .into_iter()
                .map(move |p| (c.tool.clone(), p))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn anchored_cites() -> Vec<Cite> {
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
            for call in calls_on_line(line) {
                for param in call.params {
                    out.push(Cite {
                        file: rel.clone(),
                        line: i + 1,
                        tool: call.tool.clone(),
                        param,
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
             claim to a spec or plan, which this scan deliberately does not read.\n\n\
             IF THE PARAMETER LOOKS CORRECT, suspect the scanner before the document — and it \
             will not volunteer this, so it is written here. It reads one LINE at a time and \
             counts `()`, `{{}}` and `[]` outside double-quoted strings to find a call's own \
             arguments. Two shapes it gets wrong:\n  \
             - a `key=value` inside an object or array literal belongs to that literal, not to \
             the call. That is fixed and pinned, but a THIRD nesting form would land here \
             looking exactly like a real violation.\n  \
             - single quotes are not tracked as strings (an apostrophe in prose is commoner \
             here than a single-quoted argument), so `'a=b'` inside a call still reads as a \
             named argument.\n  \
             There is no escape at the citation site: a fenced block is not exempt and the \
             scanner cannot be told to skip a line. Rewrite the example, and add the shape to \
             the scanner's bug file rather than working around it silently — the last time a \
             limitation here was worked around, it silently dictated a prose convention and \
             nothing recorded that it had.",
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
        "only {} anchored citations found across {} files — measured 313 on 2026-09-01 and 713 \
             on 2026-09-12, so this population GROWS with the corpus and a reading far above \
             either is expected, not a defect. The floor is deliberately far below both: it \
             exists to catch a collapse to double digits, which means the regex or the walk is \
             broken, and it is not a ratchet.",
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

/// The reproduction from
/// `docs/issues/archive/2026-09-02-anchored-cites-tracks-parens-but-not-braces-so-nested-object-args-misattribute.md`,
/// and the first test this scanner was ever able to have. Before `calls_on_line` was
/// extracted, the only entry point walked the corpus, so expressing this case meant planting
/// it in a document the gate scans — which is why an `IC-6` member sat filed-but-untested for
/// ten days.
///
/// `prompt`, `params` and `inner` are fields of an object literal, so they are arguments of
/// NOTHING. Billed to `doc`, each is a false RED naming a real tool and a plausible parameter.
#[test]
fn a_named_argument_inside_a_nested_object_is_not_billed_to_the_outer_call() {
    let got = named_args_on_line(
        r#"doc(action="augment", id="x", augment={prompt=1, params={inner=2}})"#,
    );
    let params: Vec<&str> = got.iter().map(|(_, p)| p.as_str()).collect();
    assert_eq!(
        params,
        vec!["action", "id", "augment"],
        "expected only `doc`'s own three arguments. Extra names mean literal nesting is being \
         billed to the enclosing call (the filed defect); MISSING names mean the walker now \
         over-suppresses and real citations are being dropped, which turns this whole gate \
         green for the wrong reason. Both directions are failures and this assertion is \
         deliberately an equality, not a `contains`."
    );
}

/// Over-match guard for the fix above, and not a hypothetical one: `read_file(path,
/// headings=[...])` is the exact shape of a live violation that `CALL_OPEN`'s own doc comment
/// records this gate catching in the manual's *Recommended Workflow* table. A depth-aware
/// walker that suppressed a `key=` merely for sitting NEAR a literal would lose it and look
/// like a clean fix.
#[test]
fn a_key_whose_value_is_a_literal_is_still_billed_to_the_call() {
    let got = named_args_on_line("read_file(path, headings=[...])");
    assert!(
        got.contains(&("read_file".into(), "headings".into())),
        "`headings=` is the call's own argument — only its VALUE is nested. Suppressing it \
         would silence a violation this gate is documented as having caught: {got:?}"
    );
}

/// The second latent case named in the same bug: a delimiter inside a string literal was
/// counted as real, because quoted spans are blanked only after the walk. An unbalanced one
/// opened a literal the span never closed, dropping every argument after it — a SILENT loss,
/// the opposite failure direction from the false RED above and the more dangerous one.
///
/// Fixed by making the walk quote-aware rather than by blanking the line first: blanking
/// first would let a prose apostrophe (`the tool's ...`) swallow a real call.
#[test]
fn an_unbalanced_delimiter_inside_a_string_does_not_swallow_the_rest_of_the_span() {
    let got = named_args_on_line(r#"grep(pattern="a[b", glob="x")"#);
    let params: Vec<&str> = got.iter().map(|(_, p)| p.as_str()).collect();
    assert_eq!(
        params,
        vec!["pattern", "glob"],
        "the `[` lives inside a string literal and opens nothing. Losing `glob` means the \
         walker counted it: every argument after an unbalanced delimiter in a string goes \
         unscanned, and the gate reports green over them."
    );
}

/// Pins the two behaviours the walker already had, so the depth change above cannot quietly
/// cost either. The neighbouring-call case was measured: running to the last `)` on the line
/// produced 20 reported violations of which 14 were phantoms billed across calls.
#[test]
fn the_walker_still_separates_neighbouring_calls_and_ignores_quoted_equals() {
    let neighbours = named_args_on_line("artifact(id=1) and artifact_augment(merge=true)");
    assert_eq!(
        neighbours,
        vec![
            ("artifact".to_string(), "id".to_string()),
            ("artifact_augment".to_string(), "merge".to_string()),
        ],
        "a neighbouring call's arguments must not be billed to this one: {neighbours:?}"
    );

    let quoted = named_args_on_line(r#"grep(pattern="a=b")"#);
    assert_eq!(
        quoted,
        vec![("grep".to_string(), "pattern".to_string())],
        "`a=b` is inside a string literal and names no parameter: {quoted:?}"
    );
}

/// **The catch the depth fix would otherwise have SILENTLY removed, converted into a louder
/// one.** `present_tense_surfaces`' own doc comment records this gate catching
/// `augment={prompt: ..., params=...)` — a malformed call whose unclosed brace made the
/// paren-only walker read `params=` as a top-level argument of `doc`. Counting braces makes
/// that field fall at depth 1, so the malformed document would now pass unremarked.
///
/// Rather than accept that trade, the walker records `unclosed` and this reports it. The
/// class's standing remedy is to say so at the refusal site: a scanner that cannot read a
/// line should name it, not guess and not shrug.
#[test]
fn a_documented_call_closes_its_own_literals() {
    let root = repo_root();
    let mut bad: Vec<String> = Vec::new();
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
            for call in calls_on_line(line) {
                if call.unclosed {
                    bad.push(format!(
                        "  {}:{}\n      `{}(` closes its parenthesis with a `{{` or `[` still \
                         open\n      line: {}",
                        rel,
                        i + 1,
                        call.tool,
                        line.trim().chars().take(110).collect::<String>()
                    ));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} documented call(s) close a parenthesis with an object or array literal still \
         open.\n\n{}\n\n\
         The document is almost certainly missing a `}}` or `]` \u{2014} that is the shape this \
         caught when it was still doing so by accident, via a paren-only walker that billed \
         the orphaned field to the enclosing call. A call whose arguments merely WRAP across \
         lines is not this and is not reported here: its paren never closes on the line, so \
         the two are distinguishable. If the call really is well-formed, the scanner has met \
         a third shape — record it on its bug file rather than working around it silently.",
        bad.len(),
        bad.join("\n\n")
    );
}

/// Non-vacuity for the gate above, in both directions. An `unclosed` flag that never fires is
/// exactly the silence a correct corpus produces, so the passing side proves nothing on its
/// own; and a flag that fires on everything would make the gate unreadable rather than wrong.
#[test]
fn the_unclosed_flag_fires_on_the_malformed_form_and_not_the_correct_one() {
    let malformed = calls_on_line(r#"doc(action="augment", augment={prompt: 1, params=2)"#);
    assert!(
        malformed.iter().any(|c| c.tool == "doc" && c.unclosed),
        "the historical malformed line must be flagged: {malformed:?}"
    );
    let correct = calls_on_line(r#"doc(action="augment", augment={prompt: 1, params: 2})"#);
    assert!(
        correct.iter().all(|c| !c.unclosed),
        "the corrected form differs only by the closing brace and must NOT be flagged, or the \
         gate reds on every well-formed nested call: {correct:?}"
    );
    // The arm the corpus added. Flagging `nest > 0` alone fired on 8 correct documents, all
    // of this shape — a call whose arguments wrap, whose first line therefore ends with both
    // its paren and its literal open. Requiring the paren to have CLOSED is what separates
    // malformed from merely wrapped, and without this arm that distinction can be deleted
    // with the two assertions above still green.
    let wrapped = calls_on_line(r#"doc(update, id=X, patch={body_edits: [{"#);
    assert!(
        wrapped.iter().all(|c| !c.unclosed),
        "a call whose arguments wrap across lines is not malformed \u{2014} its paren never closes on \
         this line. Flagging it makes this gate red on correct documentation: {wrapped:?}"
    );
}
