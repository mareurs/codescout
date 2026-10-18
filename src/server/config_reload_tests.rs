//! A mid-session `project.toml` edit that FAILS TO LOAD must not leave a policy looser than
//! the file asks for, and must say so.
//!
//! Every test here drives the real dispatch (`call_tool_inner`, the one `tools/call` takes),
//! because the write gate (`check_tool_access`) and the notice both live in that path and a
//! tool's own `call()` meets neither. The edit is made with `std::fs::write` on purpose: an
//! out-of-band edit reaches the server through `ActiveProject::config_is_stale`, which is the
//! path under test; routing it through `edit_file` would take the eager reload instead.
//!
//! docs/issues/2026-10-17-an-invalid-project-toml-reload-is-ignored-silently-so-a-tightening-saved-with-it-never-applies.md

use super::test_support::make_server_with_project_toml;
use super::*;
use serde_json::json;

const BASE_TOML: &str = "[project]\nname = \"t\"\n";

/// One dispatched call: `(is_error, all text blocks joined)`.
pub(super) async fn call(server: &CodeScoutServer, name: &str, args: Value) -> (bool, String) {
    let req: CallToolRequestParams = serde_json::from_value(json!({
        "name": name,
        "arguments": args,
    }))
    .unwrap();
    let result = server
        .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
        .await
        .unwrap();
    let text = result
        .content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n");
    (result.is_error == Some(true), text)
}

fn project_toml(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().join(".codescout").join("project.toml")
}

/// The issue's scenario B4, through the real dispatch: writes work, then `project.toml` is
/// rewritten with `file_write_enabled = false` AND an `extra_write_roots` entry the loader
/// refuses (`~` is not expanded). Before the fix the whole reload failed, the previous
/// config stayed, and `b.txt` was written.
#[tokio::test]
async fn a_tightening_saved_with_an_invalid_entry_still_disables_writes() {
    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;

    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": "a.txt", "content": "a\n"}),
    )
    .await;
    assert!(
        !err && dir.path().join("a.txt").exists(),
        "control: the project must START writable, or the flip below proves nothing: {text}"
    );

    std::fs::write(
        project_toml(&dir),
        "[project]\nname = \"t\"\n\n[security]\nfile_write_enabled = false\n\
         extra_write_roots = [\"~/scratch\"]\n",
    )
    .unwrap();

    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": "b.txt", "content": "b\n"}),
    )
    .await;
    assert!(
        err,
        "writes were turned off in the same save as the invalid entry; the call must be \
         refused. Got: {text}"
    );
    assert!(
        !dir.path().join("b.txt").exists(),
        "b.txt was written although the saved file says file_write_enabled = false"
    );
}

/// Rewrite `project.toml` out of band. The stamp is `(mtime, len)`, and a filesystem with a
/// coarse mtime tick can give two quick writes the same mtime, so a same-length rewrite could
/// be invisible. Refuse to run such a test rather than let it pass or fail for that reason.
fn save(dir: &tempfile::TempDir, text: &str) {
    let path = project_toml(dir);
    let before = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    assert_ne!(
        before,
        text.len() as u64,
        "test bug: the rewrite must change the file's length, or the staleness stamp may not move"
    );
    std::fs::write(path, text).unwrap();
}

/// `[security]` body appended to the base `[project]` table.
fn with_security(body: &str) -> String {
    format!("{BASE_TOML}\n[security]\n{body}")
}

/// An `extra_write_roots` entry the loader refuses (relative), used to make the reload fail
/// while the rest of the file stays well-formed TOML.
const BAD_ENTRY: &str = "extra_write_roots = [\"relative/dir\"]\n";

async fn security(server: &CodeScoutServer) -> crate::util::path_security::PathSecurityConfig {
    server.agent.security_config().await
}

/// The failure must reach the NEXT tool response, on the channel callers already read
/// (`_workspace_notice`), and stay there while the file stays broken: the stamp advances on
/// a failed reload, so a one-shot notice would be gone before anyone fixed anything.
#[tokio::test]
async fn a_failed_reload_is_reported_on_every_response_until_the_file_loads() {
    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    let (_, before) = call(&server, "tree", json!({"path": "."})).await;
    assert!(
        !before.contains("FAILED TO LOAD"),
        "control: a healthy project carries no config notice: {before}"
    );

    save(&dir, &with_security(BAD_ENTRY));
    for round in 0..2 {
        let (_, text) = call(&server, "tree", json!({"path": "."})).await;
        assert!(
            text.contains("FAILED TO LOAD") && text.contains("extra_write_roots entry 0"),
            "round {round}: the response must say project.toml failed and why: {text}"
        );
    }

    save(&dir, &with_security("shell_command_mode = \"warn\"\n"));
    let (_, after) = call(&server, "tree", json!({"path": "."})).await;
    assert!(
        !after.contains("FAILED TO LOAD"),
        "a file that loads again must clear the notice: {after}"
    );
}

