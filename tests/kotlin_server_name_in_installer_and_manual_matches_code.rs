//! The Kotlin server codescout starts is the literal in the `"kotlin" =>` arm of
//! `default_config` (`src/lsp/servers/mod.rs`, `lsp_binary_name("...")`). The installer and the
//! manual are documents of that fact, and nothing re-read them against it:
//! `docs/issues/2026-10-17-install-lsp-and-manual-name-kotlin-language-server-but-codescout-starts-kotlin-lsp.md`
//! is the bug this closes. `scripts/install-lsp.sh` installed fwcd's `kotlin-language-server`
//! and three manual pages told the reader to look for it, while codescout started JetBrains'
//! `kotlin-lsp` and never looked for anything else.
//!
//! **Static, not live.** It reads the source and the documents; it does not run the installer
//! (which downloads and writes under `$HOME`) and it does not check that the archive URL or the
//! launcher path inside it are still right. Only an install on a clean machine reads those.
//!
//! **Scope of the ban:** the old name is forbidden in the files below only, because each of them
//! says which server codescout uses. Files that may legitimately explain the old name as history
//! (the changelog, archived issues, code comments recording what an old server did) are not read.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Files that state which Kotlin server codescout uses or installs.
const KOTLIN_SERVER_STATING_FILES: [&str; 5] = [
    "scripts/install-lsp.sh",
    "docs/manual/src/troubleshooting.md",
    "docs/manual/src/architecture.md",
    "docs/manual/src/language-support.md",
    "contrib/systemd/README.md",
];

const OLD_SERVER: &str = "kotlin-language-server";

/// The binary codescout starts for Kotlin, read from the code that starts it.
fn started_kotlin_binary() -> String {
    let src = read("src/lsp/servers/mod.rs");
    let arm = src
        .find("\"kotlin\" =>")
        .expect("src/lsp/servers/mod.rs has no `\"kotlin\" =>` arm; this test is looking in the wrong place");
    let re = regex::Regex::new(r#"lsp_binary_name\("([^"]+)"\)"#).unwrap();
    re.captures(&src[arm..])
        .expect("no lsp_binary_name(\"...\") after the `\"kotlin\" =>` arm")[1]
        .to_string()
}

#[test]
fn no_file_that_states_the_kotlin_server_names_the_one_codescout_does_not_start() {
    let started = started_kotlin_binary();
    assert_ne!(
        started, OLD_SERVER,
        "the code now starts {OLD_SERVER}; this test's premise is gone and it must be rewritten"
    );

    let mut offenders = Vec::new();
    for rel in KOTLIN_SERVER_STATING_FILES {
        let content = read(rel);
        assert!(!content.is_empty(), "{rel} is empty; the read is wrong");
        for (i, line) in content.lines().enumerate() {
            if line.contains(OLD_SERVER) {
                offenders.push(format!("{rel}:{}  {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these lines name {OLD_SERVER}, but codescout starts `{started}`:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_installer_maps_kotlin_to_the_started_binary_and_links_that_name_onto_path() {
    let started = started_kotlin_binary();
    let script = read("scripts/install-lsp.sh");

    // `binary_for` decides what `--check` looks for and what `install_lang` skips as present.
    let map = regex::Regex::new(r#"(?m)^\s*kotlin\)\s+echo "([^"]+)""#).unwrap();
    let mapped = map
        .captures(&script)
        .expect("scripts/install-lsp.sh `binary_for` has no `kotlin)  echo \"...\"` entry")[1]
        .to_string();
    assert_eq!(
        mapped, started,
        "`binary_for kotlin` names a binary codescout does not start"
    );

    // `install_kotlin` must put that name on PATH, or a successful install changes nothing.
    let link = format!(r#"ln -sf "${{launcher}}" "${{INSTALL_DIR}}/{started}""#);
    assert!(
        script.lines().any(|l| l.trim() == link),
        "install_kotlin does not link the launcher as `{started}`; expected the line: {link}"
    );
}

#[test]
fn the_architecture_table_and_the_troubleshooting_check_name_the_started_binary() {
    let started = started_kotlin_binary();

    let arch = read("docs/manual/src/architecture.md");
    let row = regex::Regex::new(r"(?m)^\|\s*Kotlin\s*\|\s*`([^`]+)`").unwrap();
    assert_eq!(
        &row.captures(&arch)
            .expect("architecture.md has no `| Kotlin | `server` |` row")[1],
        started.as_str()
    );

    let trouble = read("docs/manual/src/troubleshooting.md");
    assert!(
        trouble
            .lines()
            .any(|l| l.trim() == format!("which {started}")),
        "troubleshooting.md's Kotlin check is not `which {started}`"
    );
    assert!(
        trouble
            .lines()
            .any(|l| l.starts_with(&format!("| `{started}` |"))),
        "troubleshooting.md's startup-time table has no `{started}` row"
    );
}
