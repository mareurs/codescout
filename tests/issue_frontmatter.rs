//! Frontmatter of `docs/issues/*.md` is well formed.
//!
//! Until 2026-10-17 nothing validated it. `tests/issue_clusters.rs` reads only open bug files and only
//! their `cluster/` tag lines, so a `status: fixed` file whose frontmatter had two keys glued onto one
//! line (`closed: 2026-10-17owner: marius`) passed every gate, and two archived files whose blocks do not
//! parse at all went unnoticed. A block that does not parse is invisible to every reader of `status` and
//! `kind`.
//! docs/issues/2026-10-17-no-test-validates-the-frontmatter-of-docs-issues-files-a-glued-key-line-and-two-broken-archive-files-went-unnoticed.md
//!
//! Dependency-free on purpose: a strict LINE grammar, not a YAML parser. It is stricter than YAML where
//! the repo's own files allow it, and it is a subset check: a block this accepts is not proven valid
//! YAML (multi-line scalars, anchors and flow collections are not modelled), only free of the defects
//! below.
//!
//! The rules, each with its own fixture test so that a checker which silently stopped applying one fails:
//!
//! 1. every line is blank, a `#` comment, `key: value`, `- item`, or an indented continuation;
//! 2. no top-level key repeats;
//! 3. an unquoted value contains no `: ` and does not end in `:` (two keys glued onto one line, or a
//!    colon inside prose, which YAML reads as a nested mapping), does not start with a character YAML
//!    reserves (a backtick, `@`, `%`), and is not a date glued to letters;
//! 4. a quoted value closes on its own line, and a single-quoted one doubles its inner quotes;
//! 5. `kind` and `status` are present; `status: fixed` carries `closed: YYYY-MM-DD`;
//! 6. `opened: YYYY-MM-DD` is present on a file named for a date on or after [`OPENED_REQUIRED_FROM`].
//!
//! Rules 5 and 6 are loosened from "kind, status, title and opened always": measured 2026-10-17 over the
//! 79 tracked non-archive files (the template set aside), 11 have no `title` (one as recent as 2026-10-16,
//! and the template carries none), and 10 have no `opened`, the newest named for 2026-09-23. So `title` is
//! not required, and `opened` is required only from the day after the last file that lacks it, so a new
//! file cannot repeat the gap. Only `fixed` needs `closed`: 7 `mitigated` files legitimately have none (the
//! template says to fill it when the status flips to fixed, mitigated or wontfix, and the repo does not
//! follow that for `mitigated`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;

/// First file-name date for which `opened` is mandatory. The last non-archive file lacking it is
/// `2026-09-23-read-file-file-not-found-on-a-real-untr...`.
const OPENED_REQUIRED_FROM: &str = "2026-09-24";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The text between the opening `---` line and the next `---` line, or `None` when the file does not
/// start with a frontmatter fence.
fn frontmatter_block(content: &str) -> Option<&str> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---\n").or_else(|| {
        // A file that is only a block, closed by the final fence with no trailing newline.
        rest.strip_suffix("\n---").map(str::len)
    })?;
    Some(&rest[..end])
}

/// `key` and the text after `key:` when the line is a top-level `key: value` or `key:` line.
fn split_key_line(line: &str) -> Option<(&str, &str)> {
    let colon = line.find(':')?;
    let key = &line[..colon];
    let mut chars = key.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_')
        || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    let rest = &line[colon + 1..];
    if rest.is_empty() {
        return Some((key, ""));
    }
    rest.strip_prefix(' ').map(|value| (key, value))
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            _ => c.is_ascii_digit(),
        })
}