/// A refused write must carry the load error too: under the unparseable-file fail-closed the
/// refusal is the only text a writer sees, and on its own it blames a setting the file does
/// not contain.
#[tokio::test]
async fn a_write_refused_because_the_file_failed_to_load_says_so() {
    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    save(&dir, "[project\nname = \"t\"\n");
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": "c.txt", "content": "c\n"}),
    )
    .await;
    assert!(
        err && !dir.path().join("c.txt").exists(),
        "an unparseable project.toml must fail closed for writes: {text}"
    );
    assert!(
        text.contains("FAILED TO LOAD"),
        "the refusal must say the file failed to load, not only that writes are off: {text}"
    );
}

/// A file too large to read is unreadable in the same sense as a syntax error, and takes the
/// same fail-closed path. One test per way the read can fail: the size check is its own site.
#[tokio::test]
async fn an_oversized_project_toml_fails_closed_for_writes() {
    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    let big = format!("{BASE_TOML}# {}\n", "x".repeat(1024 * 1024 + 16));
    save(&dir, &big);
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": "d.txt", "content": "d\n"}),
    )
    .await;
    assert!(
        err && !dir.path().join("d.txt").exists(),
        "an unreadable project.toml must fail closed for writes: {text}"
    );
}

#[tokio::test]
async fn an_invalid_reload_still_applies_indexing_enabled_false() {
    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    assert!(security(&server).await.indexing_enabled, "control");
    save(
        &dir,
        &with_security(&format!("indexing_enabled = false\n{BAD_ENTRY}")),
    );
    assert!(
        !security(&server).await.indexing_enabled,
        "indexing was turned off in the same save as the invalid entry"
    );
}

#[tokio::test]
async fn an_invalid_reload_still_applies_a_stricter_shell_mode() {
    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    assert_eq!(
        security(&server).await.shell_command_mode,
        "warn",
        "control"
    );
    save(
        &dir,
        &with_security(&format!("shell_command_mode = \"disabled\"\n{BAD_ENTRY}")),
    );
    assert_eq!(
        security(&server).await.shell_command_mode,
        "disabled",
        "the shell was disabled in the same save as the invalid entry"
    );
}

/// `unrestricted -> warn` is a tightening too, and it is a different branch from `-> disabled`.
#[tokio::test]
async fn an_invalid_reload_applies_warn_over_unrestricted() {
    let (dir, server) = make_server_with_project_toml(Some(&with_security(
        "shell_command_mode = \"unrestricted\"\n",
    )))
    .await;
    assert_eq!(
        security(&server).await.shell_command_mode,
        "unrestricted",
        "control"
    );
    save(
        &dir,
        &with_security(&format!("shell_command_mode = \"warn\"\n{BAD_ENTRY}\n")),
    );
    assert_eq!(security(&server).await.shell_command_mode, "warn");
}

/// A value the file gives that cannot be read counts as the strictest, not as "no change".
#[tokio::test]
async fn an_invalid_reload_reads_an_unknown_shell_mode_as_disabled() {
    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    save(
        &dir,
        &with_security(&format!("shell_command_mode = \"strict\"\n{BAD_ENTRY}")),
    );
    assert_eq!(security(&server).await.shell_command_mode, "disabled");
}

/// The direction guard: an invalid reload must never LOOSEN. `warn -> unrestricted` saved with
/// an invalid entry stays `warn`.
#[tokio::test]
async fn an_invalid_reload_never_loosens_the_shell_mode() {
    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    save(
        &dir,
        &with_security(&format!(
            "shell_command_mode = \"unrestricted\"\n{BAD_ENTRY}\n"
        )),
    );
    assert_eq!(
        security(&server).await.shell_command_mode,
        "warn",
        "a loosening saved with an invalid entry must not apply"
    );
}

