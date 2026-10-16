//! Write-time requirements for artifacts of a kind, read from the project's own policy file.
//!
//! Decision: `docs/adrs/2026-10-16-a-rule-lives-where-the-agent-already-acts.md`. A rule an agent must
//! follow belongs in the write path, as a refusal that carries its own remedy, not in a document the
//! agent has to remember. The mechanism is generic and lives here. The POLICY is data, in
//! [`REQUIREMENTS_REL`], so a project that has no such file is not touched at all.
//!
//! WHY A FILE OF ITS OWN and not a table in `.codescout/librarian.toml`: `classify::RuleFile` is
//! `#[serde(deny_unknown_fields)]`, so a new table there would make every session running an older
//! server binary fail to parse its classifier rules. Live sessions run several binary versions at
//! once (17 of 30 were on a replaced binary on 2026-10-16). A separate file is invisible to a binary
//! that does not know it.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::librarian::tools::LibrarianRecoverableError;

/// Per-project requirements file, relative to the project root.
pub const REQUIREMENTS_REL: &str = ".codescout/requirements.toml";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequirementsFile {
    #[serde(default, rename = "require")]
    requires: Vec<Requirement>,
}

/// One requirement: what an artifact of `kind` must carry to be created.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Requirement {
    kind: String,
    /// Exactly one tag must start with this prefix (for example `cluster/`).
    #[serde(default)]
    tag_prefix: Option<String>,
    /// Files or directories (of `.md` files), relative to the project root, that DECLARE the values the
    /// tag may take. When empty the tag is only counted, not checked against a closed set.
    #[serde(default)]
    tag_values_from: Vec<String>,
    /// A line starting with this text declares a value in its first backticked `<prefix><value>` token.
    #[serde(default)]
    tag_declared_by: Option<String>,
    /// H2 headings the body must contain, outside any code fence, each with enough text.
    #[serde(default)]
    sections: Vec<String>,
    #[serde(default = "default_min_section_chars")]
    min_section_chars: usize,
    /// What to write under a required section. Shown only when the section is refused.
    #[serde(default)]
    section_hint: Option<String>,
}

fn default_min_section_chars() -> usize {
    40
}

fn load(project_root: &Path) -> Result<Vec<Requirement>> {
    let path = project_root.join(REQUIREMENTS_REL);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading project requirements at {}", path.display()))?;
    let file: RequirementsFile = toml::from_str(&text)
        .with_context(|| format!("parsing project requirements at {}", path.display()))?;
    Ok(file.requires)
}

/// The values a tag may take, read from the files that declare them.
///
/// Mirrors `valid_slugs` in `scripts/pre-commit-ledger-counts.py`, which the commit hook uses for the
/// same closed set: a declaring line starts with `declared_by`, its first backticked token is
/// `<prefix><value>`, and a value is lowercase ASCII letters and `-` only. That last rule drops the
/// template placeholder `cluster/<slug>` and, as a side effect the hook shares, any value with a digit.
fn declared_values(req: &Requirement, root: &Path, prefix: &str) -> Result<BTreeSet<String>> {
    let Some(declared_by) = req.tag_declared_by.as_deref() else {
        anyhow::bail!("{REQUIREMENTS_REL}: `tag_values_from` needs `tag_declared_by`");
    };
    let mut files = Vec::new();
    for rel in &req.tag_values_from {
        let p = root.join(rel);
        if p.is_dir() {
            let mut kids: Vec<_> = std::fs::read_dir(&p)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|k| k.extension().is_some_and(|x| x == "md"))
                .collect();
            kids.sort();
            files.extend(kids);
        } else if p.is_file() {
            files.push(p);
        }
    }
    let mut out = BTreeSet::new();
    for f in files {
        for line in std::fs::read_to_string(&f)?.lines() {
            let Some(rest) = line.strip_prefix(declared_by) else {
                continue;
            };
            let mut ticks = rest.split('`');
            let (Some(_), Some(token)) = (ticks.next(), ticks.next()) else {
                continue;
            };
            let Some(value) = token.strip_prefix(prefix) else {
                continue;
            };
            if !value.is_empty() && value.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                out.insert(value.to_string());
            }
        }
    }
    Ok(out)
}

