//! A tool write must not change the settings that decide what the tools may do.
//!
//! `<project>/.codescout/project.toml` sits under the always-writable project root and holds
//! `[security]` (`profile`, `extra_write_roots`, `file_write_enabled`, `shell_command_mode`).
//! The config is re-read when the file changes (`ActiveProject::config_is_stale`), so a write
//! tool that could edit it could raise its own privileges from the next call. Every test here
//! drives the real dispatch (`call_tool_inner`), because the refusal has to hold for the tool
//! as a caller meets it; the file is compared byte for byte afterwards, because a refused call
//! and a write that landed look the same to anything weaker.
//!
//! Not everything in the file is protected: `[embeddings]` is edited through `edit_file` by the
//! onboarding prompt's embedding-model step, and that flow must keep working (the controls below).
//!
//! docs/issues/2026-10-17-project-toml-under-the-writable-project-root-can-set-security-profile-root.md

use super::config_reload_tests::call;
use super::test_support::make_server_with_project_toml;
use serde_json::json;

const PROJECT_TOML: &str = "[project]\nname = \"t\"\n\n[embeddings]\nmodel = \"local:AllMiniLML6V2Q\"\n\n[security]\nshell_command_mode = \"warn\"\n";

const ROOT_PROFILE: &str = "[project]\nname = \"t\"\n\n[security]\nprofile = \"root\"\n";

fn toml_path(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().join(".codescout").join("project.toml")
}

fn on_disk(dir: &tempfile::TempDir) -> String {
    std::fs::read_to_string(toml_path(dir)).unwrap()
}

/// The refusal must be a failure the caller sees, and the file must be exactly as it was.
fn assert_refused_and_unchanged(dir: &tempfile::TempDir, before: &str, err: bool, text: &str) {
    assert_eq!(
        on_disk(dir),
        before,
        "project.toml changed through a write tool: {text}"
    );
    assert!(
        err,
        "the call must fail loudly, not report success on a write it did not make: {text}"
    );
}

/// The issue's own reproduction, end to end: an outside-root write asks for approval; after a
/// tool writes `profile = "root"` into project.toml it must STILL ask.
#[tokio::test]
async fn a_tool_cannot_raise_its_own_profile_through_project_toml() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    let outside = tempfile::Builder::new()
        .prefix("cs-root-profile-")
        .tempdir_in(
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .unwrap(),
        )
        .unwrap();
    let target = outside.path().join("probe.txt");
    let write_outside = || json!({"path": target.to_string_lossy(), "content": "x\n"});

    let (_, text) = call(&server, "create_file", write_outside()).await;
    assert!(
        text.contains("pending_ack") && !target.exists(),
        "control: an outside-root write must ask for approval first: {text}"
    );

    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": ".codescout/project.toml", "content": ROOT_PROFILE, "overwrite": true}),
    )
    .await;
    assert_refused_and_unchanged(&dir, PROJECT_TOML, err, &text);

    let (_, text) = call(&server, "create_file", write_outside()).await;
    assert!(
        text.contains("pending_ack") && !target.exists(),
        "the outside-root write must still ask for approval: {text}"
    );
}

#[tokio::test]
async fn create_file_cannot_rewrite_the_security_section() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": ".codescout/project.toml", "content": ROOT_PROFILE, "overwrite": true}),
    )
    .await;
    assert_refused_and_unchanged(&dir, PROJECT_TOML, err, &text);
    assert!(
        text.contains("editor"),
        "the refusal must send the reader to the one step that is theirs to take: {text}"
    );
}

#[tokio::test]
async fn edit_file_text_edit_cannot_change_the_security_section() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    let (err, text) = call(
        &server,
        "edit_file",
        json!({
            "path": ".codescout/project.toml",
            "old_string": "[security]\n",
            "new_string": "[security]\nprofile = \"root\"\n",
        }),
    )
    .await;
    assert_refused_and_unchanged(&dir, PROJECT_TOML, err, &text);
}

#[tokio::test]
async fn edit_file_append_cannot_change_the_security_section() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    // `[security]` is the last table, so an appended key lands in it.
    let (err, text) = call(
        &server,
        "edit_file",
        json!({
            "path": ".codescout/project.toml",
            "insert": "append",
            "new_string": "profile = \"root\"\n",
        }),
    )
    .await;
    assert_refused_and_unchanged(&dir, PROJECT_TOML, err, &text);
}

