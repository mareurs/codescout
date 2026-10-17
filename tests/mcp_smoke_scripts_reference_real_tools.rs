//! `tests/mcp-smoke-*.sh` drive a live MCP server by hand and sit outside
//! `cargo test`, clippy, and every markdown-scoped gate — nothing re-reads them
//! when a tool is renamed or removed.
//! `docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md`
//! is the bug this closes: both scripts called `get_symbols_overview`, a tool
//! folded into `symbols` months earlier, and neither mistake errored loudly —
//! a `symbols` call with no name argument silently falls back to the path
//! overview, which still returns symbols and satisfies a naive "found
//! something" assertion.
//!
//! **Static, not live.** This does not spin up an MCP server — these scripts
//! exist precisely because a unit test cannot drive stdio+LSP through one
//! interactive scenario per case. It cross-references every `call <name>`
//! invocation against `fn name(&self) -> &str { "<literal>" }` bodies under
//! `src/tools/`, the same literal every registered tool's `Tool::name()`
//! returns — cheap, mechanical, and the same source of truth a live registry
//! reads from.
//!
//! **Ceiling, stated rather than left implicit:** misses a tool whose `name()`
//! returns something other than a string literal (`Self::NAME`, `self.name`) —
//! `ActivateProject` is the one in-tree example today. Neither smoke script
//! calls it, so the miss costs nothing now; if one ever does, this check needs
//! widening rather than trusting a false pass.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn smoke_scripts() -> Vec<PathBuf> {
    ["tests/mcp-smoke-rust.sh", "tests/mcp-smoke-kotlin.sh"]
        .iter()
        .map(|p| repo_root().join(p))
        .collect()
}

/// Every tool name declared via a string-literal `fn name(&self) -> &str { "..." }`
/// body anywhere under `src/tools/`.
fn declared_tool_names() -> BTreeSet<String> {
    let re = regex::Regex::new(r#"fn name\(&self\) -> &str \{\s*"([a-z_][a-z_0-9]*)""#).unwrap();
    let mut names = BTreeSet::new();
    let mut stack = vec![repo_root().join("src/tools")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let Ok(src) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for cap in re.captures_iter(&src) {
                    names.insert(cap[1].to_string());
                }
            }
        }
    }
    names
}

/// Every `call <name>` invocation in a script, as `(1-indexed line, name)`.
fn called_tool_names(content: &str) -> Vec<(usize, String)> {
    let re = regex::Regex::new(r"^\s*call ([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
    content
        .lines()
        .enumerate()
        .filter_map(|(i, line)| re.captures(line).map(|c| (i + 1, c[1].to_string())))
        .collect()
}

#[test]
fn mcp_smoke_scripts_call_only_registered_tool_names() {
    let declared = declared_tool_names();
    let mut offenders = Vec::new();

    for script in smoke_scripts() {
        let content = std::fs::read_to_string(&script)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));
        let rel = script
            .strip_prefix(repo_root())
            .unwrap_or(&script)
            .display()
            .to_string();
        for (line, name) in called_tool_names(&content) {
            if !declared.contains(&name) {
                offenders.push(format!("{rel}:{line} — call {name}"));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these smoke-script calls name a tool this crate does not register:\n  {}\n\n\
         Either the tool was renamed/removed and the script is stale (fix the call), \
         or `declared_tool_names`'s literal-only scan needs widening for a tool whose \
         `name()` is not a bare string literal.",
        offenders.join("\n  ")
    );
}

/// Non-vacuity: the population this gate checks must actually be non-empty in both
/// directions, or an empty result reads as "nothing wrong" when it might mean
/// "found nothing to check" — the same false-green class as a mistyped directory in
/// `tests/committed_paths.rs`'s own non-vacuity test.
#[test]
fn the_call_and_declared_name_populations_are_not_vacuous() {
    let declared = declared_tool_names();
    assert!(
        declared.len() > 20,
        "expected >20 registered tool names from src/tools/'s name() literals; got {} — \
         the scan is looking in the wrong place or the regex stopped matching",
        declared.len()
    );
    assert!(
        declared.contains("symbols") && declared.contains("read_file") && declared.contains("grep"),
        "expected well-known tool names among the declared set; got: {declared:?}"
    );

    let mut total_calls = 0usize;
    for script in smoke_scripts() {
        let content = std::fs::read_to_string(&script)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));
        total_calls += called_tool_names(&content).len();
    }
    assert!(
        total_calls > 10,
        "expected >10 `call <tool>` invocations across both smoke scripts; got {total_calls} — \
         the line regex stopped matching the scripts' actual call shape"
    );
}

