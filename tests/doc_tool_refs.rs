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

    // The advertised surface is the source schema PLUS what `list_tools` injects, and reading
    // only the source made this guard narrower than the tool it checks. `server.rs`'s
    // `inject_workspace_param` adds an optional `workspace` pin to every `pinnable()` tool
    // between `input_schema()` and the wire, so `{"tool": "symbols", "arguments":
    // {"workspace": …}}` is a CORRECT document that a source-only extractor reports as naming a
    // parameter that does not exist. Found 2026-09-13 when the JSON-payload scan reached the one
    // page using it; the prose scan never had. `server.rs`'s own `tool_surface_chars` already
    // reproduces this injection for the byte budget, with a comment saying a bare
    // `input_schema()` measurement "would miss ~6.2 KB of injected `workspace` prose" — the
    // knowledge existed in the codebase and this file did not share it.
    let unpinnable = unpinnable_tools();
    assert!(
        !unpinnable.is_empty(),
        "control: `Tool::pinnable`'s exclusion list parsed EMPTY, which would silently grant \
         every tool a `workspace` parameter and loosen this guard in the false-negative \
         direction — the one direction no assertion in this file can report"
    );
    for (name, params) in map.iter_mut() {
        if !unpinnable.contains(name) {
            params.insert("workspace".to_string());
        }
    }
    map
}