#[tokio::test]
async fn edit_file_batch_cannot_change_the_security_section() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    let (err, text) = call(
        &server,
        "edit_file",
        json!({
            "path": ".codescout/project.toml",
            "edits": [
                {"old_string": "name = \"t\"", "new_string": "name = \"u\""},
                {"old_string": "shell_command_mode = \"warn\"", "new_string": "shell_command_mode = \"unrestricted\""},
            ],
        }),
    )
    .await;
    assert_refused_and_unchanged(&dir, PROJECT_TOML, err, &text);
}

/// A write that does not parse as TOML cannot be shown to leave `[security]` alone, so it is
/// refused too; and a refusal is not the reload failure that writing it would have caused.
#[tokio::test]
async fn a_write_that_is_not_toml_is_refused() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": ".codescout/project.toml", "content": "[security\nprofile = \"root\"\n", "overwrite": true}),
    )
    .await;
    assert_refused_and_unchanged(&dir, PROJECT_TOML, err, &text);
}

/// Creating the file where none exists is the same question with an empty "before".
#[tokio::test]
async fn creating_project_toml_with_a_security_section_is_refused_without_one_allowed() {
    let (dir, server) = make_server_with_project_toml(None).await;
    assert!(!toml_path(&dir).exists(), "control: no project.toml yet");
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": ".codescout/project.toml", "content": ROOT_PROFILE}),
    )
    .await;
    assert!(
        err && !toml_path(&dir).exists(),
        "a created project.toml must not carry a [security] section: {text}"
    );

    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": ".codescout/project.toml", "content": "[project]\nname = \"t\"\n"}),
    )
    .await;
    assert!(
        !err && toml_path(&dir).exists(),
        "a project.toml with no [security] section is an ordinary file: {text}"
    );
}

/// The onboarding prompt's embedding-model step tells the model to `edit_file`
/// `.codescout/project.toml` and change `model`. That flow must keep working, and the new
/// model must reach the running session (`reload_config_if_project_toml_for`).
#[tokio::test]
async fn the_onboarding_embedding_model_edit_still_works_and_reloads() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    let (err, text) = call(
        &server,
        "edit_file",
        json!({
            "path": ".codescout/project.toml",
            "old_string": "model = \"local:AllMiniLML6V2Q\"",
            "new_string": "model = \"local:AllMiniLML6V2Q\"\nmax_inflight = 4",
        }),
    )
    .await;
    assert!(!err, "an [embeddings] edit must be allowed: {text}");
    assert!(
        on_disk(&dir).contains("max_inflight = 4"),
        "the edit did not land"
    );
    // `max_inflight`, not `model`: an env override (`CODESCOUT_EMBED_MODEL`) wins over the file
    // for the model on a developer machine, which would make this assertion about the env.
    let inflight = server
        .agent
        .with_project(|p| Ok(p.config.embeddings.max_inflight))
        .await
        .unwrap();
    assert_eq!(inflight, Some(4), "the edit must reach the running session");
}

/// A rewrite that leaves `[security]` exactly as it is (here: a comment, a reordered table and
/// an extra section) is not a security change, whatever else it touches.
#[tokio::test]
async fn a_rewrite_that_leaves_the_security_section_equal_is_allowed() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    let rewritten = "# reviewed\n[security]\nshell_command_mode = \"warn\"\n\n[project]\nname = \"t\"\n\n[embeddings]\nmodel = \"local:AllMiniLML6V2Q\"\n\n[libraries]\nauto_index = true\n";
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": ".codescout/project.toml", "content": rewritten, "overwrite": true}),
    )
    .await;
    assert!(!err, "{text}");
    assert_eq!(on_disk(&dir), rewritten);
}

/// Other files under `.codescout/` that tools legitimately write stay writable: the guard is
/// about the configuration files, not the directory.
#[tokio::test]
async fn ordinary_files_under_dot_codescout_are_still_writable() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": ".codescout/notes.txt", "content": "one\n"}),
    )
    .await;
    assert!(!err, "{text}");
    let (err, text) = call(
        &server,
        "edit_file",
        json!({
            "path": ".codescout/notes.txt",
            "old_string": "one",
            "new_string": "two",
        }),
    )
    .await;
    assert!(!err, "{text}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".codescout").join("notes.txt")).unwrap(),
        "two\n"
    );
    // A `project.toml` that is NOT the one directly inside `.codescout` is an ordinary file.
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": "docs/project.toml", "content": "[security]\nprofile = \"root\"\n"}),
    )
    .await;
    assert!(!err, "{text}");
}