fn check_tags(req: &Requirement, root: &Path, tags: &[String]) -> Result<()> {
    let Some(prefix) = req.tag_prefix.as_deref() else {
        return Ok(());
    };
    let mine: Vec<&String> = tags.iter().filter(|t| t.starts_with(prefix)).collect();
    let declared = if req.tag_values_from.is_empty() {
        None
    } else {
        let d = declared_values(req, root, prefix)?;
        if d.is_empty() {
            anyhow::bail!(
                "{REQUIREMENTS_REL}: none of {} declares a `{prefix}…` value on a `{}` line, so the tag of a \
                 {} cannot be checked. The files may have moved; fix the policy rather than skip the check.",
                req.tag_values_from.join(", "),
                req.tag_declared_by.as_deref().unwrap_or(""),
                req.kind,
            );
        }
        Some(d)
    };
    let listing = declared
        .as_ref()
        .map(|d| {
            format!(
                " Declared values: {}.",
                d.iter().cloned().collect::<Vec<_>>().join(", ")
            )
        })
        .unwrap_or_default();
    let how = format!("Pass tags=[\"{prefix}<value>\"] to doc(action=\"create\").{listing}");
    if mine.len() != 1 {
        let has = if mine.is_empty() {
            "none".to_string()
        } else {
            format!(
                "{}: {}",
                mine.len(),
                mine.iter()
                    .map(|t| t.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        return Err(LibrarianRecoverableError::with_hint(
            format!(
                "a {} needs exactly one `{prefix}…` tag; this one has {has}",
                req.kind
            ),
            how,
        ));
    }
    if let Some(d) = declared {
        let value = &mine[0][prefix.len()..];
        if !d.contains(value) {
            return Err(LibrarianRecoverableError::with_hint(
                format!("`{}` is not a declared value of `{prefix}…`", mine[0]),
                how,
            ));
        }
    }
    Ok(())
}

/// Letters and digits under the H2 `## <title>`, outside code fences, up to the next H1 or H2.
/// Several sections with the same title count by the largest, so a stray empty one cannot mask a real one.
fn section_text_len(body: &str, title: &str) -> Option<usize> {
    let mut fenced = false;
    let mut current: Option<usize> = None;
    let mut best: Option<usize> = None;
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if !fenced && (line.starts_with("# ") || line.starts_with("## ")) {
            if let Some(n) = current.take() {
                best = Some(best.map_or(n, |b| b.max(n)));
            }
            if line.strip_prefix("## ").is_some_and(|h| h.trim() == title) {
                current = Some(0);
            }
            continue;
        }
        if let Some(n) = current.as_mut() {
            *n += line.chars().filter(|c| c.is_alphanumeric()).count();
        }
    }
    if let Some(n) = current {
        best = Some(best.map_or(n, |b| b.max(n)));
    }
    best
}

fn check_sections(req: &Requirement, body: &str) -> Result<()> {
    for title in &req.sections {
        let have = section_text_len(body, title);
        if have.is_some_and(|n| n >= req.min_section_chars) {
            continue;
        }
        let state = match have {
            None => "this one has none".to_string(),
            Some(n) => format!("this one has {n}"),
        };
        let default_hint = format!(
            "Add a `## {title}` heading outside any code fence, with at least {} letters or digits under it.",
            req.min_section_chars
        );
        return Err(LibrarianRecoverableError::with_hint(
            format!(
                "a {} needs a `## {title}` section with at least {} letters or digits of text; {state}",
                req.kind, req.min_section_chars
            ),
            req.section_hint.clone().unwrap_or(default_hint),
        ));
    }
    Ok(())
}

/// Refuse the creation of an artifact that the project's requirements say is incomplete.
///
/// A project with no [`REQUIREMENTS_REL`] is unaffected. A malformed file is an error, not a skip:
/// a policy that silently stops applying is worse than one that says it is broken.
pub fn enforce_on_create(
    project_root: &Path,
    kind: &str,
    tags: &[String],
    body: &str,
) -> Result<()> {
    for req in load(project_root)?.iter().filter(|r| r.kind == kind) {
        check_tags(req, project_root, tags)?;
        check_sections(req, body)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// The policy this repository will ship, in miniature: a bug carries exactly one `cluster/…` tag
    /// whose value some file declares on a `**Slug:**` line, and a `## Class fit` section.
    const POLICY: &str = r#"
[[require]]
kind = "bug"
tag_prefix = "cluster/"
tag_values_from = ["ledger.md", "classes"]
tag_declared_by = "**Slug:**"
sections = ["Class fit"]
min_section_chars = 40
"#;

    /// 49 letters, well over the floor of 40 (hand-counted: the=3 selector=8 covers=6 one=3 of=2 the=3
    /// two=3 forms=5 the=3 population=10 has=3).
    const GOOD: &str = "the selector covers one of the two forms the population has";

    fn write(root: &Path, rel: &str, content: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    /// A project with the policy, a roster-style file declaring `unclassified`, and two class files.
    /// The class file `IC-9-template.md` carries the placeholder `cluster/<slug>`, which is not a value.
    fn project() -> TempDir {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), REQUIREMENTS_REL, POLICY);
        write(
            tmp.path(),
            "ledger.md",
            "# roster\n\n**Slug:** `cluster/unclassified`\n",
        );
        write(
            tmp.path(),
            "classes/IC-1-alpha.md",
            "# a\n\n**Slug:** `cluster/alpha`\n",
        );
        write(
            tmp.path(),
            "classes/IC-2-beta.md",
            "# b\n\n**Slug:** `cluster/beta`\n",
        );
        write(
            tmp.path(),
            "classes/IC-9-template.md",
            "**Slug:** `cluster/<slug>`\n",
        );
        tmp
    }

    fn tags(t: &[&str]) -> Vec<String> {
        t.iter().map(|s| s.to_string()).collect()
    }

    fn fit(text: &str) -> String {
        format!("# BUG: x\n\n## Summary\n\ns\n\n## Class fit\n\n{text}\n\n## References\n\nr\n")
    }

    fn refusal(root: &Path, kind: &str, t: &[&str], body: &str) -> String {
        enforce_on_create(root, kind, &tags(t), body)
            .expect_err("this input must be refused")
            .to_string()
    }

    // --- Nothing changes where nothing is declared --------------------------------------------

    /// Kills a validator that applies the policy to every project. Without the file, a bug with no
    /// tag and no section is exactly as acceptable as it was before this module existed.
    #[test]
    fn a_project_with_no_requirements_file_is_untouched() {
        let tmp = TempDir::new().unwrap();
        enforce_on_create(tmp.path(), "bug", &[], "# BUG: x\n").unwrap();
    }

    /// Kills a validator that ignores `kind`: the bug policy must not reach a tracker.
    #[test]
    fn another_kind_is_untouched_by_the_bug_policy() {
        let tmp = project();
        enforce_on_create(tmp.path(), "tracker", &[], "# T\n").unwrap();
    }

    // --- The tag --------------------------------------------------------------------------------

    /// Kills "no tag is fine". The hint must list the declared values: the whole point of refusing at
    /// the write is that the refusal says what to write, so the caller does not go and read a ledger.
    #[test]
    fn a_bug_with_no_cluster_tag_is_refused_and_the_declared_values_are_listed() {
        let tmp = project();
        let msg = refusal(tmp.path(), "bug", &["other"], &fit(GOOD));
        assert!(msg.contains("exactly one"), "got: {msg}");
        assert!(msg.contains("cluster/"), "got: {msg}");
        for v in ["alpha", "beta", "unclassified"] {
            assert!(
                msg.contains(v),
                "the declared value {v} must be listed: {msg}"
            );
        }
    }

    /// Kills "at least one": a bug in two classes makes the counts non-additive.
    #[test]
    fn two_cluster_tags_are_refused() {
        let tmp = project();
        let msg = refusal(
            tmp.path(),
            "bug",
            &["cluster/alpha", "cluster/beta"],
            &fit(GOOD),
        );
        assert!(msg.contains("exactly one"), "got: {msg}");
        assert!(
            msg.contains("cluster/alpha") && msg.contains("cluster/beta"),
            "got: {msg}"
        );
    }

    /// Kills a validator that checks only the prefix. A typo'd slug is otherwise a silent class of
    /// one: it reads as a real cluster and matches nothing.
    #[test]
    fn an_undeclared_slug_is_refused_and_named() {
        let tmp = project();
        let msg = refusal(tmp.path(), "bug", &["cluster/typo"], &fit(GOOD));
        assert!(msg.contains("typo"), "got: {msg}");
        assert!(
            msg.contains("alpha"),
            "the declared values must be listed: {msg}"
        );
    }

    /// Kills a value scan that does not tell a placeholder from a value. The ledger's own template
    /// declares `cluster/<slug>`; a bug tagged with it is not classified.
    #[test]
    fn the_template_placeholder_is_not_a_declared_value() {
        let tmp = project();
        let msg = refusal(tmp.path(), "bug", &["cluster/<slug>"], &fit(GOOD));
        assert!(msg.contains("<slug>"), "got: {msg}");
    }

    /// Kills a scan that reads only the directory, or only the file. `unclassified` is declared in the
    /// roster file and the rest in the class directory, as in the live ledger.
    #[test]
    fn a_value_declared_in_the_roster_file_is_accepted_like_one_in_the_directory() {
        let tmp = project();
        enforce_on_create(
            tmp.path(),
            "bug",
            &tags(&["cluster/unclassified"]),
            &fit(GOOD),
        )
        .unwrap();
        enforce_on_create(tmp.path(), "bug", &tags(&["cluster/alpha"]), &fit(GOOD)).unwrap();
    }

    // --- The section ----------------------------------------------------------------------------

    /// Kills "the tag is enough". The refusal names the exact heading to write.
    #[test]
    fn a_declared_slug_without_the_section_is_refused() {
        let tmp = project();
        let msg = refusal(
            tmp.path(),
            "bug",
            &["cluster/alpha"],
            "# BUG: x\n\n## Summary\n\ns\n",
        );
        assert!(msg.contains("## Class fit"), "got: {msg}");
        assert!(msg.contains("40"), "the floor must be stated: {msg}");
    }

    /// Kills a presence-only check: an empty heading is a placeholder, and any rule that a placeholder
    /// satisfies is satisfied by an agent in a hurry.
    #[test]
    fn a_section_with_too_little_text_is_refused() {
        let tmp = project();
        let msg = refusal(tmp.path(), "bug", &["cluster/alpha"], &fit("TBD"));
        assert!(msg.contains("## Class fit"), "got: {msg}");
    }

    /// Kills a floor that counts punctuation and whitespace as text.
    #[test]
    fn punctuation_and_whitespace_do_not_count_as_text() {
        let tmp = project();
        let pad = "... --- ... --- ... --- ... --- ... --- ... --- ... --- ... ---";
        let msg = refusal(tmp.path(), "bug", &["cluster/alpha"], &fit(pad));
        assert!(msg.contains("## Class fit"), "got: {msg}");
    }

    /// Kills a heading scan that is not fence-aware. 32 `## ` lines sit inside code fences across 15 of
    /// 1,131 bug files (measured 2026-10-16), so a naive scan is wrong on real input.
    #[test]
    fn a_heading_inside_a_code_fence_is_not_a_section() {
        let tmp = project();
        let body = format!("# BUG: x\n\n```\n## Class fit\n\n{GOOD}\n```\n");
        let msg = refusal(tmp.path(), "bug", &["cluster/alpha"], &body);
        assert!(msg.contains("## Class fit"), "got: {msg}");
    }

    /// Kills a heading match on any depth: `### Class fit` is a subsection of something else.
    #[test]
    fn a_heading_one_level_deeper_is_not_the_section() {
        let tmp = project();
        let body = format!("# BUG: x\n\n### Class fit\n\n{GOOD}\n");
        let msg = refusal(tmp.path(), "bug", &["cluster/alpha"], &body);
        assert!(msg.contains("## Class fit"), "got: {msg}");
    }
    /// Kills a heading match that accepts a SHALLOWER heading. The heading scan's branch is entered only
    /// for `# ` and `## ` lines, so the `###` case above never reaches it and says nothing about this one:
    /// a bug whose title line happens to read `# Class fit` would otherwise carry the section for free.
    /// (Found by a mutation run on 2026-10-16: replacing the `## ` match with "any number of hashes"
    /// survived every test.)
    #[test]
    fn a_level_one_heading_is_not_the_section() {
        let tmp = project();
        let body = format!("# Class fit\n\n{GOOD}\n");
        let msg = refusal(tmp.path(), "bug", &["cluster/alpha"], &body);
        assert!(msg.contains("## Class fit"), "got: {msg}");
    }

    /// Kills a floor that reads past the section: text under the NEXT heading must not satisfy this one.
    #[test]
    fn text_under_the_next_heading_does_not_count() {
        let tmp = project();
        let body = format!("# BUG: x\n\n## Class fit\n\nshort\n\n## Fix\n\n{GOOD}\n");
        let msg = refusal(tmp.path(), "bug", &["cluster/alpha"], &body);
        assert!(msg.contains("## Class fit"), "got: {msg}");
    }

    /// The accept case. Other tags are allowed alongside the one cluster tag; the rule is "exactly one
    /// `cluster/…`", not "only one tag".
    #[test]
    fn a_complete_bug_is_accepted_and_other_tags_are_allowed() {
        let tmp = project();
        enforce_on_create(
            tmp.path(),
            "bug",
            &tags(&["ci", "cluster/alpha", "flake"]),
            &fit(GOOD),
        )
        .unwrap();
    }

    // --- A broken policy is loud ------------------------------------------------------------------

    /// Kills "a malformed file means no policy". A policy that silently stops applying is how a rule
    /// decays unnoticed.
    #[test]
    fn a_malformed_requirements_file_is_an_error_not_a_skip() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), REQUIREMENTS_REL, "[[require]]\nkind = 1\n");
        enforce_on_create(tmp.path(), "bug", &[], "x").expect_err("a broken policy must be loud");
    }

    /// Kills `deny_unknown_fields` being dropped: a typo'd key would otherwise disable a rule silently.
    #[test]
    fn an_unknown_key_in_the_requirements_file_is_an_error() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            REQUIREMENTS_REL,
            "[[require]]\nkind = \"bug\"\nsectoins = [\"x\"]\n",
        );
        enforce_on_create(tmp.path(), "bug", &[], "x").expect_err("an unknown key must be loud");
    }

    /// Kills "no declared values means every tag passes". If the ledger moved, the check cannot be made,
    /// and it must say so rather than wave every bug through.
    #[test]
    fn a_source_that_declares_nothing_makes_the_tag_check_loud() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), REQUIREMENTS_REL, POLICY);
        let msg = enforce_on_create(tmp.path(), "bug", &tags(&["cluster/alpha"]), &fit(GOOD))
            .expect_err("nothing declared: the tag cannot be checked")
            .to_string();
        // The message must be about the SOURCE, not about the tag. An earlier version accepted any message
        // containing "declare", which "`cluster/alpha` is not a declared value" also does: a code path that
        // reported the wrong error passed this test (a mutation run found it, 2026-10-16).
        assert!(msg.contains("cannot be checked"), "got: {msg}");
        assert!(
            msg.contains("ledger.md"),
            "the missing source must be named: {msg}"
        );
        assert!(
            !msg.contains("is not a declared value"),
            "this is a policy problem, not a bad tag: {msg}"
        );
    }
    /// Kills drift between this module's reading of the ledger and the commit hook's. They are two
    /// implementations of one parser (`valid_slugs` in `scripts/pre-commit-ledger-counts.py` is the
    /// other), and they judge the SAME tag: if they disagree, an agent is let through here and refused
    /// at commit, or the reverse. Compared on the LIVE ledger, not a fixture, because the hook's
    /// digit-dropping rule and the template placeholder only show on real input.
    ///
    /// Also a vacuity guard: two parsers that both read nothing agree perfectly.
    #[test]
    fn the_declared_values_agree_with_the_commit_hooks_on_the_live_ledger() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let roster = root.join("docs/trackers/issue-clusters.md");
        let classes = root.join("docs/trackers/issue-clusters");
        assert!(
            roster.is_file() && classes.is_dir(),
            "the live ledger must be present to compare against"
        );

        // The hook reads the roster and the per-class files as one concatenated text.
        let mut text = std::fs::read_to_string(&roster).unwrap();
        let mut kids: Vec<_> = std::fs::read_dir(&classes)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|k| k.extension().is_some_and(|x| x == "md"))
            .collect();
        kids.sort();
        for k in kids {
            text.push('\n');
            text.push_str(&std::fs::read_to_string(k).unwrap());
        }
        let mut child = Command::new("python3")
            .args([
                "-I",
                "-c",
                "import importlib.util, json, sys\n\
                 s = importlib.util.spec_from_file_location('hook', 'scripts/pre-commit-ledger-counts.py')\n\
                 m = importlib.util.module_from_spec(s)\n\
                 s.loader.exec_module(m)\n\
                 print(json.dumps(sorted(m.valid_slugs(sys.stdin.read()))))",
            ])
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("python3 failed to spawn");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "the hook's valid_slugs failed to run");
        let hook: BTreeSet<String> = serde_json::from_slice(&out.stdout).unwrap();

        let req: Requirement = toml::from_str(
            "kind = \"bug\"\ntag_prefix = \"cluster/\"\n\
             tag_values_from = [\"docs/trackers/issue-clusters.md\", \"docs/trackers/issue-clusters\"]\n\
             tag_declared_by = \"**Slug:**\"\n",
        )
        .unwrap();
        let here = declared_values(&req, root, "cluster/").unwrap();

        assert!(
            hook.len() > 10,
            "the hook found only {} values; the comparison would be vacuous",
            hook.len()
        );
        assert_eq!(
            here, hook,
            "the server and the commit hook disagree about the declared cluster values"
        );
    }
    /// The server and the commit hook count a section the same way, on a table of bodies whose answers
    /// are written out by hand.
    ///
    /// Both are judged against the literal, not only against each other: two parsers that share one
    /// mistake agree perfectly. Each row is a shape that one real mutation of the scan gets wrong, so
    /// the table is also what keeps the Python copy honest after an edit to either:
    ///
    /// - a heading inside a backtick fence or a tilde fence is not a section;
    /// - `###` and `#` do not open it, and an indented `## ` is not a heading at all;
    /// - the section ends at the NEXT H1 or H2, and of several same-title sections the largest counts
    ///   (the row with the real one in the middle is the one a "first size wins" mutant fails);
    /// - a carriage return, trailing spaces and U+2028 do not change the count: Rust's `lines()` does
    ///   not split on U+2028, which is why the hook uses `split("\n")` and not `splitlines()`.
    #[test]
    fn the_commit_hook_and_the_server_count_a_section_the_same_way() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let table: &[(&str, Option<usize>)] = &[
            ("", None),
            ("## Class fit\n", Some(0)),
            ("## Class fit\nabc def 123\n", Some(9)),
            ("```\n## Class fit\nabcdefghij\n```\n", None),
            ("~~~\n## Class fit\nabcdefghij\n~~~\n", None),
            ("  ```\n## Class fit\nabcd\n  ```\n", None),
            ("### Class fit\nabc\n", None),
            ("# Class fit\nabc\n", None),
            ("  ## Class fit\nabc\n", None),
            ("## Class fit\nab\n## Other\nabcdefgh\n", Some(2)),
            ("## Class fit\n\n## Class fit\nabcd\n", Some(4)),
            ("## Class fit\nabcd\n## X\nz\n## Class fit\n\n", Some(4)),
            (
                "## Class fit\n\n## Class fit\nabcd\n\n## Root cause\nx\n",
                Some(4),
            ),
            ("## Class fit\r\nabc\r\n", Some(3)),
            ("## Class fit   \nabc\n", Some(3)),
            ("##  Class fit\nabc\n", Some(3)),
            ("## Class fit\nab\u{2028}cd\n", Some(4)),
            (
                "## Class fit\n\u{c5}ngstr\u{f6}m \u{65e5}\u{672c}\u{8a9e} \u{661}\u{662}\u{663}\n",
                Some(14),
            ),
            ("## Class fit\n...---!!!\n", Some(0)),
        ];
        for (body, want) in table {
            assert_eq!(
                section_text_len(body, "Class fit"),
                *want,
                "the server miscounts {body:?}"
            );
        }

        let bodies: Vec<&str> = table.iter().map(|(b, _)| *b).collect();
        let mut child = Command::new("python3")
            .args([
                "-I",
                "-c",
                "import importlib.util, json, sys\n\
                 s = importlib.util.spec_from_file_location('hook', 'scripts/pre-commit-ledger-counts.py')\n\
                 m = importlib.util.module_from_spec(s)\n\
                 s.loader.exec_module(m)\n\
                 print(json.dumps([m.section_text_len(b, m.CLASS_FIT_TITLE) for b in json.load(sys.stdin)]))",
            ])
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("python3 failed to spawn");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(serde_json::to_string(&bodies).unwrap().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "the hook's section_text_len failed to run"
        );
        let hook: Vec<Option<usize>> = serde_json::from_slice(&out.stdout).unwrap();
        let want: Vec<Option<usize>> = table.iter().map(|(_, w)| *w).collect();
        assert_eq!(
            hook, want,
            "the commit hook counts a section differently from the server"
        );
    }

    /// The commit hook holds the section title and the floor as constants, and this repository's
    /// policy file holds them as data. They are one policy read by two programs, and a value changed
    /// on one side only makes `doc(create)` accept what the commit refuses, or the reverse.
    ///
    /// Compared on the LIVE policy file, and guarded against vacuity: a file that declares no `bug`
    /// requirement would make "the hook agrees with nothing" pass.
    #[test]
    fn the_commit_hook_and_the_requirements_file_agree_on_the_class_fit_policy() {
        use std::process::Command;

        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let reqs = load(root).expect("the repository's own requirements file must parse");
        let bug = reqs.iter().find(|r| r.kind == "bug").expect(
            "the repository must declare a `bug` requirement; the comparison would be vacuous",
        );
        assert_eq!(
            bug.sections.len(),
            1,
            "the hook enforces ONE section; a second would be enforced at `doc(create)` only"
        );

        let out = Command::new("python3")
            .args([
                "-I",
                "-c",
                "import importlib.util, json\n\
                 s = importlib.util.spec_from_file_location('hook', 'scripts/pre-commit-ledger-counts.py')\n\
                 m = importlib.util.module_from_spec(s)\n\
                 s.loader.exec_module(m)\n\
                 print(json.dumps([m.CLASS_FIT_TITLE, m.CLASS_FIT_MIN]))",
            ])
            .current_dir(root)
            .output()
            .expect("python3 failed to spawn");
        assert!(out.status.success(), "the hook failed to load");
        let (title, min): (String, usize) = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            (title, min),
            (bug.sections[0].clone(), bug.min_section_chars),
            "the commit hook and `.codescout/requirements.toml` disagree about the Class fit rule"
        );
    }
}