/// Problems with one unquoted or quoted scalar value; empty when it passes rules 3 and 4.
fn value_problems(key: &str, value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let value = value.trim_end();
    if let Some(inner) = value.strip_prefix('\'') {
        // Rule 4, single quotes: closes on this line, inner quotes doubled.
        match inner.strip_suffix('\'') {
            None => out.push(format!(
                "`{key}`: single-quoted value does not close on its line"
            )),
            Some(body) => {
                if body.replace("''", "").contains('\'') {
                    out.push(format!("`{key}`: lone `'` inside a single-quoted value"));
                }
            }
        }
    } else if let Some(inner) = value.strip_prefix('"') {
        // Rule 4, double quotes: closes on this line with an unescaped quote.
        let closed = inner.strip_suffix('"').is_some_and(|body| {
            let trailing_backslashes = body.bytes().rev().take_while(|b| *b == b'\\').count();
            trailing_backslashes % 2 == 0
        });
        if !closed {
            out.push(format!(
                "`{key}`: double-quoted value does not close on its line"
            ));
        }
    } else if !value.is_empty() && !value.starts_with(['[', '{', '|', '>', '&', '*', '!']) {
        // Rule 3, plain scalar.
        if value.contains(": ") || value.ends_with(':') {
            out.push(format!(
                "`{key}`: plain value contains `: ` (two keys glued onto one line, or a colon in prose \
                 that needs quoting): {value:.60}"
            ));
        }
        if value.starts_with(['`', '@', '%']) {
            out.push(format!(
                "`{key}`: plain value starts with a character YAML reserves, so it needs quoting: {value:.40}"
            ));
        }
        let head: String = value.chars().take(10).collect();
        if is_date(&head)
            && value.len() > 10
            && value[10..].starts_with(|c: char| c.is_alphabetic())
        {
            out.push(format!("`{key}`: a date glued to letters: {value:.40}"));
        }
    }
    out
}

/// Every defect in one frontmatter block of the file named `file_name` (bare name, for the date).
fn block_problems(file_name: &str, block: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for (i, line) in block.lines().enumerate() {
        let n = i + 2; // the opening fence is line 1
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            continue; // indented continuation or nested item
        }
        if line == "-" || line.starts_with("- ") {
            continue;
        }
        match split_key_line(line) {
            Some((key, value)) => {
                if seen
                    .insert(key.to_string(), value.trim().to_string())
                    .is_some()
                {
                    out.push(format!("line {n}: duplicate key `{key}`"));
                }
                for p in value_problems(key, value) {
                    out.push(format!("line {n}: {p}"));
                }
            }
            None => out.push(format!(
                "line {n}: not `key: value`, `- item`, or an indented continuation: {line:.60}"
            )),
        }
    }
    for required in ["kind", "status"] {
        if seen.get(required).map_or(true, |v| v.is_empty()) {
            out.push(format!("missing `{required}`"));
        }
    }
    let file_date = file_name.get(..10).filter(|d| is_date(d)).unwrap_or("");
    if !file_date.is_empty() && file_date >= OPENED_REQUIRED_FROM {
        match seen.get("opened") {
            None => out.push(format!(
                "missing `opened` (required for files dated {OPENED_REQUIRED_FROM} or later)"
            )),
            Some(v) if !is_date(v) => out.push(format!("`opened` is not YYYY-MM-DD: {v:?}")),
            _ => {}
        }
    }
    if seen.get("status").map(String::as_str) == Some("fixed") {
        match seen.get("closed") {
            Some(v) if is_date(v) => {}
            other => out.push(format!(
                "`status: fixed` needs `closed: YYYY-MM-DD`, found {other:?}"
            )),
        }
    }
    out
}

/// Problems with a whole file: no block at all, or the block's problems.
fn file_problems(file_name: &str, content: &str) -> Vec<String> {
    match frontmatter_block(content) {
        None => vec!["no `---` frontmatter block at the top of the file".to_string()],
        Some(block) => block_problems(file_name, block),
    }
}