/// The case the name rule has to survive on a case-insensitive filesystem.
#[tokio::test]
async fn the_protected_name_is_matched_ignoring_ascii_case() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": ".CodeScout/Project.TOML", "content": ROOT_PROFILE}),
    )
    .await;
    assert!(
        err && !dir.path().join(".CodeScout").join("Project.TOML").exists(),
        "a differently-cased spelling names the same file on some filesystems: {text}"
    );
}

/// `memory` writes under `.codescout/memories/`; a topic that climbs out of it must not reach
/// the config.
#[tokio::test]
async fn a_memory_topic_cannot_reach_project_toml() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    for topic in [
        "../project.toml",
        "../../.codescout/project.toml",
        "../project",
    ] {
        let (e, t) = call(
            &server,
            "memory",
            json!({"action": "write", "topic": topic, "content": ROOT_PROFILE}),
        )
        .await;
        let _ = (e, t);
        assert_eq!(
            on_disk(&dir),
            PROJECT_TOML,
            "memory topic {topic:?} reached project.toml"
        );
    }
}

/// The librarian's `doc` tool creates files from a repo-relative path. Measured: with a
/// project.toml already present it refuses ("path exists"); on a fresh project, where
/// create_file could write the file, `doc(create)` must not be a second way in.
#[cfg(feature = "librarian")]
#[tokio::test]
async fn doc_create_cannot_write_a_security_section_into_a_fresh_project_toml() {
    let (dir, server) = make_server_with_project_toml(None).await;
    let (e, t) = call(
        &server,
        "doc",
        json!({
            "action": "create",
            "kind": "note",
            "title": "t",
            "rel_path": ".codescout/project.toml",
            "body": ROOT_PROFILE,
        }),
    )
    .await;
    let landed = toml_path(&dir).exists()
        && std::fs::read_to_string(toml_path(&dir))
            .unwrap()
            .contains("profile");
    assert!(
        !landed,
        "doc(create) wrote a [security] section into project.toml (is_error={e}): {t}"
    );
}

/// `doc(move)` renames a catalogued file to a repo-relative path, so a plain markdown file
/// whose body is valid TOML could be moved onto `.codescout/project.toml` without a byte
/// passing through `atomic_write`. A project with no `project.toml` yet is the case where the
/// destination is free.
#[cfg(feature = "librarian")]
#[tokio::test]
async fn doc_move_cannot_turn_a_markdown_file_into_a_security_config() {
    let (dir, server) = make_server_with_project_toml(None).await;
    let (_, created) = call(
        &server,
        "doc",
        json!({"action": "create", "kind": "note", "title": "t", "rel_path": "staging.md", "body": "x"}),
    )
    .await;
    let id = serde_json::from_str::<serde_json::Value>(
        created.split("\n<!--").next().unwrap_or(&created),
    )
    .ok()
    .and_then(|v| v["id"].as_str().map(str::to_string))
    .unwrap_or_else(|| panic!("no id in doc(create) result: {created}"));
    // Replace the whole file with TOML, frontmatter and all: the catalog row stays.
    std::fs::write(dir.path().join("staging.md"), ROOT_PROFILE).unwrap();

    let (e, t) = call(
        &server,
        "doc",
        json!({"action": "move", "id": id, "new_rel_path": ".codescout/project.toml"}),
    )
    .await;
    let landed = toml_path(&dir).exists()
        && std::fs::read_to_string(toml_path(&dir))
            .unwrap()
            .contains("profile");
    assert!(
        !landed,
        "doc(move) put a [security] section into project.toml (is_error={e}): {t}"
    );
}