/// `profile = "root"` turns every gate off, so leaving it is the most valuable tightening and
/// the file must still CONFIRM it to keep it: dropping the key, or writing anything other than
/// "root", ends it.
#[tokio::test]
async fn an_invalid_reload_ends_the_root_profile_unless_the_file_confirms_it() {
    use crate::util::path_security::SecurityProfile::{Default as Sandboxed, Root};
    // One server per case: a tightened policy stays tightened until a reload succeeds, so a
    // later case on the same server could not observe `Root` again.
    let cases: [(&str, String, _); 4] = [
        (
            "the file still says root: kept",
            format!("profile = \"root\"\n{BAD_ENTRY}"),
            Root,
        ),
        (
            "explicit default",
            format!("profile = \"default\"\n{BAD_ENTRY}"),
            Sandboxed,
        ),
        (
            "a deleted profile key is the default profile",
            BAD_ENTRY.to_string(),
            Sandboxed,
        ),
        (
            "a value that is not a recognisable profile is not root",
            format!("profile = \"admin\"\n{BAD_ENTRY}"),
            Sandboxed,
        ),
    ];
    for (label, body, expected) in cases {
        let (dir, server) =
            make_server_with_project_toml(Some(&with_security("profile = \"root\"\n"))).await;
        assert_eq!(security(&server).await.profile, Root, "{label}: control");
        save(&dir, &with_security(&body));
        assert_eq!(security(&server).await.profile, expected, "{label}");
    }
}

/// Narrowing `extra_write_roots` applies; widening does not. The invalid entry is dropped
/// along with everything the file added.
#[tokio::test]
async fn an_invalid_reload_keeps_only_the_write_roots_the_file_still_lists() {
    let (dir, server) = make_server_with_project_toml(Some(&with_security(
        "extra_write_roots = [\"/probe/a\", \"/probe/b\"]\n",
    )))
    .await;
    let roots = |c: crate::util::path_security::PathSecurityConfig| {
        c.extra_write_roots
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        roots(security(&server).await),
        ["/probe/a", "/probe/b"],
        "control"
    );

    save(
        &dir,
        &with_security("extra_write_roots = [\"/probe/a\", \"/probe/c\", \"relative/dir\"]\n"),
    );
    assert_eq!(
        roots(security(&server).await),
        ["/probe/a"],
        "/probe/b was removed (narrowing applies); /probe/c was added (widening must not)"
    );

    // A key that is not a list of strings confirms nothing.
    save(&dir, &with_security("extra_write_roots = \"/probe/a\"\n\n"));
    assert!(
        roots(security(&server).await).is_empty(),
        "an unreadable extra_write_roots confirms no root"
    );
}

/// A value the file gives for a gate that is not a boolean cannot be read as "open", so it
/// counts as closed. Both gates, because each names its own key.
#[tokio::test]
async fn a_non_boolean_gate_value_counts_as_closed() {
    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    save(
        &dir,
        &with_security(&format!("file_write_enabled = \"yes\"\n{BAD_ENTRY}")),
    );
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": "e.txt", "content": "e\n"}),
    )
    .await;
    assert!(
        err && !dir.path().join("e.txt").exists(),
        "an unreadable file_write_enabled must not leave writes open: {text}"
    );

    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    save(
        &dir,
        &with_security(&format!("indexing_enabled = 0\n{BAD_ENTRY}")),
    );
    assert!(
        !security(&server).await.indexing_enabled,
        "an unreadable indexing_enabled must not leave indexing open"
    );
}

/// With no readable file nothing can be confirmed: not a `root` profile, not a write root.
/// (The write gate for this case is pinned by
/// `a_write_refused_because_the_file_failed_to_load_says_so`.)
#[tokio::test]
async fn an_unparseable_project_toml_confirms_no_root_profile_and_no_write_roots() {
    let (dir, server) = make_server_with_project_toml(Some(&with_security(
        "profile = \"root\"\nextra_write_roots = [\"/probe/a\"]\n",
    )))
    .await;
    let before = security(&server).await;
    assert!(
        before.profile == crate::util::path_security::SecurityProfile::Root
            && before.extra_write_roots.len() == 1,
        "control"
    );
    save(&dir, "[security\nprofile = \"root\"\n");
    let after = security(&server).await;
    assert_eq!(
        after.profile,
        crate::util::path_security::SecurityProfile::Default
    );
    assert!(after.extra_write_roots.is_empty());
}