/// An absence assertion over a variable a failed call BLANKS is satisfied by
/// construction, so it passes precisely when the server is unreachable — the one
/// condition a smoke test exists to detect.
///
/// `call()` in both scripts ends `... 2>/dev/null) || RESULT=""`, which is two separate
/// losses: `2>/dev/null` discards the only artifact distinguishing "answered, string
/// absent" from "no answer", and `|| RESULT=""` then supplies a well-formed value rather
/// than a status. `assert_contains` survives that pair because it fails loudly on an
/// empty RESULT; a negated grep does not. The two are blind in OPPOSITE directions over
/// one variable, which is why they read as a symmetric pair and are not one.
///
/// **Why a static gate rather than a fix to the scripts themselves.** These scripts need
/// the `mcp` CLI and a live server and sit in no CI lane, so a repair to `call()` cannot
/// be given an observed RED here — which is exactly why the bug this closes was filed
/// "not attempted, deliberately". This assertion is the part that CAN be driven red
/// without a server, and BOTH its branches were driven, 2026-09-12:
///
/// | probe | result |
/// |---|---|
/// | `assert_not_contains() { ! echo "$RESULT" \| grep -q "$1"; }` re-added as CODE | **RED**, naming `mcp-smoke-rust.sh:38` |
/// | the byte-identical line appended as a `#` COMMENT | green — the skip below is real, not decorative |
///
/// The second row is why the comment skip is not dead defensiveness: documenting a
/// banned shape is the likeliest way someone writes it next, and without the skip this
/// gate would red on its own explanation.
///
/// Deliberately matches the SHAPE (a negated grep over `$RESULT`), not the former helper
/// NAME — renaming `assert_not_contains` must not buy a pass. It does not reach a
/// negative check written some third way; that ceiling is stated rather than implied.
/// docs/issues/archive/2026-09-09-assert-not-contains-cannot-fail-when-the-call-it-checks-failed.md
#[test]
fn no_smoke_script_asserts_absence_over_a_blankable_result() {
    // `! ... grep`, with or without a leading `echo "$RESULT" |`, on one line.
    let re = regex::Regex::new(r#"!\s*(echo\s+)?"?\$\{?RESULT\}?"?\s*\|\s*grep"#).unwrap();
    let mut offenders = Vec::new();
    let mut scanned_lines = 0usize;

    for script in smoke_scripts() {
        let content = std::fs::read_to_string(&script)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));
        let rel = script
            .strip_prefix(repo_root())
            .unwrap_or(&script)
            .display()
            .to_string();
        for (i, line) in content.lines().enumerate() {
            scanned_lines += 1;
            // A comment explaining why the shape is banned is not the shape.
            if line.trim_start().starts_with('#') {
                continue;
            }
            if re.is_match(line) {
                offenders.push(format!("{rel}:{}  {}", i + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these lines assert the ABSENCE of a string in `$RESULT`, which `call()` sets to \
         \"\" whenever the `mcp call` fails — so each one passes when the server is \
         unreachable:\n  {}\n\n\
         If the negative check is genuinely wanted, assert `[ -n \"$RESULT\" ]` FIRST, or \
         make `call()` exit non-zero instead of blanking RESULT. The discriminator is \
         already in the variable, unused.",
        offenders.join("\n  ")
    );

    // Non-vacuity: a mistyped path or an empty script set would make the loop above find
    // nothing and report clean. Both scripts are ~300 lines.
    assert!(
        scanned_lines > 100,
        "expected >100 lines across both smoke scripts; scanned {scanned_lines} — the \
         script list or the read is wrong, and this gate checked almost nothing"
    );
}

/// `mcp call` prints the tool's reply as PLAIN TEXT, so a helper that parses `$RESULT` as
/// JSON can never be satisfied by it, and the scripts spent their first live run red for
/// that reason alone.
///
/// Measured 2026-10-16 (the first live run of both scripts, docs/issues/archive/
/// 2026-09-24-residual-mcp-smoke-scripts-run.md): `mcp call read_file -p '{"path":
/// "_test_smoke.txt"}'` prints `1 line`, a blank line and `test`, not
/// `{"content": ...}`; `symbols` prints `src/server.rs (4)` followed by indented rows. Of
/// 26 steps in `mcp-smoke-rust.sh`, 18 failed, and 11 of those failed on a JSON helper
/// (`assert_json_has "content"`, `assert_symbols_found`) that reads `json.load(sys.stdin)`
/// over text. `mcp call -f json` does not rescue the shape: it wraps the reply as
/// `{"content":[{"type":"text","text":...}]}`, so `assert_json_has "content"` would then
/// pass on ANY reply, an error included, which is the same vacuity from the other side.
///
/// So the shape is banned rather than repaired: assert on the text the server printed.
/// Matches the helper NAMES and the `json.load` over `$RESULT` separately, so deleting
/// the helpers and inlining the python buys no pass.
#[test]
fn no_smoke_script_parses_the_reply_as_json() {
    let re = regex::Regex::new(r"assert_json_has|assert_symbols_found|json\.load\(\s*sys\.stdin")
        .unwrap();
    let mut offenders = Vec::new();
    let mut scanned_lines = 0usize;

    for script in smoke_scripts() {
        let content = std::fs::read_to_string(&script)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));
        let rel = script
            .strip_prefix(repo_root())
            .unwrap_or(&script)
            .display()
            .to_string();
        for (i, line) in content.lines().enumerate() {
            scanned_lines += 1;
            // A comment explaining why the shape is banned is not the shape.
            if line.trim_start().starts_with('#') {
                continue;
            }
            if re.is_match(line) {
                offenders.push(format!("{rel}:{}  {}", i + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these lines parse the `mcp call` reply as JSON, but the reply is plain text:\n  {}\n\n\
         Assert on the text instead (`assert_contains '<string the server prints>'`). \
         `mcp call -f json` is not a way out: it wraps the text in a content array, so a \
         `\"content\"` key is present on every reply, errors included.",
        offenders.join("\n  ")
    );

    // Non-vacuity: an empty script set would find nothing and report clean.
    assert!(
        scanned_lines > 100,
        "expected >100 lines across both smoke scripts; scanned {scanned_lines} — the \
         script list or the read is wrong, and this gate checked almost nothing"
    );
}
/// `tests/mcp-smoke-kotlin.sh` hardcoded a project on one developer's disk
/// (`/home/marius/work/mirela/backend-kotlin`) and asserted on files inside it. The project
/// was reorganised, every path went stale, and nothing re-read the script against it:
/// `docs/issues/2026-10-16-mcp-smoke-kotlin-targets-a-project-layout-that-no-longer-exists.md`.
///
/// The script is a document of its target's layout, so the target must be one this repository
/// owns and this test can read. Three checks, each a different way for that to rot again:
/// the script names no machine-local absolute path; it declares a `KOTLIN_FIXTURE` that
/// exists in the repo; and every literal `"path": "..."` it passes to a tool exists in that
/// fixture, or is written into the throwaway copy by the script itself
/// (`> "$KOTLIN_PROJECT/<path>"`) — the escape for non-code files the fixture does not carry.
///
/// **Ceiling:** checks `"path"` literals only. A `"glob"` or a `"pattern"` is not resolved
/// against the fixture, and an assertion string the fixture no longer contains is invisible to
/// this test; only a live run reads those.
#[test]
fn the_kotlin_smoke_script_targets_a_checked_in_fixture_and_every_path_it_reads_exists() {
    let script = repo_root().join("tests/mcp-smoke-kotlin.sh");
    let content = std::fs::read_to_string(&script)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));

    let abs = regex::Regex::new(r"/home/[A-Za-z0-9_.-]+/").unwrap();
    let machine_local: Vec<String> = content
        .lines()
        .enumerate()
        .filter(|(_, l)| abs.is_match(l))
        .map(|(i, l)| format!("tests/mcp-smoke-kotlin.sh:{}  {}", i + 1, l.trim()))
        .collect();
    assert!(
        machine_local.is_empty(),
        "the Kotlin smoke script names a path on one machine, which this repository cannot \
         keep true:\n  {}\n\nPoint it at a fixture under tests/fixtures/ instead.",
        machine_local.join("\n  ")
    );

    let fixture_re = regex::Regex::new(r#"(?m)^KOTLIN_FIXTURE="([^"]+)""#).unwrap();
    let fixture_rel = fixture_re
        .captures(&content)
        .map(|c| c[1].to_string())
        .expect("tests/mcp-smoke-kotlin.sh must declare KOTLIN_FIXTURE=\"<repo-relative dir>\"");
    let fixture = repo_root().join(&fixture_rel);
    assert!(
        fixture.is_dir(),
        "KOTLIN_FIXTURE={fixture_rel} does not exist under the repo root"
    );

    let path_re = regex::Regex::new(r#""path":\s*"([^"]+)""#).unwrap();
    let mut missing = Vec::new();
    let mut checked = 0usize;
    let mut kt_in_fixture = 0usize;
    for (i, line) in content.lines().enumerate() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        for cap in path_re.captures_iter(line) {
            let p = &cap[1];
            checked += 1;
            let generated = content.contains(&format!("\"$KOTLIN_PROJECT/{p}\""));
            if fixture.join(p).is_file() {
                if p.ends_with(".kt") {
                    kt_in_fixture += 1;
                }
            } else if !generated {
                missing.push(format!("tests/mcp-smoke-kotlin.sh:{}  {p}", i + 1));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "these paths are read by the Kotlin smoke script but are neither in {fixture_rel} nor \
         written by the script into its copy:\n  {}",
        missing.join("\n  ")
    );

    // Non-vacuity: a regex that stopped matching would check zero paths and report clean.
    assert!(
        checked >= 5 && kt_in_fixture >= 1,
        "expected >=5 `\"path\"` literals including a .kt file in the fixture; saw {checked} \
         paths, {kt_in_fixture} of them .kt — the path regex stopped matching the script"
    );
}