/// The global config (`$XDG_CONFIG_HOME/codescout/config.toml`) carries `[security]` too and
/// is merged under every project. A project may list its directory as a write root, which is
/// how a tool write can reach it without an approval. Driven in a child process because the
/// global path comes from the environment.
#[cfg(unix)]
#[test]
fn a_tool_cannot_change_the_global_configs_security_section() {
    let fixture = tempfile::tempdir().unwrap();
    let xdg = fixture.path().canonicalize().unwrap().join("xdg");
    crate::platform::test_child::run(
        module_path!(),
        "a_tool_cannot_change_the_global_configs_security_section",
        &[("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap()))],
        || {
            let xdg = std::path::PathBuf::from(std::env::var("XDG_CONFIG_HOME").unwrap());
            let global_dir = xdg.join("codescout");
            std::fs::create_dir_all(&global_dir).unwrap();
            let global = global_dir.join("config.toml");
            let before = "[security]\nshell_command_mode = \"warn\"\n";
            std::fs::write(&global, before).unwrap();
            let project = format!(
                "[project]\nname = \"t\"\n\n[security]\nextra_write_roots = [{:?}]\n",
                global_dir.to_str().unwrap()
            );
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (_dir, server) = make_server_with_project_toml(Some(&project)).await;
                    let (err, text) = call(
                        &server,
                        "create_file",
                        json!({
                            "path": global.to_string_lossy(),
                            "content": "[security]\nshell_command_mode = \"unrestricted\"\n",
                            "overwrite": true,
                        }),
                    )
                    .await;
                    assert!(err, "writing the global config's [security] must fail: {text}");
                    assert_eq!(std::fs::read_to_string(&global).unwrap(), before);

                    // The same file, with [security] untouched, is an ordinary write.
                    let (err, text) = call(
                        &server,
                        "create_file",
                        json!({
                            "path": global.to_string_lossy(),
                            "content": "[security]\nshell_command_mode = \"warn\"\n\n[embeddings]\nmodel = \"x\"\n",
                            "overwrite": true,
                        }),
                    )
                    .await;
                    assert!(!err, "{text}");
                });
        },
    );
}

fn profile_of(
    server: &super::CodeScoutServer,
) -> impl std::future::Future<Output = crate::util::path_security::SecurityProfile> + '_ {
    async move { server.agent.security_config().await.profile }
}

/// `.codescout` can be a link to a directory with ANOTHER name. The write tools hand the guard
/// a path whose parent is already resolved (`cfg-real/project.toml`), so a match on the
/// spelling `.codescout/project.toml` never fires; the file has to be recognised by identity.
#[cfg(unix)]
#[tokio::test]
async fn a_dot_codescout_that_is_a_link_to_another_name_is_still_guarded() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    let real = dir.path().join("cfg-real");
    std::fs::rename(dir.path().join(".codescout"), &real).unwrap();
    std::os::unix::fs::symlink(&real, dir.path().join(".codescout")).unwrap();
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": ".codescout/project.toml", "content": ROOT_PROFILE, "overwrite": true}),
    )
    .await;
    assert_refused_and_unchanged(&dir, PROJECT_TOML, err, &text);
    assert_eq!(
        profile_of(&server).await,
        crate::util::path_security::SecurityProfile::Default
    );
}

/// `project.toml` can be a link to a file elsewhere; writing that file under ITS OWN name
/// changes what `project.toml` says without naming it.
#[cfg(unix)]
#[tokio::test]
async fn the_target_of_a_symlinked_project_toml_is_guarded_under_its_own_name() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    let shared = dir.path().join("shared.toml");
    std::fs::rename(toml_path(&dir), &shared).unwrap();
    std::os::unix::fs::symlink("../shared.toml", toml_path(&dir)).unwrap();
    let (err, text) = call(
        &server,
        "create_file",
        json!({"path": "shared.toml", "content": ROOT_PROFILE, "overwrite": true}),
    )
    .await;
    assert_eq!(
        std::fs::read_to_string(&shared).unwrap(),
        PROJECT_TOML,
        "the file project.toml links to changed through a write tool: {text}"
    );
    assert!(err, "the call must fail loudly: {text}");
    assert_eq!(
        profile_of(&server).await,
        crate::util::path_security::SecurityProfile::Default
    );
}

/// F1 probe. Under the DEFAULT `shell_command_mode = "warn"`, a shell redirect rewrites
/// project.toml; the guard is on the file-writing tools and does not see it.
#[tokio::test]
async fn probe_run_command_can_write_the_security_section() {
    let (dir, server) = make_server_with_project_toml(Some(PROJECT_TOML)).await;
    let before = profile_of(&server).await;
    let (err, text) = call(
        &server,
        "run_command",
        json!({"command": "printf '[project]\\nname = \"t\"\\n\\n[security]\\nprofile = \"root\"\\n' > .codescout/project.toml"}),
    )
    .await;
    let after = profile_of(&server).await;
    eprintln!(
        "F1 profile_before={before:?} run_command_is_error={err} profile_after={after:?}\n{text}\non_disk={:?}",
        on_disk(&dir)
    );
}
