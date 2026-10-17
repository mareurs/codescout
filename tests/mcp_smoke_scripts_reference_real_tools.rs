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

/// Replace the first occurrence of `old` in the script, panicking if it is absent: a mutant
/// whose anchor text vanished must fail loudly, not pass as an unmutated copy.
fn mutate(content: &str, old: &str, new: &str) -> String {
    assert!(
        content.contains(old),
        "self-test anchor missing from tests/mcp-smoke-kotlin.sh: {old}"
    );
    content.replacen(old, new, 1)
}

/// Comment out the first non-comment line containing `needle` (panics if there is none).
fn comment_out_line(content: &str, needle: &str) -> String {
    let mut done = false;
    let mut out: Vec<String> = Vec::new();
    for line in content.lines() {
        if !done && !line.trim_start().starts_with('#') && line.contains(needle) {
            done = true;
            out.push(format!("# {line}"));
        } else {
            out.push(line.to_string());
        }
    }
    assert!(
        done,
        "self-test anchor missing from tests/mcp-smoke-kotlin.sh: {needle}"
    );
    out.join("\n") + "\n"
}

/// Everything this repository can check statically about the target of
/// `tests/mcp-smoke-kotlin.sh`, one line per offence; empty means clean. Comment lines are
/// never evidence: a requirement met only in a comment is not met.
fn kotlin_smoke_script_offenders(content: &str, root: &std::path::Path) -> Vec<String> {
    let re = |p: &str| regex::Regex::new(p).unwrap();
    let mut bad: Vec<String> = Vec::new();

    // No path on one machine, comments included: there is no legitimate reason to name one.
    let machine_local = re(r"/home/[A-Za-z0-9_.-]+/");
    for (i, l) in content.lines().enumerate() {
        if machine_local.is_match(l) {
            bad.push(format!(
                "line {}: names a path on one machine: {}",
                i + 1,
                l.trim()
            ));
        }
    }

    let code: Vec<(usize, &str)> = content
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l))
        .filter(|(_, l)| !l.trim_start().starts_with('#'))
        .collect();
    let first = |pat: &str| -> Option<usize> {
        let r = re(pat);
        code.iter().find(|(_, l)| r.is_match(l)).map(|(n, _)| *n)
    };

    let fixture_rel = re(r#"(?m)^KOTLIN_FIXTURE="([^"]+)"$"#)
        .captures(content)
        .map(|c| c[1].to_string());
    let Some(fixture_rel) = fixture_rel else {
        bad.push("no KOTLIN_FIXTURE=\"<repo-relative dir>\" declaration".into());
        return bad;
    };
    let fixture = root.join(&fixture_rel);
    if !fixture.is_dir() {
        bad.push(format!(
            "KOTLIN_FIXTURE={fixture_rel} is not a directory under the repo root"
        ));
        return bad;
    }

    // The target is a fresh mktemp directory, never the fixture and never a path on a disk.
    let work = first(r#"^KOTLIN_WORK="\$\(mktemp -d\)"$"#);
    if work.is_none() {
        bad.push("KOTLIN_WORK must be assigned exactly `\"$(mktemp -d)\"`".into());
    }
    let project_assign = re(r"^KOTLIN_PROJECT=");
    let project_ok = re(r#"^KOTLIN_PROJECT="\$KOTLIN_WORK/[^"$]+"$"#);
    let mut project_assignments = 0;
    for (n, l) in &code {
        if project_assign.is_match(l) {
            project_assignments += 1;
            if !project_ok.is_match(l) {
                bad.push(format!(
                    "line {n}: KOTLIN_PROJECT must live under $KOTLIN_WORK, got: {}",
                    l.trim()
                ));
            }
        }
    }
    if project_assignments == 0 {
        bad.push("KOTLIN_PROJECT is never assigned".into());
    }
    if first(r#"^export TMPDIR="\$KOTLIN_WORK/[^"$]+"$"#).is_none() {
        bad.push(
            "TMPDIR must be exported inside $KOTLIN_WORK, or kotlin-lsp's temp_dir() state \
             outlives the run"
                .into(),
        );
    }

    // The trap is armed after the directory exists and before anything that can fail.
    match first(r"^trap cleanup EXIT$") {
        None => bad.push("`trap cleanup EXIT` is missing".into()),
        Some(t) => {
            if !matches!(work, Some(w) if w < t) {
                bad.push("`trap cleanup EXIT` must come after KOTLIN_WORK is created".into());
            }
            for (what, pat) in [
                ("the release build", "cargo build"),
                ("the alias add", "mcp alias add"),
            ] {
                if first(pat).is_some_and(|n| n < t) {
                    bad.push(format!("`trap cleanup EXIT` is armed after {what}"));
                }
            }
        }
    }
    let cleanup_start = first(r"^cleanup\(\) \{$");
    let cleanup_end = cleanup_start.and_then(|s| {
        code.iter()
            .find(|(n, l)| *n > s && *l == "}")
            .map(|(n, _)| *n)
    });
    match (cleanup_start, cleanup_end) {
        (Some(s), Some(e)) => {
            let body: Vec<&str> = code
                .iter()
                .filter(|(n, _)| *n > s && *n < e)
                .map(|(_, l)| l.trim())
                .collect();
            for need in [
                r#"rm -rf "$KOTLIN_WORK""#,
                "mcp alias remove",
                r#"pkill -f -- "$KOTLIN_WORK""#,
            ] {
                if !body.iter().any(|l| l.starts_with(need)) {
                    bad.push(format!("cleanup() does not run `{need}`"));
                }
            }
            // The mux re-creates its directories as it shuts down, so it must be stopped
            // before the directory is removed, not after.
            let at = |needle: &str| body.iter().position(|l| l.starts_with(needle));
            if let (Some(k), Some(r)) = (at("pkill"), at("rm -rf")) {
                if k > r {
                    bad.push("cleanup() removes $KOTLIN_WORK before stopping its processes".into());
                }
            }
        }
        _ => bad.push("no cleanup() { ... } function".into()),
    }

    // The copy: tracked files only, with the fixture's depth stripped, and a plain-copy
    // fallback for a tree that is not a git checkout.
    let depth = fixture_rel.split('/').count();
    let archive = re(
        r#"^\s*git -C "\$REPO_ROOT" archive HEAD "\$KOTLIN_FIXTURE" \| tar -x --strip-components=(\d+) -C "\$KOTLIN_PROJECT"$"#,
    );
    match code.iter().find_map(|(_, l)| archive.captures(l)) {
        None => bad.push(
            "no `git -C \"$REPO_ROOT\" archive HEAD \"$KOTLIN_FIXTURE\" | tar -x \
             --strip-components=N -C \"$KOTLIN_PROJECT\"` line"
                .into(),
        ),
        Some(c) => {
            if c[1].parse::<usize>().ok() != Some(depth) {
                bad.push(format!(
                    "--strip-components={} does not match the {depth} components of {fixture_rel}",
                    &c[1]
                ));
            }
        }
    }
    if first(r#"^\s*cp -R "\$REPO_ROOT/\$KOTLIN_FIXTURE"/\. "\$KOTLIN_PROJECT"/$"#).is_none() {
        bad.push(
            "no `cp -R \"$REPO_ROOT/$KOTLIN_FIXTURE\"/. \"$KOTLIN_PROJECT\"/` fallback line".into(),
        );
    }

    // Every path the script hands a tool exists in the fixture, or is written by the script
    // into its copy with `> "$KOTLIN_PROJECT/<path>"` on a code line.
    let path_re = re(r#""path":\s*"([^"]+)""#);
    let mut checked = 0usize;
    let mut kt_in_fixture = 0usize;
    for (n, line) in &code {
        for cap in path_re.captures_iter(line) {
            let p = &cap[1];
            checked += 1;
            if p.starts_with('/') || p.split('/').any(|c| c == "..") {
                bad.push(format!("line {n}: path `{p}` escapes the project"));
                continue;
            }
            let written = re(&format!(r#">\s*"\$KOTLIN_PROJECT/{}""#, regex::escape(p)));
            if fixture.join(p).is_file() {
                if p.ends_with(".kt") {
                    kt_in_fixture += 1;
                }
            } else if !code.iter().any(|(_, l)| written.is_match(l)) {
                bad.push(format!(
                    "line {n}: path `{p}` is neither in {fixture_rel} nor written by the script into its copy"
                ));
            }
        }
    }
    // Non-vacuity: a regex that stopped matching would check zero paths and report clean.
    if checked < 5 || kt_in_fixture < 1 {
        bad.push(format!(
            "expected >=5 `\"path\"` literals including a .kt file in the fixture; saw {checked} \
             paths, {kt_in_fixture} of them .kt"
        ));
    }
    bad
}

/// `tests/mcp-smoke-kotlin.sh` hardcoded a project on one developer's disk
/// (`/home/marius/work/mirela/backend-kotlin`) and asserted on files inside it. The project
/// was reorganised, every path went stale, and nothing re-read the script against it:
/// `docs/issues/2026-10-16-mcp-smoke-kotlin-targets-a-project-layout-that-no-longer-exists.md`.
///
/// The script is a document of its target's layout, so the target must be one this repository
/// owns and this test can read. `kotlin_smoke_script_offenders` checks that the script runs
/// against a throwaway `mktemp` copy of the tracked fixture files (never the fixture itself,
/// never a path on a disk), that the trap which removes the copy, its alias and its `TMPDIR`
/// is armed first, and that every `"path": "..."` it hands a tool stays inside the fixture or
/// is written by the script (`> "$KOTLIN_PROJECT/<path>"`) for the non-code files the fixture
/// does not carry. Each of those is also run against a mutant of the script that breaks it,
/// which must be reported.
///
/// **Ceiling:** a static read. It resolves `"path"` literals only: a `"glob"` or a `"pattern"`
/// is not resolved against the fixture, and an assertion string the fixture no longer
/// contains is invisible here; only a live run reads those.
#[test]
fn the_kotlin_smoke_script_targets_a_checked_in_fixture_and_every_path_it_reads_exists() {
    let script = repo_root().join("tests/mcp-smoke-kotlin.sh");
    let content = std::fs::read_to_string(&script)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));

    let offenders = kotlin_smoke_script_offenders(&content, &repo_root());
    assert!(
        offenders.is_empty(),
        "tests/mcp-smoke-kotlin.sh no longer targets a throwaway copy of a checked-in fixture:\n  {}",
        offenders.join("\n  ")
    );

    // The checks above are only as good as their ability to fail, so each mutant of the
    // script below must be reported. A mutant whose anchor text vanished panics rather than
    // passing silently, so a rewrite of the script cannot quietly retire a case.
    let mutants: Vec<(&str, String)> = vec![
        (
            "target the checked-in fixture itself (cleanup would rm -rf it)",
            mutate(
                &content,
                r#"KOTLIN_PROJECT="$KOTLIN_WORK/project""#,
                r#"KOTLIN_PROJECT="$KOTLIN_FIXTURE""#,
            ),
        ),
        (
            "target a project under $HOME",
            mutate(
                &content,
                r#"KOTLIN_PROJECT="$KOTLIN_WORK/project""#,
                r#"KOTLIN_PROJECT="$HOME/work/mirela/backend-kotlin""#,
            ),
        ),
        (
            "mktemp replaced by a fixed directory",
            mutate(
                &content,
                r#"KOTLIN_WORK="$(mktemp -d)""#,
                r#"KOTLIN_WORK="$HOME/kotlin-smoke""#,
            ),
        ),
        (
            "git archive copy deleted",
            comment_out_line(&content, "archive HEAD"),
        ),
        (
            "cp fallback deleted",
            comment_out_line(&content, r#"cp -R "$REPO_ROOT/$KOTLIN_FIXTURE""#),
        ),
        (
            "git archive strips the wrong number of components",
            mutate(&content, "--strip-components=3", "--strip-components=2"),
        ),
        (
            "README write commented out",
            comment_out_line(&content, r#"> "$KOTLIN_PROJECT/README.md""#),
        ),
        (
            "rm -rf dropped from cleanup",
            comment_out_line(&content, r#"rm -rf "$KOTLIN_WORK""#),
        ),
        (
            "alias removal dropped from cleanup",
            comment_out_line(&content, "mcp alias remove"),
        ),
        (
            "processes not stopped before the directory is removed",
            comment_out_line(&content, "pkill -f --"),
        ),
        (
            "directory removed before the processes are stopped",
            mutate(
                &content,
                "    pkill -f --",
                "    rm -rf \"$KOTLIN_WORK\"\n    pkill -f --",
            ),
        ),
        (
            "trap removed",
            comment_out_line(&content, "trap cleanup EXIT"),
        ),
        (
            "trap armed only after the release build and the alias add",
            format!(
                "{}\ntrap cleanup EXIT\n",
                comment_out_line(&content, "trap cleanup EXIT")
            ),
        ),
        (
            "TMPDIR no longer pointed inside the per-run directory",
            comment_out_line(&content, "export TMPDIR="),
        ),
        (
            "a path escaping the fixture",
            mutate(
                &content,
                r#""path": "gradle.properties""#,
                r#""path": "../../../Cargo.toml""#,
            ),
        ),
        (
            "an absolute path",
            mutate(
                &content,
                r#""path": "gradle.properties""#,
                r#""path": "/etc/hostname""#,
            ),
        ),
        (
            "a path the fixture does not carry",
            mutate(&content, "models/Genre.kt", "models/Genr.kt"),
        ),
    ];
    for (name, mutated) in &mutants {
        assert!(
            !kotlin_smoke_script_offenders(mutated, &repo_root()).is_empty(),
            "mutant not caught by the static check: {name}"
        );
    }
}