/// The direction guard for the boolean gates, which the shell-mode one above does not reach:
/// once an invalid reload has closed a gate, a later invalid reload that says "open" must not
/// reopen it. Only a reload that LOADS replaces the policy.
#[tokio::test]
async fn an_invalid_reload_never_reopens_a_closed_gate() {
    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    save(
        &dir,
        &with_security(&format!("indexing_enabled = false\n{BAD_ENTRY}")),
    );
    assert!(!security(&server).await.indexing_enabled, "control");
    save(
        &dir,
        &with_security(&format!("indexing_enabled = true\n{BAD_ENTRY}\n\n")),
    );
    assert!(
        !security(&server).await.indexing_enabled,
        "a loosening saved with an invalid entry must not reopen a closed gate"
    );
}

/// An invalid reload that does not mention a gate leaves it as it was: the salvage closes
/// only what the file asks to close (or cannot be read as open).
#[tokio::test]
async fn an_invalid_reload_leaves_gates_the_file_does_not_mention_alone() {
    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    save(&dir, &with_security(BAD_ENTRY));
    let after = security(&server).await;
    assert!(
        after.file_write_enabled && after.indexing_enabled,
        "an unrelated invalid entry must not close gates the file never touched"
    );
    assert_eq!(after.shell_command_mode, "warn");
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": "f.txt", "content": "f\n"}),
    )
    .await;
    assert!(!err, "writes stay allowed: {text}");
}

#[tokio::test]
async fn an_invalid_reload_reads_a_non_string_shell_mode_as_disabled() {
    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    save(
        &dir,
        &with_security(&format!("shell_command_mode = 5\n{BAD_ENTRY}")),
    );
    assert_eq!(security(&server).await.shell_command_mode, "disabled");
}