/// Tracked, non-archive `docs/issues/*.md`, except underscore-prefixed files (`_TEMPLATE.md` is a template
/// with a placeholder date, not a record). Deduplicated: `git ls-files` lists an unmerged path once per
/// stage, and this population is files.
fn tracked_issue_files() -> Vec<String> {
    let out = Command::new("git")
        .current_dir(repo_root())
        .args(["ls-files", "-z", "--", "docs/issues"])
        .output()
        .expect("git ls-files must run");
    assert!(
        out.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("tracked paths are UTF-8")
        .split('\0')
        .filter_map(|p| p.strip_prefix("docs/issues/"))
        .filter(|rest| !rest.contains('/') && rest.ends_with(".md") && !rest.starts_with('_'))
        .map(|rest| format!("docs/issues/{rest}"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The frontmatter block of the p50 emission-budget issue as it stood at `31f0041c^`: a `fixed` record
/// whose `closed` line had `owner: marius` glued onto it. PyYAML refuses it: "mapping values are not
/// allowed here".
const PRE_FIX_GLUED_FILE: &str = "---
kind: bug
status: fixed
title: 'the p50 emission-budget fixture moves docs/specs to docs/specs, so archiving a bug file (about 9.6 KB of tracker-conventions sections after a doc.move) is outside every session-level byte budget'
tags:
- cluster/instrument-omits-the-dimension-that-grows
- guides
- section-grain-delivery
opened: 2026-10-17
closed: 2026-10-17owner: marius
severity: low
related:
- docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md
---

# BUG: no session-level byte budget covers a bug-file archive move
";

const P50_NAME: &str = "2026-10-17-p50-emission-budget-fixture-never-archives-a-bug-file.md";

/// The same file after the repair: the two keys on their own lines.
fn repaired() -> String {
    PRE_FIX_GLUED_FILE.replace(
        "closed: 2026-10-17owner: marius",
        "closed: 2026-10-17\nowner: marius",
    )
}

#[test]
fn every_tracked_issue_file_has_wellformed_frontmatter() {
    let files = tracked_issue_files();
    // Floor, not a total: an empty or truncated walk must not pass by checking nothing. Measured
    // 2026-10-17: 80 tracked non-archive files, 79 once the template is set aside.
    assert!(
        files.len() >= 60,
        "the walk found only {} tracked non-archive issue files; it is not reading the population",
        files.len()
    );
    let mut failures = Vec::new();
    for file in &files {
        let content = std::fs::read_to_string(repo_root().join(file))
            .unwrap_or_else(|e| panic!("tracked file {file} is unreadable: {e}"));
        let name = file.rsplit('/').next().unwrap();
        for p in file_problems(name, &content) {
            failures.push(format!("{file}: {p}"));
        }
    }
    assert!(
        failures.is_empty(),
        "docs/issues frontmatter is malformed ({} problem(s) over {} files). Repair the YAML syntax in \
         place; do not loosen the rule:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}

#[test]
fn the_pre_fix_glued_blob_is_rejected_and_the_repair_passes() {
    let before = file_problems(P50_NAME, PRE_FIX_GLUED_FILE);
    assert!(
        before
            .iter()
            .any(|p| p.contains("`closed`") && p.contains("glued")),
        "the glued `closed` line must be named; got {before:#?}"
    );
    let after = file_problems(P50_NAME, &repaired());
    assert!(
        after.is_empty(),
        "the repaired file must pass; got {after:#?}"
    );
}

#[test]
fn a_file_without_a_frontmatter_block_is_refused() {
    assert!(!file_problems(P50_NAME, "# BUG: no fence\n").is_empty());
    assert!(!file_problems(P50_NAME, "---\nkind: bug\nstatus: open\n").is_empty());
}

#[test]
fn a_missing_opened_is_refused_only_from_the_cutoff() {
    let without_opened = repaired().replace("opened: 2026-10-17\n", "");
    let after = file_problems(P50_NAME, &without_opened);
    assert!(
        after.iter().any(|p| p.contains("missing `opened`")),
        "{after:#?}"
    );
    // A legacy file named before the cutoff may lack it.
    let legacy = file_problems("2026-09-23-legacy.md", &without_opened);
    assert!(legacy.is_empty(), "{legacy:#?}");
    let bad_date = repaired().replace("opened: 2026-10-17\n", "opened: yesterday\n");
    assert!(file_problems(P50_NAME, &bad_date)
        .iter()
        .any(|p| p.contains("`opened` is not")));
}

#[test]
fn a_fixed_file_needs_a_dated_closed_and_a_mitigated_one_does_not() {
    let no_closed = repaired().replace("closed: 2026-10-17\n", "");
    assert!(
        file_problems(P50_NAME, &no_closed)
            .iter()
            .any(|p| p.contains("needs `closed")),
        "fixed without closed must be refused"
    );
    let empty_closed = repaired().replace("closed: 2026-10-17\n", "closed:\n");
    assert!(file_problems(P50_NAME, &empty_closed)
        .iter()
        .any(|p| p.contains("needs `closed")));
    let mitigated = no_closed.replace("status: fixed", "status: mitigated");
    assert!(file_problems(P50_NAME, &mitigated).is_empty());
}

#[test]
fn kind_and_status_are_required() {
    let no_kind = repaired().replace("kind: bug\n", "");
    assert!(file_problems(P50_NAME, &no_kind)
        .iter()
        .any(|p| p.contains("missing `kind`")));
    let no_status = repaired().replace("status: fixed\n", "");
    assert!(file_problems(P50_NAME, &no_status)
        .iter()
        .any(|p| p.contains("missing `status`")));
}

#[test]
fn a_duplicate_top_level_key_is_refused() {
    let dup = repaired().replace("severity: low\n", "severity: low\nseverity: medium\n");
    assert!(file_problems(P50_NAME, &dup)
        .iter()
        .any(|p| p.contains("duplicate key `severity`")));
}

#[test]
fn a_line_that_is_none_of_the_allowed_shapes_is_refused() {
    let junk = repaired().replace("severity: low\n", "severity low\n");
    assert!(file_problems(P50_NAME, &junk)
        .iter()
        .any(|p| p.contains("not `key: value`")));
}

#[test]
fn a_plain_value_that_yaml_cannot_read_is_refused_but_a_quoted_one_is_not() {
    // The two archived files repaired alongside this test: a leading backtick, and `: ` in prose.
    let backtick = repaired().replace(
        "severity: low\n",
        "unverified: `a` and `b` were dropped\nseverity: low\n",
    );
    assert!(file_problems(P50_NAME, &backtick)
        .iter()
        .any(|p| p.contains("reserves")));
    let colon = repaired().replace(
        "severity: low\n",
        "unverified: it is PERMANENT: change both\nseverity: low\n",
    );
    assert!(file_problems(P50_NAME, &colon)
        .iter()
        .any(|p| p.contains("contains `: `")));
    let quoted = repaired().replace(
        "severity: low\n",
        "unverified: \"`a` and `b` were dropped: both\"\nseverity: low\n",
    );
    assert!(file_problems(P50_NAME, &quoted).is_empty());
    let single = repaired().replace(
        "severity: low\n",
        "unverified: 'it''s: fine'\nseverity: low\n",
    );
    assert!(file_problems(P50_NAME, &single).is_empty());
}

#[test]
fn a_quoted_value_that_does_not_close_or_has_a_lone_quote_is_refused() {
    let open = repaired().replace(
        "severity: low\n",
        "unverified: 'never closed\nseverity: low\n",
    );
    assert!(file_problems(P50_NAME, &open)
        .iter()
        .any(|p| p.contains("does not close")));
    let lone = repaired().replace(
        "severity: low\n",
        "unverified: 'it's broken'\nseverity: low\n",
    );
    assert!(file_problems(P50_NAME, &lone)
        .iter()
        .any(|p| p.contains("lone `'`")));
}

#[test]
fn a_date_glued_to_letters_is_refused_even_without_a_colon() {
    let glued = repaired().replace("closed: 2026-10-17\n", "closed: 2026-10-17fixed\n");
    assert!(file_problems(P50_NAME, &glued)
        .iter()
        .any(|p| p.contains("glued")));
}

#[test]
fn indented_continuations_comments_and_blank_lines_are_allowed() {
    let ok = repaired().replace(
        "severity: low\n",
        "severity: low\n# a comment\n\nrelated2:\n  - nested: item\n  continuation text\n",
    );
    assert!(
        file_problems(P50_NAME, &ok).is_empty(),
        "{:#?}",
        file_problems(P50_NAME, &ok)
    );
}
