//! The routes `read_file` names carry a file's path, and a route is text a caller copies and runs.
//!
//! Two shapes exist. `read_file("<path>", json_path="…", start_line=N, end_line=M)` holds the path
//! inside a JSON string, so a `"`, a `\` or a line break in it must be JSON-escaped.
//! `run_command("grep -o 'PATTERN[^,]*' <path>")` holds it inside a shell command that is itself
//! inside a JSON string, so it must be shell-quoted and then JSON-escaped; a name no one-line
//! command can carry (a line break) gets no route at all.
//!
//! Every test here parses the text a tool printed back into its arguments with a real JSON parser
//! and checks the path against the file's, byte for byte. The tests in [`unix_only`] go further
//! and run it: a `read_file(...)` is executed through `ReadFile`; a `run_command(...)` is executed
//! under the real `sh` (and `dash` when this machine has it: Ubuntu CI's `/bin/sh` is dash) and
//! through the real `RunCommand` tool, against a real file of that name. They need file names
//! NTFS forbids (`"`, `*`, `?`, `<`, `>`, `|`, and `\` is a separator) and a POSIX shell, so they
//! are `cfg(unix)`; the tests outside [`unix_only`] build no file and spawn no shell.

use super::{grep_o_route, line_hint, over_budget_line_hint};
use serde_json::{json, Value};

/// The arguments of a parsed call: `(key, value)` in order, the key `None` for the positional one.
type Args = Vec<(Option<String>, Value)>;

/// One call as written in a route: `name(<arg>, key=<arg>, …)`, every argument a JSON value (a
/// string, an integer, `true`). The first argument may be positional. Parsed with a real JSON
/// parser, the way a caller who pastes the route would have it read. Returns the name, the
/// arguments in order, and the text after the closing parenthesis.
fn parse_call(text: &str) -> (String, Args, &str) {
    let open = text
        .find('(')
        .unwrap_or_else(|| panic!("not a call: {text:.200}"));
    let name = text[..open].trim().to_string();
    let mut rest = &text[open + 1..];
    let mut args = Vec::new();
    loop {
        rest = rest.trim_start();
        let ident = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let key = if ident > 0 && rest[ident..].starts_with('=') {
            let k = rest[..ident].to_string();
            rest = &rest[ident + 1..];
            Some(k)
        } else {
            None
        };
        // A string is self-delimiting, so the stream parser reads exactly one. A number or `true`
        // is not (`187,` is "trailing characters" to it): it ends at the next `,` or `)`.
        let (value, used) = if rest.starts_with('"') {
            let mut values = serde_json::Deserializer::from_str(rest).into_iter::<Value>();
            let v = values
                .next()
                .unwrap_or_else(|| panic!("{name}: an argument is missing in {text:.300}"))
                .unwrap_or_else(|e| panic!("{name}: argument is not JSON ({e}) in {text:.300}"));
            (v, values.byte_offset())
        } else {
            let end = rest.find([',', ')']).unwrap_or(rest.len());
            let v = serde_json::from_str(rest[..end].trim()).unwrap_or_else(|e| {
                panic!(
                    "{name}: argument {:?} is not JSON ({e}) in {text:.300}",
                    &rest[..end]
                )
            });
            (v, end)
        };
        rest = &rest[used..];
        args.push((key, value));
        rest = rest.trim_start();
        if let Some(r) = rest.strip_prefix(',') {
            rest = r;
            continue;
        }
        let Some(after) = rest.strip_prefix(')') else {
            panic!("{name}: the call is not closed at {rest:.100} in {text:.300}");
        };
        return (name, args, after);
    }
}

/// The call that starts at `prefix` (`"read_file("`) in a hint or an error: a route named inside
/// a sentence.
fn call_in<'a>(text: &'a str, prefix: &str) -> Option<(Args, &'a str)> {
    let at = text.find(prefix)?;
    let (_, args, rest) = parse_call(&text[at..]);
    Some((args, rest))
}