/// The notice describes the policy THIS call ran under, so it follows the call's
/// `workspace=` pin to the project that call resolved to: a pinned call into a broken
/// project carries it, and an unpinned call into a healthy home project does not.
#[tokio::test]
async fn a_pinned_call_carries_the_failed_reload_notice_too() {
    let (_home, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    let pinned = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(pinned.path().join(".codescout")).unwrap();
    std::fs::write(project_toml(&pinned), BASE_TOML).unwrap();
    let root = std::fs::canonicalize(pinned.path()).unwrap();
    let pin = |p: &str| json!({"path": p, "workspace": root.to_string_lossy()});

    // Resident and healthy first, so the edit below is a RELOAD, not a first load.
    let (_, healthy) = call(&server, "tree", pin(".")).await;
    assert!(!healthy.contains("FAILED TO LOAD"), "control: {healthy}");

    save(&pinned, &with_security(BAD_ENTRY));
    let (_, text) = call(&server, "tree", pin(".")).await;
    assert!(
        text.contains("FAILED TO LOAD"),
        "a pinned call must carry its own project's notice: {text}"
    );
    let (_, unpinned) = call(&server, "tree", json!({"path": "."})).await;
    assert!(
        !unpinned.contains("FAILED TO LOAD"),
        "the home project is healthy: another project's failure must not leak onto it: {unpinned}"
    );
}

/// The refusal's load error is read for the project the call resolved to, like the notice.
#[tokio::test]
async fn a_pinned_write_refusal_carries_the_pinned_projects_load_error() {
    let (_home, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
    let pinned = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(pinned.path().join(".codescout")).unwrap();
    std::fs::write(project_toml(&pinned), BASE_TOML).unwrap();
    let root = std::fs::canonicalize(pinned.path()).unwrap();
    let write =
        |name: &str| json!({"path": name, "content": "x\n", "workspace": root.to_string_lossy()});

    let (err, text) = call(&server, "create_file", write("ok.txt")).await;
    assert!(
        !err && root.join("ok.txt").exists(),
        "control: a pinned write into the healthy project lands: {text}"
    );

    save(&pinned, "[project\nname = \"p\"\n");
    let (err, text) = call(&server, "create_file", write("no.txt")).await;
    assert!(
        err && !root.join("no.txt").exists(),
        "the pinned project's unparseable file must refuse its writes: {text}"
    );
    assert!(
        text.contains("FAILED TO LOAD"),
        "the refusal must carry the PINNED project's load error: {text}"
    );
}

/// A mode the dispatcher does not know is already refused by it, so it is the strictest
/// state there is; an invalid reload must not replace it with a looser known one.
#[tokio::test]
async fn an_invalid_reload_never_replaces_an_unknown_shell_mode_with_a_known_one() {
    let (dir, server) =
        make_server_with_project_toml(Some(&with_security("shell_command_mode = \"strict\"\n")))
            .await;
    assert_eq!(
        security(&server).await.shell_command_mode,
        "strict",
        "control"
    );
    save(
        &dir,
        &with_security(&format!("shell_command_mode = \"warn\"\n{BAD_ENTRY}")),
    );
    assert_eq!(security(&server).await.shell_command_mode, "strict");
}

/// F4. Deleting a looser value in the same save as an invalid entry must not keep it: with the
/// key absent the file means the built-in default, and the policy may never be looser than
/// what the file would give with the invalid entry absent.
#[tokio::test]
async fn an_invalid_reload_that_deletes_a_looser_shell_mode_reads_the_default() {
    let (dir, server) = make_server_with_project_toml(Some(&with_security(
        "shell_command_mode = \"unrestricted\"\n",
    )))
    .await;
    assert_eq!(
        security(&server).await.shell_command_mode,
        "unrestricted",
        "control"
    );
    save(&dir, &with_security(&format!("{BAD_ENTRY}\n")));
    assert_eq!(
        security(&server).await.shell_command_mode,
        "warn",
        "the file without the invalid entry means the default, warn"
    );
}

/// F4 with a GLOBAL config that is stricter than the project's own looser values: the project
/// overrides it, then the keys are deleted in a save that fails to load. An absent key reads
/// as the global value. Child process: the global path comes from the environment.
#[cfg(unix)]
#[test]
fn an_invalid_reload_that_deletes_a_looser_key_reads_the_global_value() {
    let fixture = tempfile::tempdir().unwrap();
    let xdg = fixture.path().canonicalize().unwrap().join("xdg");
    crate::platform::test_child::run(
        module_path!(),
        "an_invalid_reload_that_deletes_a_looser_key_reads_the_global_value",
        &[("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap()))],
        || {
            let global_dir = std::path::PathBuf::from(std::env::var("XDG_CONFIG_HOME").unwrap())
                .join("codescout");
            std::fs::create_dir_all(&global_dir).unwrap();
            std::fs::write(
                global_dir.join("config.toml"),
                "[security]\nshell_command_mode = \"disabled\"\nfile_write_enabled = false\nindexing_enabled = false\n",
            )
            .unwrap();
            let loose = with_security(
                "shell_command_mode = \"unrestricted\"\nfile_write_enabled = true\nindexing_enabled = true\n",
            );
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (dir, server) = make_server_with_project_toml(Some(&loose)).await;
                    let before = security(&server).await;
                    assert!(
                        before.shell_command_mode == "unrestricted"
                            && before.file_write_enabled
                            && before.indexing_enabled,
                        "control: the project overrides the stricter global config"
                    );
                    save(&dir, &with_security(&format!("{BAD_ENTRY}\n")));
                    let after = security(&server).await;
                    assert_eq!(after.shell_command_mode, "disabled", "shell");
                    assert!(!after.file_write_enabled, "file_write_enabled");
                    assert!(!after.indexing_enabled, "indexing_enabled");
                });
        },
    );
}

/// A global config that cannot be read counts as the strictest value for a key the file does
/// not set, on all three keys.
#[cfg(unix)]
#[test]
fn an_unreadable_global_config_counts_as_the_strictest_value() {
    let fixture = tempfile::tempdir().unwrap();
    let xdg = fixture.path().canonicalize().unwrap().join("xdg");
    crate::platform::test_child::run(
        module_path!(),
        "an_unreadable_global_config_counts_as_the_strictest_value",
        &[("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap()))],
        || {
            let global_dir = std::path::PathBuf::from(std::env::var("XDG_CONFIG_HOME").unwrap())
                .join("codescout");
            std::fs::create_dir_all(&global_dir).unwrap();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (dir, server) = make_server_with_project_toml(Some(BASE_TOML)).await;
                    std::fs::write(global_dir.join("config.toml"), "not = valid = toml").unwrap();
                    save(&dir, &with_security(BAD_ENTRY));
                    let after = security(&server).await;
                    assert_eq!(after.shell_command_mode, "disabled", "shell");
                    assert!(!after.file_write_enabled, "file_write_enabled");
                    assert!(!after.indexing_enabled, "indexing_enabled");
                });
        },
    );
}