/// Tools that do NOT receive the injected `workspace` pin.
///
/// Read from `Tool::pinnable`'s own `matches!` arm rather than restated, so adding a tool to that
/// list cannot leave this guard checking against a surface the server no longer advertises.
fn unpinnable_tools() -> HashSet<String> {
    // Compiled once. The same shape cost this suite 0.32s -> 133s in `calls_on_line` and was
    // invisible to every assertion; clippy's `regex_creation_in_loops` catches it here, which is
    // the mechanism that lesson was missing.
    static LIT: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let lit = LIT.get_or_init(|| regex::Regex::new(r#""([a-z_][a-z0-9_]*)""#).unwrap());
    let mut out = HashSet::new();
    for text in production_sources() {
        let Some(at) = text.find("fn pinnable(&self) -> bool {") else {
            continue;
        };
        let Some(m) = text[at..].find("matches!").map(|i| at + i) else {
            continue;
        };
        // Balanced scan to `matches!`'s own closing paren. A bare `find(')')` lands on
        // `self.name()`'s paren — which sits BEFORE every string literal in the arm, so the
        // extraction silently returned nothing. The control assertion in `tool_params` caught
        // that; it is the reason this comment exists rather than a quiet `.find(')')`.
        let bytes = text.as_bytes();
        let Some(open) = text[m..].find('(').map(|i| m + i) else {
            continue;
        };
        let mut depth = 0usize;
        let mut close = open;
        for (i, &b) in bytes[open..].iter().enumerate() {
            match b {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        if close <= open {
            continue;
        }
        let lit_matches = lit.captures_iter(&text[m..close]);
        for c in lit_matches {
            out.insert(c[1].to_string());
        }
    }
    out
}

/// Tool name → the values its `action` parameter declares in `"enum"`.
///
/// **This exists to keep the bare-identifier scan from firing on the corpus's dominant idiom.**
/// `doc(get)`, `librarian(reindex)`, `workspace(activate)` and `memory(recall)` are
/// action-dispatch shorthand for `doc(action="get")` and friends — house style throughout the
/// manual, the guides and `CLAUDE.md` itself. Measured 2026-09-12 over the 132 present-tense
/// surfaces at `408709ea`: **45 of the 48** bare identifiers that are not parameters are this
/// shape. Billing them as parameter claims would have produced 45 false REDs.
///
/// Derived from the schema rather than allow-listed, which is the whole difference. An allowlist
/// of `get`/`update`/`reindex`/… would also silence a genuinely dead action name, and it would
/// need hand-editing every time a tool gains one. Reading `enum` means a retired action stops
/// being excused the moment it leaves the schema.
fn tool_actions() -> HashMap<String, HashSet<String>> {
    let name_re = regex::Regex::new(
        r#"fn name\(&self\)\s*->\s*&(?:'static\s+)?str\s*\{\s*"([a-z_][a-z0-9_]*)""#,
    )
    .unwrap();
    let val_re = regex::Regex::new(r#""([a-z_][a-z0-9_]*)""#).unwrap();
    let mut map: HashMap<String, HashSet<String>> = HashMap::new();

    for text in production_sources() {
        for m in name_re.captures_iter(&text) {
            let name = m[1].to_string();
            let after = m.get(0).unwrap().end();
            let Some(schema_at) = text[after..].find("fn input_schema").map(|i| after + i) else {
                continue;
            };
            // Bounded to this tool's schema: stop at the next `fn name(` so a tool with no
            // `action` cannot inherit the next tool's enum.
            let end = text[schema_at..]
                .find("fn name(&self)")
                .map(|i| schema_at + i)
                .unwrap_or(text.len());
            let region = &text[schema_at..end];
            let Some(act) = region.find("\"action\"") else {
                continue;
            };
            let Some(enum_at) = region[act..].find("\"enum\"").map(|i| act + i) else {
                continue;
            };
            let Some(open) = region[enum_at..].find('[').map(|i| enum_at + i) else {
                continue;
            };
            let Some(close) = region[open..].find(']').map(|i| open + i) else {
                continue;
            };
            let entry = map.entry(name).or_default();
            for v in val_re.captures_iter(&region[open..close]) {
                entry.insert(v[1].to_string());
            }
        }
    }
    map
}

/// The bare identifiers of `call` that are parameter CLAIMS, with action-dispatch values removed.
///
/// Extracted so the exclusion rule is exercised by tests directly. Asserting it through a
/// re-implementation in the test body would be a second level testing its own copy — green
/// whatever the shipped rule does.
fn billable_bare(call: &LineCall, actions: &HashMap<String, HashSet<String>>) -> Vec<String> {
    let acts = actions.get(&call.tool);
    call.bare
        .iter()
        .filter(|b| !acts.is_some_and(|a| a.contains(*b)))
        .cloned()
        .collect()
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

/// One parameter claim, anchored to the tool whose parens it sits in.
struct Cite {
    file: String,
    line: usize,
    tool: String,
    param: String,
    /// Written as a bare identifier (`references(symbol, path)`) rather than `key=`. Carried
    /// so the failure message can quote the form the author actually used — telling someone
    /// their `references(name_path=` is wrong when the page says `references(name_path,` sends
    /// them looking for a string that is not there.
    bare: bool,
    text: String,
}

/// One call found on one line: what it names, what it claims, and whether the scanner could
/// read it at all.
#[derive(Debug)]
struct LineCall {
    tool: String,
    params: Vec<String>,
    /// Arguments written as a BARE identifier — no `=`, no value. The manual's signature form
    /// (`references(name_path, path)`) names parameters this way, and [`params`] cannot see it
    /// because [`NAMED_ARG`] requires the `=`.
    ///
    /// Deliberately NOT merged into `params`: a bare identifier is ambiguous in a way a
    /// `key=` is not. It is a parameter NAME in `references(name_path, path)` and an argument
    /// VALUE in `symbols(main)` / `get_guide(librarian)`, and the two are byte-identical.
    /// Kept as a separate field so the caller decides, and so the ambiguity is visible in the
    /// type rather than resolved silently inside the walker.
    bare: Vec<String>,
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
        // A chunk that is EXACTLY one lowercase identifier, with nothing else in it. The
        // strictness is the point: `include_body=true` and `path=…` both carry an `=` and are
        // excluded here because [`params`] already owns them, and a chunk holding anything
        // besides the identifier is not the manual's signature form.
        let bare: Vec<String> = span
            .split(',')
            .filter_map(|chunk| {
                let t = chunk.trim();
                let mut cs = t.chars();
                let head_ok = matches!(cs.next(), Some(c) if c.is_ascii_lowercase() || c == '_');
                let rest_ok = t
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
                (head_ok && rest_ok).then(|| t.to_string())
            })
            .collect();
        out.push(LineCall {
            tool,
            params: arg.captures_iter(&span).map(|a| a[1].to_string()).collect(),
            bare,
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
    let actions = tool_actions();
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
                let named = call.params.iter().cloned().map(|p| (p, false));
                // A bare identifier that is one of this tool's declared ACTION values is the
                // `doc(get)` dispatch shorthand, not a parameter claim. Excluded here rather
                // than in the assertion so both tests see the same population.
                let bare = billable_bare(&call, &actions)
                    .into_iter()
                    .map(|b| (b, true));
                for (param, is_bare) in named.chain(bare) {
                    out.push(Cite {
                        file: rel.clone(),
                        line: i + 1,
                        tool: call.tool.clone(),
                        param,
                        bare: is_bare,
                        text: line.trim().chars().take(110).collect(),
                    });
                }
            }
        }
    }
    out
}

/// What one surface's fenced ` ```json ` blocks yielded.
///
/// The two counters are not bookkeeping. A parser that silently declines everything satisfies
/// every assertion about what it finds, so the population it *refuses* has to be as visible as
/// the population it reads — which is the same defect, one level up, that this whole file exists
/// to catch.
#[derive(Default)]
struct PayloadScan {
    /// Objects carrying both `"tool"` and `"arguments"` — the shape that is a real call.
    cites: Vec<Cite>,
    /// Fenced json blocks that parsed as JSON, payload or not.
    parsed: usize,
    /// Fenced json blocks `serde_json` refused — elisions (`…`), fragments, deliberate
    /// non-examples. Skipping them is correct; skipping them SILENTLY is not.
    unparseable: usize,
}

/// Argument keys named in the manual's JSON payload form.
///
/// **A second parser, deliberately, and the parent bug (`da911452d5a00116`) said so.**
/// [`CALL_OPEN`] anchors on `name(`, and the payload form has no paren — it uses `:` and `{`.
/// Widening `\b…\(` to reach it would mean matching bare prose, which is how the guard would
/// start reporting on English.
///
/// **This input is real JSON, which is the whole reason this is cheap.** The parent bug had to
/// invent a `<placeholder>` escape because its input was prose and a parser over prose owes one
/// (`IC-6`). A fenced ` ```json ` block either parses or it does not — no grammar to design, no
/// escape to invent, no house convention silently dictated by a scanner.
fn json_payload_cites() -> PayloadScan {
    let root = repo_root();
    let mut out = PayloadScan::default();

    for path in present_tense_surfaces() {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string();

        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0usize;
        while i < lines.len() {
            if lines[i].trim_start().starts_with("```json") {
                let open = i;
                let mut j = i + 1;
                while j < lines.len() && !lines[j].trim_start().starts_with("```") {
                    j += 1;
                }
                let block = lines[open + 1..j.min(lines.len())].join("\n");
                match serde_json::from_str::<serde_json::Value>(&block) {
                    Ok(v) => {
                        out.parsed += 1;
                        // One block may hold a single call or an array of them.
                        let items: Vec<&serde_json::Value> = match v.as_array() {
                            Some(a) => a.iter().collect(),
                            None => vec![&v],
                        };
                        for item in items {
                            let (Some(tool), Some(args)) = (
                                item.get("tool").and_then(serde_json::Value::as_str),
                                item.get("arguments").and_then(serde_json::Value::as_object),
                            ) else {
                                continue; // a config example, not a call — parsed, not a payload
                            };
                            for key in args.keys() {
                                out.cites.push(Cite {
                                    file: rel.clone(),
                                    line: open + 1,
                                    tool: tool.to_string(),
                                    param: key.clone(),
                                    bare: false,
                                    text: format!("{{\"tool\": \"{tool}\", \"arguments\": {{…}}}}"),
                                });
                            }
                        }
                    }
                    Err(_) => out.unparseable += 1,
                }
                i = j + 1;
            } else {
                i += 1;
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
            let form = if c.bare {
                format!("`{}({}` — bare, no `=`", c.tool, c.param)
            } else {
                format!("`{}({}=`", c.tool, c.param)
            };
            bad.push(format!(
                "  {}:{}\n      {} — {} has no such parameter.\n      line: {}\n      declared: {}",
                c.file,
                c.line,
                form,
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
             IF THE CITATION IS `tool(ident)` WITH NO `=`, READ THIS FIRST. The scan bills bare \
             identifiers because the manual names parameters that way 97 times, and \
             `references(name_path, path)` rotted inside that blind spot for the whole life of \
             its bug file. Two forms are excused and a third is your escape:\n  \
             - an ACTION value (`doc(get)`, `librarian(reindex)`) is dispatch shorthand and is \
             skipped, read from the schema's own `enum` — so a RETIRED action stops being excused \
             the moment it leaves the schema, which is why they are not allow-listed.\n  \
             - a real parameter is simply correct and never reaches here.\n  \
             - IF YOU MEANT A VALUE, NOT A PARAMETER NAME, WRITE `tool(<placeholder>)`. \
             `symbols(<found_file>)` reads as a value to a human and is invisible to this scan, \
             because `<` is not an identifier character. That is the escape this parser owes you, \
             and it is why there is no allowlist to add yourself to. Better still, name the \
             parameter too — `symbols(path=<found_file>)` tells the reader which slot the value \
             goes in, which the bare form never did.\n\n\
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

/// Anchored-call false positives: [`CALL_OPEN`] matched `word(` in prose that is not a tool
/// call at all — code quoted as a worked example, or an action name used as shorthand for
/// `doc(action="…")`. **Not** `ALIAS_ALLOWLIST`: that constant is for a live tool cited under
/// another name, and every entry here names no tool, live or dead. Keyed on the exact
/// `(file, line, tool)` triple, never on the bare word, so a real dead-tool call that happens
/// to share one of these words elsewhere is still caught — widening this to a bare-word
/// denylist would silently blind the guard to that case.
///
/// Surfaced 2026-09-19 when `stale_tool_call_findings` stopped skipping every single-word
/// (no-underscore) citation — see `docs/issues/2026-09-02-both-doc-citation-guards-skip-
/// half-the-corpus-without-saying-so.md` § Fix: *"decide what to do with the unrecognised
/// bucket… its size is unknown until measured."* Measured here: 4, all verified by hand.
const ANCHOR_FALSE_POSITIVES: &[(&str, usize, &str)] = &[
    // `for(i=3;i<=n;i++) if(a[i]~/^[A-Z]+$/) print a[i]` — an awk one-liner quoted verbatim
    // as a worked example. `for` is awk syntax, not a codescout tool.
    ("docs/TAXONOMY.md", 41, "for"),
    // "a test that reaches a `#[cfg(feature = \"librarian\")]` item" — a Rust attribute
    // quoted in prose, not a call.
    ("CONTRIBUTING.md", 167, "cfg"),
    // "`sorted(x, key=f)` counts" — Python's builtin, quoted as a worked example of a
    // callback shape in a probe's own doc row.
    ("docs/PROBES.md", 201, "sorted"),
    // "`find(kind=\"bug\", status=\"open\")` — the triage query" — prose shorthand for
    // `doc(action="find", …)`, using the ACTION name as if it were the tool name. No tool
    // is named "find".
    ("src/prompts/guides/tracker-conventions.md", 89, "find"),
];

/// Findings for [`a_documented_call_names_a_live_tool`], deduplicated by `(file, line, tool)` —
/// one call site is one finding, however many named arguments it carries. Filed as
/// `docs/issues/archive/2026-09-02-doc-tool-refs-counts-call-param-pairs-as-documents.md`:
/// [`anchored_cites`] emits one [`Cite`] per named argument, which is the right grain for
/// [`a_documented_tool_parameter_exists_on_that_tool`] but not for this per-tool question — do
/// NOT reuse this dedup there, its grain is already correct.
fn stale_tool_call_findings(
    cites: &[Cite],
    names: &HashSet<String>,
    allowed: &HashSet<&str>,
) -> Vec<String> {
    let mut seen: HashSet<(String, usize, String)> = HashSet::new();
    let mut bad = Vec::new();

    for c in cites {
        if c.bare {
            continue;
        }
        if names.contains(&c.tool) || allowed.contains(c.tool.as_str()) {
            continue;
        }
        if ANCHOR_FALSE_POSITIVES
            .iter()
            .any(|(file, line, tool)| *file == c.file && *line == c.line && *tool == c.tool)
        {
            continue;
        }
        if !seen.insert((c.file.clone(), c.line, c.tool.clone())) {
            continue;
        }
        bad.push(format!(
            "  {}:{}\n      `{}(…)` names no registered tool.\n      line: {}",
            c.file, c.line, c.tool, c.text
        ));
    }
    bad
}

/// An anchored call to a name no tool answers to.
///
/// Separate from the parameter test because the remedies differ: a wrong parameter is a one-token
/// edit, a retired tool name means the page describes an interface that no longer exists and
/// usually needs rewriting rather than patching.
#[test]
fn a_documented_call_names_a_live_tool() {
    let cites = anchored_cites();
    let names = tool_names();
    let allowed: HashSet<&str> = ALIAS_ALLOWLIST.iter().copied().collect();
    let checked = cites.iter().filter(|c| !c.bare).count();
    let bad = stale_tool_call_findings(&cites, &names, &allowed);

    assert!(
        bad.is_empty(),
        "{} stale call(s) name a tool that does not exist (checked {checked} of {} anchored \
         citations; a bare dispatch-shorthand citation is excluded from this check by \
         `stale_tool_call_findings` itself, which is a different exclusion from the one this \
         count would otherwise hide).\n\n{}\n\n\
         If the name is a live alias, add it to ALIAS_ALLOWLIST with the declaration site — \
         but read that constant's comment first: the last candidate for it was a gap in this \
         file's own extractor, not a real alias.",
        bad.len(),
        cites.len(),
        bad.join("\n\n")
    );
}

/// A stale call carrying multiple named arguments must be reported once, not once per argument —
/// the defect fixed alongside this test in
/// `docs/issues/archive/2026-09-02-doc-tool-refs-counts-call-param-pairs-as-documents.md`. Keyed on the
/// emitted text rather than the count alone, so a dedup that over-collapses (e.g. keying on
/// `file` alone) cannot pass this by producing a smaller but still-wrong number.
#[test]
fn a_stale_call_is_reported_once_regardless_of_argument_count() {
    let cite = |param: &str| Cite {
        file: "docs/example.md".to_string(),
        line: 335,
        tool: "artifact_event".to_string(),
        param: param.to_string(),
        bare: false,
        text: "`artifact_event(action=\"list\", artifact_id=X)`".to_string(),
    };
    let cites = vec![cite("action"), cite("artifact_id")];
    let names = tool_names();
    let allowed: HashSet<&str> = ALIAS_ALLOWLIST.iter().copied().collect();

    let bad = stale_tool_call_findings(&cites, &names, &allowed);

    assert_eq!(
        bad.len(),
        1,
        "one call carrying two named arguments must produce one finding, not one per argument: {bad:?}"
    );
    assert!(
        bad[0].contains("artifact_event"),
        "the one finding must still name the offending tool: {bad:?}"
    );
}

/// A retired tool name with no underscore (`artifact`, since renamed to `doc`) must be
/// reported as stale, not silently skipped by a name-shape heuristic.
///
/// Filed as `docs/issues/2026-09-02-both-doc-citation-guards-skip-half-the-corpus-without-
/// saying-so.md`: the old guard's `!c.tool.contains('_')` bailed out before checking the
/// registry at all, so every single-word tool citation — live or dead — went unexamined.
/// `artifact` is a real fixture, not a synthetic one: it is the tool this file's own `doc`
/// replaced, so it is guaranteed retired rather than merely assumed to be.
#[test]
fn a_retired_single_word_tool_name_is_reported_not_skipped() {
    let names = tool_names();
    assert!(
        !names.contains("artifact"),
        "fixture assumption broken: \"artifact\" is registered as a live tool again — \
         pick a different retired single-word name for this fixture"
    );

    let cite = Cite {
        file: "docs/example.md".to_string(),
        line: 42,
        tool: "artifact".to_string(),
        param: "action".to_string(),
        bare: false,
        text: "`artifact(action=\"get\")`".to_string(),
    };
    let allowed: HashSet<&str> = ALIAS_ALLOWLIST.iter().copied().collect();

    let bad = stale_tool_call_findings(&[cite], &names, &allowed);

    assert_eq!(
        bad.len(),
        1,
        "a single-word tool name with no underscore must still be checked against the \
         registry, not skipped outright: {bad:?}"
    );
    assert!(
        bad[0].contains("artifact"),
        "the finding must name the offending tool: {bad:?}"
    );
}

/// `ANCHOR_FALSE_POSITIVES` is keyed on `(file, line, tool)`, not on the bare word — the same
/// word at a different citation site must still be caught. Guards against the exclusion
/// silently widening from "this exact prose example" to "any mention of `for`".
#[test]
fn the_anchor_false_positive_list_is_keyed_on_the_citation_site_not_the_word() {
    let names = tool_names();
    let allowed: HashSet<&str> = ALIAS_ALLOWLIST.iter().copied().collect();

    let cite = Cite {
        file: "docs/some-other-file.md".to_string(),
        line: 999,
        tool: "for".to_string(),
        param: String::new(),
        bare: false,
        text: "`for(…)` pretending to be a tool at a different site".to_string(),
    };
    let bad = stale_tool_call_findings(&[cite], &names, &allowed);

    assert_eq!(
        bad.len(),
        1,
        "the same word at a citation site NOT in ANCHOR_FALSE_POSITIVES must still be \
         reported: {bad:?}"
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

/// The manual's signature form names parameters, and the scan bills them.
///
/// The founding case: `references(name_path, path)` sat in three files for the whole life of
/// `43888bfe2327cbda` while this suite was green, because [`NAMED_ARG`] requires an `=` and the
/// signature form has none. Measured 2026-09-12 at `408709ea`: **97** bare identifiers in the
/// present-tense corpus are real parameters written this way — every one of them unguarded
/// until this landed, and any of them free to rot exactly as `name_path` did.
#[test]
fn the_signature_form_is_billed_as_a_parameter_claim() {
    let sig = calls_on_line("**`references(name_path, path)`**");
    assert_eq!(
        sig[0].bare,
        vec!["name_path", "path"],
        "both identifiers are claims, not just the first — an earlier draft of this scan read \
         only the leading argument and reported green over a live violation in the manual's own \
         Recommended Workflow table"
    );
    assert!(
        sig[0].params.is_empty(),
        "and they stay OUT of `params`, which means `key=`: a caller told their \
         `references(name_path=` is wrong goes looking for a string the page does not contain"
    );

    // The MIXED form, which is the commoner shape and the one a params-only scan reports on
    // while being silently partial about it.
    let mixed = calls_on_line("`edit_code(symbol, path, action=\"rename\", new_name)`");
    assert_eq!(mixed[0].params, vec!["action"], "the `=` argument");
    assert_eq!(
        mixed[0].bare,
        vec!["symbol", "path", "new_name"],
        "and the three the guard used to drop while reporting on the same line"
    );
}

/// Action-dispatch shorthand is not a parameter claim, and the exclusion is schema-derived.
#[test]
fn an_action_dispatch_value_is_not_billed_as_a_parameter() {
    let actions = tool_actions();
    assert!(
        actions.get("doc").is_some_and(|a| a.contains("get")),
        "control: the enum is actually being read. If this map is empty the whole exclusion \
         below is vacuous and every assertion in it passes by finding nothing"
    );

    for line in [
        "`doc(get)`",
        "`librarian(reindex)`",
        "`workspace(activate)`",
        "`memory(recall)`",
    ] {
        let c = &calls_on_line(line)[0];
        assert!(!c.bare.is_empty(), "the walker still SEES it: {line}");
        assert!(
            billable_bare(c, &actions).is_empty(),
            "{line} is dispatch shorthand and must not be billed"
        );
    }

    // Over-match guard, and the reason the exclusion is read from `enum` rather than
    // allow-listed: a bare identifier on the SAME tool that is not a declared action is still a
    // claim. A rule that simply excused every bare identifier on an action-bearing tool would
    // satisfy the four assertions above and catch nothing.
    let typo = &calls_on_line("`doc(hedaing)`")[0];
    assert_eq!(
        billable_bare(typo, &actions),
        vec!["hedaing"],
        "a non-action bare identifier on an action-bearing tool is still billed"
    );
}

/// `tool(<placeholder>)` is the escape, and it is the one this parser owes.
///
/// `IC-6`: a parser over a namespace must give the author a way to write a token that is not a
/// claim. Without it the only options are an allowlist — which silences real defects at the same
/// sites — or a corpus quietly edited to dodge the scanner with nothing recording that it had to
/// be, which this file's own history already did once.
#[test]
fn an_angle_bracket_placeholder_is_the_documented_escape() {
    let escaped = calls_on_line("`symbols(<found_file>)` — a VALUE, not a parameter name");
    assert!(
        escaped[0].bare.is_empty(),
        "`<` is not an identifier character, so the placeholder is not a claim: {:?}",
        escaped[0].bare
    );

    // Paired with its opposite direction. The assertion above is monotone under the scan going
    // dead — a walker that bills nothing at all satisfies it — so it is worth nothing alone.
    let unescaped = calls_on_line("`symbols(found_file)`");
    assert_eq!(
        unescaped[0].bare,
        vec!["found_file"],
        "the SAME site without the brackets is billed; that contrast is the whole test"
    );

    // And the form the corpus was rewritten to, which is strictly better documentation: it names
    // the slot the value goes in, which the bare form never did.
    let named = calls_on_line("`symbols(path=<found_file>)`");
    assert_eq!(named[0].params, vec!["path"], "the parameter IS checked");
    assert!(named[0].bare.is_empty(), "the value is not");
}

/// A JSON payload example names arguments its tool actually declares.
///
/// The second of the two forms `docs/manual` teaches, and the one
/// `a_documented_tool_parameter_exists_on_that_tool` cannot see: [`CALL_OPEN`] anchors on
/// `name(`, and `{"tool": "symbols", "arguments": {…}}` has no paren anywhere. Measured
/// 2026-09-13 at `d779fbd3`: **89 argument claims across 191 parsed blocks**, none of them under
/// any guard until this landed.
///
/// This is the surface a reader is most likely to COPY — it is a ready-made call — and codescout
/// drops unknown keys rather than rejecting them (`IC-15`), so a wrong key here produces a
/// successful call that quietly does something else.
#[test]
fn a_documented_json_payload_names_real_parameters() {
    let scan = json_payload_cites();
    let schemas = tool_params();

    // Non-vacuity FIRST, because every assertion below is satisfied by a parser that reads
    // nothing. Floors, not equalities: a doc edit should not red this, a parser that stopped
    // working should. Derivation — measured 2026-09-13 at `d779fbd3`: 191 parsed, 17 declined,
    // 89 cites. The floors sit ~20% under each so ordinary churn has room.
    assert!(
        scan.parsed >= 150,
        "only {} fenced json blocks parsed (was 191 on 2026-09-13). Either the corpus shrank a \
         lot or the block scanner stopped finding fences — check the ```json detection before \
         lowering this.",
        scan.parsed
    );
    assert!(
        scan.cites.len() >= 70,
        "only {} argument claims found (was 89 on 2026-09-13). Blocks are parsing, so this is \
         the `tool`+`arguments` shape recognition, not the fence scan.",
        scan.cites.len()
    );
    // The population the parser DECLINES, asserted rather than absorbed. Elisions and fragments
    // are legitimately unparseable; a sudden majority means the scanner is mis-slicing blocks.
    assert!(
        scan.unparseable * 4 < scan.parsed,
        "{} of {} fenced json blocks failed to parse — more than a quarter. That is a scanner \
         fault, not a corpus of elided examples (17 of 191 on 2026-09-13).",
        scan.unparseable,
        scan.parsed
    );

    let mut bad: Vec<String> = Vec::new();
    for c in &scan.cites {
        let Some(params) = schemas.get(&c.tool) else {
            continue; // not a tool — `a_documented_call_names_a_live_tool`'s business
        };
        if !params.contains(&c.param) {
            let mut known: Vec<&str> = params.iter().map(String::as_str).collect();
            known.sort_unstable();
            bad.push(format!(
                "  {}:{}\n      \"arguments\": {{ \"{}\": … }} — {} has no such parameter.\n      \
                 declared: {}",
                c.file,
                c.line,
                c.param,
                c.tool,
                known.join(", ")
            ));
        }
    }

    assert!(
        bad.is_empty(),
        "{} JSON payload example(s) name an argument their tool does not declare.\n\n{}\n\n\
         This form is checked by a SECOND parser, not by the prose scan — `CALL_OPEN` requires a \
         `(` and a payload has none. Two consequences worth knowing before you debug a surprise \
         here:\n  \
         - the input is real JSON, so there is no grammar to fight and no escape to invent. If a \
         block should not be scanned, it is not valid JSON or it lacks a `tool`/`arguments` \
         pair — do not reach for a marker.\n  \
         - the declared set above includes `workspace` for pinnable tools, because \
         `list_tools` INJECTS it between `input_schema()` and the wire. If a parameter you know \
         is real is reported missing, suspect that list: an extractor reading only the source \
         schema is narrower than the tool it checks, which is exactly the defect that surfaced \
         when this scan first ran.",
        bad.len(),
        bad.join("\n\n")
    );
}