/// Every `read_file(...)` call named in `text`, in order, with the `N`/`M` placeholders of a
/// generic note ("read the file in ranges with read_file(path=…, start_line=N, end_line=M)")
/// filled with `1`/`3`.
fn read_file_calls_in(text: &str) -> Vec<Args> {
    let text = text.replace("start_line=N, end_line=M", "start_line=1, end_line=3");
    let (mut calls, mut rest) = (Vec::new(), text.as_str());
    while let Some((args, after)) = call_in(rest, "read_file(") {
        calls.push(args);
        rest = after;
    }
    calls
}

/// The `path` a parsed `read_file` call names: positional, or `path=`.
fn path_of(args: &Args) -> &Value {
    let (key, value) = &args[0];
    assert!(
        matches!(key.as_deref(), None | Some("path")),
        "the first argument is not the path: {args:?}"
    );
    value
}

/// The clamp advisory of a buffer read quotes whatever it is handed in both of its routes, and
/// names no shell route for a name no one-line command can carry.
#[test]
fn the_clamp_advisory_quotes_its_target_in_both_routes() {
    let target = "@tool_a\"b\\c d";
    let hint = line_hint(target, Some("$.k"));
    let (args, _) = call_in(&hint, "read_file(").expect("the field route");
    assert_eq!(args[0], (None, json!(target)), "{hint}");
    assert_eq!(args[1], (Some("json_path".into()), json!("$.k")), "{hint}");
    // A field the caller's payload could only have through a bracketed key is quoted too.
    let hint = line_hint(target, Some("$.q\"k\\x"));
    let (args, _) = call_in(&hint, "read_file(").expect("the field route");
    assert_eq!(
        args[1],
        (Some("json_path".into()), json!("$.q\"k\\x")),
        "{hint}"
    );
    let hint = line_hint(target, Some("$.k"));
    let (args, _) = call_in(&hint, "run_command(").expect("the grep route");
    assert_eq!(
        args[0].1,
        json!("grep -o 'PATTERN[^,]*' '@tool_a\"b\\c d'"),
        "{hint}"
    );

    // Literal expectations: the helper under test must not build its own oracle.
    for (name, command) in [
        (
            "@file_a\"b\\c d",
            r#"grep -o 'PATTERN[^,]*' '@file_a"b\c d'"#,
        ),
        (
            "sp ace/q\"uote.json",
            r#"grep -o 'PATTERN[^,]*' 'sp ace/q"uote.json'"#,
        ),
        ("it's", r#"grep -o 'PATTERN[^,]*' 'it'\''s'"#),
    ] {
        let hint = over_budget_line_hint(name);
        let (args, _) = call_in(&hint, "run_command(").expect("the grep route");
        assert_eq!(args[0].1, json!(command), "{hint}");
    }

    for hint in [
        line_hint("@tool_a\nb", Some("$.k")),
        line_hint("@tool_a\nb", None),
        over_budget_line_hint("@file_a\nb"),
    ] {
        assert!(
            !hint.contains("run_command("),
            "a name with a line break must get no shell route: {hint}"
        );
        assert!(hint.contains("wider than the inline budget"), "{hint}");
    }
}

/// `grep_o_route` on what needs no file: no route for a line break, a buffer ref unchanged, and
/// exact commands for a few names.
#[test]
fn grep_o_route_names_no_route_for_a_line_break_and_leaves_a_handle_alone() {
    assert_eq!(grep_o_route("a\nb"), None);
    assert_eq!(grep_o_route("a\rb"), None);
    assert_eq!(
        grep_o_route("@cmd_0123abcd").as_deref(),
        Some(r#"run_command("grep -o 'PATTERN[^,]*' @cmd_0123abcd")"#)
    );
    assert_eq!(
        grep_o_route("@cmd_0123abcd.err").as_deref(),
        Some(r#"run_command("grep -o 'PATTERN[^,]*' @cmd_0123abcd.err")"#)
    );
    assert_eq!(
        grep_o_route("-r").as_deref(),
        Some(r#"run_command("grep -o 'PATTERN[^,]*' ./-r")"#)
    );
    assert_eq!(
        grep_o_route("sp ace/q\"uote").as_deref(),
        Some(r#"run_command("grep -o 'PATTERN[^,]*' 'sp ace/q\"uote'")"#)
    );
}

/// `run_command` swaps a handle-shaped token for the buffer's content BEFORE the shell parses
/// anything, so quotes do not hide one inside a file name. Every token in a name is broken by an
/// empty `''` between the `@` and the kind, which the shell reads as the same word.
#[test]
fn a_handle_shaped_token_inside_a_name_is_broken_so_the_resolver_does_not_see_it() {
    for (name, command) in [
        (
            "x @cmd_deadbeef y.json",
            r#"grep -o 'PATTERN[^,]*' 'x @''cmd_deadbeef y.json'"#,
        ),
        (
            "dir/@file_0123abcd.json",
            r#"grep -o 'PATTERN[^,]*' dir/@''file_0123abcd.json"#,
        ),
        (
            "z@bg_deadbeef.err w",
            r#"grep -o 'PATTERN[^,]*' 'z@''bg_deadbeef.err w'"#,
        ),
        (
            "@tool_00000000 @ack_0123abcd",
            r#"grep -o 'PATTERN[^,]*' '@''tool_00000000 @''ack_0123abcd'"#,
        ),
        // A handle at the END of a longer name is not that handle: only a whole-target handle
        // is left for the resolver.
        (
            "dir/@file_0123abcd",
            r#"grep -o 'PATTERN[^,]*' dir/@''file_0123abcd"#,
        ),
        // Longer than a handle: not a handle, and the resolver matches its 8-hex prefix.
        (
            "@cmd_deadbeefXX",
            r#"grep -o 'PATTERN[^,]*' @''cmd_deadbeefXX"#,
        ),
    ] {
        let route = grep_o_route(name).unwrap_or_else(|| panic!("{name:?}: no route"));
        let (args, _) = call_in(&route, "run_command(").unwrap();
        assert_eq!(args[0].1, json!(command), "{name:?}");
        let token = regex::Regex::new(r"@(?:cmd|file|tool|bg|ack)_[0-9a-f]{8}").unwrap();
        assert!(
            !token.is_match(command),
            "{name:?}: a handle-shaped token is left in {command}"
        );
    }
}

/// The cut notes of a file summary name the file in a `read_file(path=…)` call. Every caller but
/// one hands `fit_envelope` a buffer handle; `read_markdown_single_heading` (heading not found)
/// hands it the REAL path. So each of the four builders is given a path with a quote, a space
/// and a backslash, and the call it prints must parse back to exactly that path.
#[test]
fn the_cut_notes_of_a_summary_quote_the_file_they_name() {
    use crate::tools::file_summary::{bound_summary, cut_array_middle};
    let id = "/tmp/q\"uo te\\x.md";

    // A wide string is middle-elided; its marker carries the route to the rest.
    let summary = json!({ "type": "x", "head": "h".repeat(5_000) });
    let (cut, notes) = bound_summary(summary, id, 1_000);
    assert!(notes.is_empty(), "{notes:?}");
    let head = cut["head"].as_str().unwrap();
    let calls = read_file_calls_in(head);
    assert_eq!(calls.len(), 1, "{head}");
    assert_eq!(path_of(&calls[0]), &json!(id), "{head}");

    // Nothing to cut: the summary is replaced by the minimal one, and the note names the file.
    let summary = json!({ "type": "x", "meta": { "pad": "p".repeat(7_000) } });
    let (cut, notes) = bound_summary(summary, id, 100);
    assert_eq!(cut["summary_omitted"], json!(true));
    assert_eq!(notes.len(), 1, "{notes:?}");
    let calls = read_file_calls_in(&notes[0]);
    assert_eq!(calls.len(), 1, "{}", notes[0]);
    assert_eq!(path_of(&calls[0]), &json!(id), "{}", notes[0]);

    // An array cut whose omitted entries have lines: an exact range. Without: the generic one.
    for with_lines in [true, false] {
        let entries: Vec<Value> = (1..=100)
            .map(|i| {
                if with_lines {
                    json!({ "name": format!("sym{i:03}"), "line": i })
                } else {
                    json!({ "name": format!("sym{i:03}") })
                }
            })
            .collect();
        let mut summary = json!({ "type": "x", "a": [] });
        let note =
            cut_array_middle(&mut summary, "", "a", entries, 300, id).expect("the array was cut");
        let calls = read_file_calls_in(&note);
        assert_eq!(calls.len(), 1, "{note}");
        assert_eq!(path_of(&calls[0]), &json!(id), "{note}");
        let exact = calls[0]
            .iter()
            .any(|(k, _)| k.as_deref() == Some("start_line"));
        assert!(exact, "{note}");
        assert_eq!(
            note.contains("lines "),
            with_lines,
            "with_lines={with_lines}: {note}"
        );
    }

    // A handle is a plain word: its note is the one it always was.
    let mut summary = json!({ "type": "x", "a": [] });
    let entries: Vec<Value> = (1..=100)
        .map(|i| json!({ "name": format!("sym{i:03}"), "line": i }))
        .collect();
    let note = cut_array_middle(&mut summary, "", "a", entries, 300, "@file_89abcdef").unwrap();
    assert!(
        note.contains(r#"read_file(path="@file_89abcdef", start_line="#),
        "{note}"
    );
}

/// Everything that needs a file whose name NTFS forbids, a POSIX shell, or both.
#[cfg(unix)]
mod unix_only {
    use super::*;
    use crate::agent::Agent;
    use crate::lsp::LspManager;
    use crate::tools::read_file::ReadFile;
    use crate::tools::Tool;
    use std::path::Path;
    use std::sync::Arc;

    /// File-name stems a path can carry that break a route written raw. Every one is a legal Linux
    /// file name: a space, `"`, `'`, `\`, `$`, a backtick, glob, brace and operator characters, a
    /// leading `-`, a tab, a line break, handle-shaped tokens, and non-ASCII text.
    const HAZARDS: [&str; 18] = [
        "sp ace",
        "q\"uote",
        "it's",
        "back\\slash",
        "dollar $HOME $(echo pwned) ${x}",
        "back`tick`",
        "ünï€😀 日本",
        "glob *?[a]",
        "semi;amp&pipe|gt>lt<",
        "-dash",
        "tab\there",
        "a\"b\\c 'd' $e `f` ü",
        "line\nbreak",
        "e\\\"\\\\\"",
        // `run_command` resolves these before the shell parses anything.
        "x @cmd_deadbeef y",
        "@file_0123abcd",
        "z@bg_deadbeef.err w",
        "braces {a,b} x{1..3}y",
    ];

    /// A stem no one-line shell command can carry.
    fn has_line_break(stem: &str) -> bool {
        stem.contains('\n')
    }

    async fn project_ctx(root: &Path) -> crate::tools::ToolContext {
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        crate::tools::ToolContext {
            agent: Agent::new(Some(root.to_path_buf())).await.unwrap(),
            lsp: LspManager::new_arc(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(50)),
            progress: None,
            peer: None,
            section_coverage: Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: Arc::new(parking_lot::Mutex::new(
                crate::tools::guide_ledger::GuideLedger::mid_session(),
            )),
            workspace_override: None,
        }
    }

    /// Write `body` to `dir/name` and return the path as the caller would name it.
    fn write(dir: &Path, name: &str, body: String) -> String {
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap_or_else(|e| panic!("fixture: cannot write {name:?}: {e}"));
        p.to_str().unwrap().to_string()
    }

    /// 600 rows of 50 B: far over one page.
    fn rows() -> Vec<String> {
        (1..=600)
            .map(|i| format!("row {i:04} {}", "v".repeat(40)))
            .collect()
    }

    /// Run `command` under `sh` and, when this machine has it, `dash` (Ubuntu CI's `/bin/sh`).
    /// `sh` MUST run: a missing `sh` is a failure, not a skip, so no test here passes for want of a
    /// shell. Only `dash` may be absent, and then it says so.
    fn run_shells(command: &str, cwd: &Path) -> Vec<(&'static str, std::process::Output)> {
        let mut ran = Vec::new();
        for shell in ["sh", "dash"] {
            match std::process::Command::new(shell)
                .arg("-c")
                .arg(command)
                .current_dir(cwd)
                .output()
            {
                Ok(out) => ran.push((shell, out)),
                Err(e) if shell == "dash" => eprintln!("shell dash not run: {e}"),
                Err(e) => panic!("sh could not be started: {e}"),
            }
        }
        assert!(
            ran.iter().any(|(s, _)| *s == "sh"),
            "sh did not run, so nothing was checked"
        );
        ran
    }

    /// The arguments of a parsed `read_file` call as the tool's input: the positional one is
    /// `path`.
    fn input_of(args: Args) -> Value {
        let mut m = serde_json::Map::new();
        for (k, v) in args {
            m.insert(k.unwrap_or_else(|| "path".into()), v);
        }
        Value::Object(m)
    }

    /// Follow `first` page by page until there is no `next`, EXECUTING each `next` as written: it
    /// must be exactly one `read_file(...)` call whose positional argument equals `path` byte for
    /// byte. Every page must fit the inline limit (a path's escapes are part of the page's
    /// skeleton). Returns the lines of every page and every `next` seen.
    async fn follow(
        ctx: &crate::tools::ToolContext,
        first: Value,
        path: &str,
        label: &str,
    ) -> (Vec<String>, Vec<String>) {
        let (mut input, mut lines, mut nexts) = (first, Vec::new(), Vec::new());
        for _ in 0..500 {
            let v = ReadFile
                .call(input.clone(), ctx)
                .await
                .unwrap_or_else(|e| panic!("{label}: {input:.300} failed: {e}"));
            assert!(
                crate::tools::response_fits(&v),
                "{label}: a page is over the inline limit and would be buffered again: {:.200}",
                v.to_string()
            );
            lines.extend(
                v["content"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{label}: no content in {v:.300}"))
                    .lines()
                    .map(str::to_string),
            );
            let Some(next) = v["next"].as_str() else {
                return (lines, nexts);
            };
            nexts.push(next.to_string());
            let (name, args, rest) = parse_call(next);
            assert_eq!(name, "read_file", "{label}: {next:.300}");
            assert_eq!(rest, "", "{label}: text after the call in {next:.300}");
            assert_eq!(args[0].0, None, "{label}: the path is positional");
            assert_eq!(
                args[0].1,
                json!(path),
                "{label}: the path in next does not parse back to the file's path: {next:.300}"
            );
            input = input_of(args);
        }
        panic!("{label}: the next chain does not end");
    }

    /// A `json_path` or `toml_key` value over one page: its `next` is a call whose path parses back
    /// to the file's, and following it reads the whole value.
    #[tokio::test]
    async fn a_value_next_of_an_awkward_path_is_a_call_that_parses_back_and_runs() {
        for stem in HAZARDS {
            let dir = tempfile::tempdir().unwrap();
            let ctx = project_ctx(dir.path()).await;
            let rows = rows();
            let joined = rows.join("\n");
            let json_path = write(
                dir.path(),
                &format!("{stem}.json"),
                serde_json::to_string_pretty(&json!({ "k": joined, "other": 1 })).unwrap(),
            );
            let table: String = rows
                .iter()
                .enumerate()
                .map(|(i, r)| format!("r{i:04} = \"{r}\"\n"))
                .collect();
            let toml_text = format!("[pre]\nx = 1\n\n[k]\n{table}\n[other]\nx = 1\n");
            let toml_path = write(dir.path(), &format!("{stem}.toml"), toml_text.clone());
            let toml_value = crate::tools::file_summary::extract_toml_key(&toml_text, "k")
                .unwrap()
                .content;

            let label = format!("json_path of {stem:?}");
            let first = json!({ "path": json_path, "json_path": "$.k" });
            let (lines, nexts) = follow(&ctx, first, &json_path, &label).await;
            assert!(!nexts.is_empty(), "{label}: the value was never paged");
            assert!(
                nexts[0].starts_with("read_file("),
                "{label}: next is not a route: {:.200}",
                nexts[0]
            );
            assert_eq!(lines, rows, "{label}: the pages are not the value");

            let label = format!("toml_key of {stem:?}");
            let first = json!({ "path": toml_path, "toml_key": "k" });
            let (lines, nexts) = follow(&ctx, first, &toml_path, &label).await;
            assert!(!nexts.is_empty(), "{label}: the value was never paged");
            assert_eq!(
                lines,
                toml_value.lines().map(str::to_string).collect::<Vec<_>>(),
                "{label}: the pages are not the value"
            );
        }
    }

    /// A line range over one page of a real file (text, source and markdown arms): its `next` is
    /// a call whose path parses back to the file's, and following it reads every row.
    #[tokio::test]
    async fn a_line_range_next_of_an_awkward_path_is_a_call_that_parses_back_and_runs() {
        for stem in HAZARDS {
            for ext in ["txt", "rs", "md"] {
                let dir = tempfile::tempdir().unwrap();
                let ctx = project_ctx(dir.path()).await;
                let rows = rows();
                let path = write(dir.path(), &format!("{stem}.{ext}"), rows.join("\n") + "\n");
                let label = format!("range of {stem:?}.{ext}");
                let first = json!({ "path": path, "start_line": 1, "end_line": 600 });
                let (lines, nexts) = follow(&ctx, first, &path, &label).await;
                assert!(!nexts.is_empty(), "{label}: the range was never paged");
                assert_eq!(lines, rows, "{label}: the pages are not the rows");
            }
        }
    }

    /// The escapes of a path are part of the page's skeleton. A route that priced the page with
    /// the raw path would leave it room for bytes the real `next` then needs, and the page would
    /// be buffered again by `call_content`. The skeleton also carries a `hint` and
    /// `line_truncated` that a page without a cut line does not, about 330 B of slack, so the
    /// escapes must outweigh that: each of 240 quotes costs 2 more serialized bytes in the
    /// literal than raw, 480 B in all; 3,000 rows of 6 B pack a page to within a few bytes of the
    /// limit.
    #[tokio::test]
    async fn a_path_full_of_escapes_still_leaves_every_page_inside_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = project_ctx(dir.path()).await;
        let rows: Vec<String> = (0..3_000).map(|i| format!("r{i:04}")).collect();
        for (ext, stem) in [
            ("txt", "\"".repeat(240)),
            ("md", "\"".repeat(240)),
            ("txt", "\\".repeat(240)),
        ] {
            let path = write(dir.path(), &format!("{stem}.{ext}"), rows.join("\n") + "\n");
            let label = format!("{} escapes in .{ext}", stem.len());
            let first = json!({ "path": path, "start_line": 1, "end_line": 3_000 });
            let (lines, nexts) = follow(&ctx, first, &path, &label).await;
            assert!(
                nexts.len() >= 2,
                "{label}: fixture: paged {} times",
                nexts.len()
            );
            assert_eq!(lines, rows, "{label}");
        }
    }

    /// A heading that is not in a long markdown file lists every heading, cut in the middle by
    /// `fit_envelope`, and the cut note names the file in a `read_file(path=…)` call. This is the
    /// one caller that hands the cut the REAL path instead of a handle. Every call printed must
    /// parse back to the path and run.
    #[tokio::test]
    async fn a_heading_not_found_in_a_long_file_prints_routes_that_parse_back_and_run() {
        for stem in [
            "q\"uo te",
            "a\"b\\c 'd' $e `f` ü",
            "line\nbreak",
            "x @cmd_deadbeef y",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let ctx = project_ctx(dir.path()).await;
            let body: String = (1..=2_000)
                .map(|i| format!("## heading {i:04}\n\nbody {i}\n\n"))
                .collect();
            let path = write(dir.path(), &format!("{stem}.md"), body);
            let v = ReadFile
                .call(json!({ "path": path, "heading": "## nope-not-here" }), &ctx)
                .await
                .unwrap_or_else(|e| panic!("{stem:?}: {e}"));
            assert_eq!(v["ok"], json!(false), "{stem:?}: {:.300}", v.to_string());
            let hint = v["hint"].as_str().expect("the refusal carries a hint");
            let calls = read_file_calls_in(hint);
            assert!(
                !calls.is_empty(),
                "{stem:?}: the hint names no route: {hint}"
            );
            for args in calls {
                assert_eq!(path_of(&args), &json!(path), "{stem:?}: {hint}");
                ReadFile
                    .call(input_of(args), &ctx)
                    .await
                    .unwrap_or_else(|e| panic!("{stem:?}: a printed route failed: {e}: {hint}"));
            }
        }
    }

    /// A single line of a real file's value wider than the inline budget is shown clipped, and
    /// the hint names `run_command("grep -o 'PATTERN[^,]*' <path>")`. Parsed back, run under the
    /// real shell(s) and the real `RunCommand` against the real file, it must print the match and
    /// nothing else; with a line break in the name there is no such route.
    #[tokio::test]
    async fn the_wide_line_hint_of_an_awkward_path_is_a_command_that_prints_the_match() {
        let value = format!(
            "{}needle-token-4242,AFTER-SENTINEL,{}",
            "filler-record, ".repeat(1_300),
            "filler-record, ".repeat(1_300)
        );
        for stem in HAZARDS {
            let dir = tempfile::tempdir().unwrap();
            let elsewhere = tempfile::tempdir().unwrap();
            let ctx = project_ctx(dir.path()).await;
            let path = write(
                dir.path(),
                &format!("{stem}.json"),
                json!({ "k": value }).to_string(),
            );
            let v = ReadFile
                .call(json!({ "path": path, "json_path": "$.k" }), &ctx)
                .await
                .unwrap_or_else(|e| panic!("{stem:?}: {e}"));
            assert_eq!(
                v["line_truncated"],
                json!(true),
                "{stem:?}: fixture: the line must be clamped: {:.200}",
                v.to_string()
            );
            let hint = v["hint"].as_str().expect("a clamped line carries a hint");
            if has_line_break(stem) {
                assert!(
                    !hint.contains("run_command(") && hint.contains("No shell route"),
                    "{stem:?}: a name no one-line command can carry must get no route: {hint}"
                );
                continue;
            }
            let (args, _) = call_in(hint, "run_command(")
                .unwrap_or_else(|| panic!("{stem:?}: the hint names no route: {hint}"));
            assert_eq!(args.len(), 1, "{stem:?}: {args:?}");
            let command = args[0].1.as_str().expect("the command is a string");
            assert!(
                command.starts_with("grep -o 'PATTERN[^,]*' "),
                "{stem:?}: {command}"
            );
            let command = command.replace("PATTERN", "needle-token");

            for (shell, out) in run_shells(&command, elsewhere.path()) {
                assert_eq!(
                    String::from_utf8_lossy(&out.stdout),
                    "needle-token-4242\n",
                    "{shell}: {stem:?}: {command} said {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }

            let ran = crate::tools::run_command::RunCommand
                .call(json!({ "command": command }), &ctx)
                .await
                .unwrap_or_else(|e| panic!("{stem:?}: run_command refused {command}: {e}"));
            let shown = ran.to_string();
            assert!(
                shown.contains("needle-token-4242") && !shown.contains("AFTER-SENTINEL"),
                "{stem:?}: run_command {command} printed: {shown:.400}"
            );
        }
    }

    /// `grep_o_route` itself, on names as they are given relative to the working directory: one
    /// argument that names the file, and a leading `-` does not turn the file into an option.
    #[test]
    fn grep_o_route_is_one_argument_that_names_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut names: Vec<String> = HAZARDS
            .iter()
            .filter(|s| !has_line_break(s))
            .map(|s| s.to_string())
            .collect();
        names.push("-r".to_string());
        names.push("~".to_string());
        names.push("-dash and space".to_string());
        for name in names {
            std::fs::write(dir.path().join(&name), "xx needle-1,yy\n").unwrap();
            for target in [
                name.clone(),
                dir.path().join(&name).to_str().unwrap().to_string(),
            ] {
                let route = grep_o_route(&target).unwrap_or_else(|| panic!("{target:?}: no route"));
                let (_, args, rest) = parse_call(&route);
                assert_eq!(rest, "", "{target:?}");
                let command = args[0].1.as_str().unwrap().replace("PATTERN", "needle");
                for (shell, out) in run_shells(&command, dir.path()) {
                    assert_eq!(
                        String::from_utf8_lossy(&out.stdout),
                        "needle-1\n",
                        "{shell}: {target:?}: {command} said {}",
                        String::from_utf8_lossy(&out.stderr)
                    );
                }
            }
        }
    }

    /// A `@file_*` handle of a real file, asked for a `json_path` it cannot take, names the
    /// file's own path as the place to read it: the call must parse back to the file's path (as
    /// the handle stores it, canonical: a temp dir may sit behind a symlink) and the same
    /// `json_path`, and it runs.
    #[tokio::test]
    async fn the_real_file_route_of_a_buffer_ref_error_parses_back() {
        for stem in HAZARDS {
            let dir = tempfile::tempdir().unwrap();
            let ctx = project_ctx(dir.path()).await;
            let path = write(
                dir.path(),
                &format!("{stem}.json"),
                json!({ "k": rows().join("\n") }).to_string(),
            );
            let canonical = std::fs::canonicalize(&path).unwrap();
            let canonical = canonical.to_str().unwrap();
            let whole = ReadFile
                .call(json!({ "path": path }), &ctx)
                .await
                .unwrap_or_else(|e| panic!("{stem:?}: {e}"));
            let handle = whole["file_id"]
                .as_str()
                .unwrap_or_else(|| panic!("{stem:?}: fixture: the whole read buffered no handle"))
                .to_string();
            let err = ReadFile
                .call(json!({ "path": handle, "json_path": "$.k" }), &ctx)
                .await
                .expect_err("json_path on a @file_* ref is refused")
                .to_string();
            let (args, _) = call_in(&err, "read_file(")
                .unwrap_or_else(|| panic!("{stem:?}: the refusal names no route: {err:.400}"));
            assert_eq!(args[0], (None, json!(canonical)), "{stem:?}: {err:.400}");
            assert_eq!(
                args[1],
                (Some("json_path".into()), json!("$.k")),
                "{stem:?}: {err:.400}"
            );
            // And the route runs.
            let read = ReadFile
                .call(input_of(args), &ctx)
                .await
                .unwrap_or_else(|e| panic!("{stem:?}: the named route failed: {e}"));
            assert!(
                read["content"].is_string() || read["next"].is_string(),
                "{stem:?}: {read:.300}"
            );
        }
    }
}
