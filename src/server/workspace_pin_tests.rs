// Included into `server::tests` (see the `include!` beside `result_text`): these drive the
// REAL dispatch (`call_tool_inner`) and use that module's private `make_server`, `dispatch_tool`
// and `result_text`.
//
// What they guard: a `workspace=` pin that does not name an existing directory is REFUSED, and
// the refusal creates NOTHING on disk. Before 2026-10-18 the pin was kept as a raw (possibly
// relative) root and the first touch built a project there, leaving `<value>/.codescout/` behind.
// docs/issues/2026-10-18-a-workspace-pin-naming-a-path-that-does-not-exist-creates-a-project-there.md
//
// Isolation: no test here changes the process cwd in the shared test process. The ones that must
// (a RELATIVE pin resolves against the cwd) run in a child process (`platform::test_child`), the
// documented pattern in docs/conventions/test-env-isolation.md, with the cwd set to a tempdir in
// the CHILD only. A relative pin therefore cannot create `x/` in the repo even if the guard
// is removed.

/// Every path under `root`, relative to it, sorted. "Created nothing" is a statement about the
/// whole tree, so the tests compare listings rather than probing one expected path.
fn tree_listing(root: &std::path::Path) -> Vec<String> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            out.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
            if entry.file_type().unwrap().is_dir() {
                walk(&path, root, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// Dispatch without unwrapping the protocol layer: a refused pin must come back as an `Ok`
/// result (a recoverable answer that leaves sibling calls alive), never as a protocol error.
async fn dispatch_pinned(
    server: &CodeScoutServer,
    name: &str,
    mut args: serde_json::Value,
    pin: &str,
) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
    args["workspace"] = serde_json::json!(pin);
    let req = CallToolRequestParams::new(name.to_string())
        .with_arguments(serde_json::from_value(args).unwrap());
    server
        .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
        .await
}

/// The refusal's JSON body: `RecoverableError` is routed to `isError: false` with `ok: false`.
fn assert_pin_refused(label: &str, result: &CallToolResult, pin_text: &str) {
    let text = result_text(result);
    assert_ne!(
        result.is_error,
        Some(true),
        "{label}: a refused pin is recoverable (sibling calls must survive); got: {text}"
    );
    let body: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|_| panic!("{label}: the refusal must be a JSON body; got: {text}"));
    assert_eq!(body["ok"], false, "{label}: must be refused; got: {text}");
    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        error.contains(pin_text),
        "{label}: the refusal must name the pin {pin_text:?}; got: {text}"
    );
    assert!(
        error.contains("not an existing directory"),
        "{label}: the refusal must say what is wrong with it; got: {text}"
    );
    // Which ARGUMENT is refused: a pin here, not a project root.
    assert!(
        error.starts_with("workspace pin "),
        "{label}: the refusal must name the argument as the workspace pin; got: {text}"
    );
    assert!(
        body["hint"]
            .as_str()
            .is_some_and(|h| h.contains("absolute")),
        "{label}: the refusal must say what to do (pass an absolute path); got: {text}"
    );
}

/// The call that was actually made on 2026-10-18 (a write, `edit_file`), a read (`memory
/// list`, the issue's reproduction), and the other write shapes, each pinned to an ABSOLUTE
/// path that does not exist. A read and a write take different routes before the tool body (the
/// write takes the project's write lock first), so each is its own row.
#[tokio::test]
async fn a_pin_naming_a_missing_absolute_path_is_refused_and_creates_nothing() {
    let (_home, server) = make_server().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let missing = host_root.join("does-not-exist");
    let pin = missing.to_string_lossy().to_string();

    let calls = [
        (
            "memory list (read)",
            "memory",
            serde_json::json!({"action": "list"}),
        ),
        (
            "memory write",
            "memory",
            serde_json::json!({"action": "write", "topic": "t", "content": "c"}),
        ),
        (
            "read_file (read)",
            "read_file",
            serde_json::json!({"path": "a.txt"}),
        ),
        (
            "edit_file (the 2026-10-18 call)",
            "edit_file",
            serde_json::json!({"path": "a.txt", "old_string": "a", "new_string": "b"}),
        ),
        (
            "create_file",
            "create_file",
            serde_json::json!({"path": "n.txt", "content": "c"}),
        ),
    ];
    for (label, tool, args) in calls {
        let result = dispatch_pinned(&server, tool, args, &pin)
            .await
            .unwrap_or_else(|e| {
                panic!("{label}: a refused pin must be an Ok result, not a protocol error: {e:?}")
            });
        assert_pin_refused(label, &result, &pin);
    }
    assert!(
        !missing.exists(),
        "the refused pin must not have been created"
    );
    assert_eq!(
        tree_listing(&host_root),
        Vec::<String>::new(),
        "nothing may be created anywhere under the directory the pin pointed into"
    );
}

/// A pin that names an existing FILE is not a project root either. Guards `is_dir` against
/// `exists`: a file exists, so only the directory test refuses it. Before the fix this was not
/// refused as a pin at all; it failed deep inside residency with an OS error.
#[tokio::test]
async fn a_pin_naming_a_file_is_refused_and_leaves_it_untouched() {
    let (_home, server) = make_server().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let file = host_root.join("not-a-dir.txt");
    std::fs::write(&file, "keep").unwrap();
    let pin = file.to_string_lossy().to_string();

    let result = dispatch_pinned(
        &server,
        "memory",
        serde_json::json!({"action": "list"}),
        &pin,
    )
    .await
    .expect("a refused pin must be an Ok result");
    assert_pin_refused("memory list", &result, &pin);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "keep");
    assert_eq!(tree_listing(&host_root), vec!["not-a-dir.txt".to_string()]);
}

/// The reproduction in the issue, as the filing agent ran it: a RELATIVE placeholder pin,
/// `workspace="x"`. Runs in a child process whose cwd is a tempdir, so the pre-fix behaviour
/// (a project built at `<cwd>/x`) lands in that tempdir and never in the repo.
#[cfg(unix)]
#[test]
fn a_relative_pin_naming_a_missing_path_is_refused_and_creates_nothing() {
    crate::platform::test_child::run(
        module_path!(),
        "a_relative_pin_naming_a_missing_path_is_refused_and_creates_nothing",
        &[],
        || {
            let cwd = tempdir().unwrap();
            let cwd_root = std::fs::canonicalize(cwd.path()).unwrap();
            // Child process only: nothing else runs here, so nothing else reads the cwd.
            std::env::set_current_dir(&cwd_root).unwrap();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (_home, server) = make_server().await;
                    for (label, tool, args) in [
                        (
                            "memory list",
                            "memory",
                            serde_json::json!({"action": "list"}),
                        ),
                        (
                            "edit_file",
                            "edit_file",
                            serde_json::json!({"path": "x", "old_string": "a", "new_string": "b"}),
                        ),
                    ] {
                        let result = dispatch_pinned(&server, tool, args, "x")
                            .await
                            .unwrap_or_else(|e| panic!("{label}: protocol error: {e:?}"));
                        assert_pin_refused(label, &result, "x");
                        assert!(
                            result_text(&result).contains("working directory"),
                            "{label}: a relative pin resolves against the server's cwd, and the \
                             refusal must say so; got: {}",
                            result_text(&result)
                        );
                    }
                    assert_eq!(
                        tree_listing(&cwd_root),
                        Vec::<String>::new(),
                        "the refused relative pin must create nothing in the cwd"
                    );
                });
        },
    );
}

/// The behaviour that must NOT change: a relative pin that DOES exist still resolves against
/// the server's cwd (as `workspace(activate)` and `--project` do) and still makes that
/// directory a project. Same child-process isolation as above.
#[cfg(unix)]
#[test]
fn a_relative_pin_naming_an_existing_directory_still_resolves_against_the_cwd() {
    crate::platform::test_child::run(
        module_path!(),
        "a_relative_pin_naming_an_existing_directory_still_resolves_against_the_cwd",
        &[],
        || {
            let cwd = tempdir().unwrap();
            let cwd_root = std::fs::canonicalize(cwd.path()).unwrap();
            std::fs::create_dir(cwd_root.join("proj")).unwrap();
            std::env::set_current_dir(&cwd_root).unwrap();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (_home, server) = make_server().await;
                    let result = dispatch_pinned(
                        &server,
                        "memory",
                        serde_json::json!({"action": "list"}),
                        "proj",
                    )
                    .await
                    .expect("an existing relative pin must be served");
                    // The SUCCESS body, not the absence of one refusal's wording: a refusal of
                    // any other kind is a RecoverableError too (`is_error: false`), and
                    // residency builds `proj/.codescout` before a later step can fail.
                    // Measured: with the root left un-canonicalized the call answered
                    // `{"ok": false, "error": "workspace pin .../proj is not resident"}` and
                    // every assertion that only looked for "not an existing directory" passed.
                    assert_eq!(result_text(&result), "0 topics");
                    // And the pin really targets `proj`: a write through it lands there.
                    let written = dispatch_pinned(
                        &server,
                        "memory",
                        serde_json::json!({"action": "write", "topic": "t", "content": "c"}),
                        "proj",
                    )
                    .await
                    .expect("an existing relative pin must be writable");
                    assert!(
                        !result_text(&written).contains("\"ok\": false"),
                        "got: {}",
                        result_text(&written)
                    );
                    assert!(
                        cwd_root
                            .join("proj")
                            .join(".codescout/memories/t.md")
                            .is_file(),
                        "the pin resolved against the cwd and made THAT directory the project"
                    );
                    assert_eq!(
                        tree_listing(&cwd_root)
                            .into_iter()
                            .filter(|p| p != "proj" && !p.starts_with("proj/"))
                            .collect::<Vec<_>>(),
                        Vec::<String>::new(),
                        "nothing may be created beside the pinned directory"
                    );
                });
        },
    );
}

/// An empty or whitespace-only `workspace` is "no pin" (the existing contract), NOT a pin
/// naming a missing path: it must keep answering from the session's project and create nothing.
#[cfg(unix)]
#[test]
fn a_blank_pin_is_still_no_pin_and_creates_nothing() {
    crate::platform::test_child::run(
        module_path!(),
        "a_blank_pin_is_still_no_pin_and_creates_nothing",
        &[],
        || {
            let cwd = tempdir().unwrap();
            let cwd_root = std::fs::canonicalize(cwd.path()).unwrap();
            std::env::set_current_dir(&cwd_root).unwrap();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (_home, server) = make_server().await;
                    for blank in ["", "   "] {
                        let result = dispatch_pinned(
                            &server,
                            "memory",
                            serde_json::json!({"action": "list"}),
                            blank,
                        )
                        .await
                        .expect("a blank pin is no pin");
                        // The default project's own answer, not merely "no refusal text".
                        assert_eq!(
                            result_text(&result),
                            "0 topics",
                            "{blank:?} is no pin and must be answered by the default project"
                        );
                    }
                    assert_eq!(tree_listing(&cwd_root), Vec::<String>::new());
                });
        },
    );
}

/// A relative pin is resolved against the server's cwd, and the refusal says where that is. If
/// the cwd itself is unreadable (here: deleted under the process, which Linux allows) it must
/// still be refused, and say that the path is relative without inventing a directory. Guards
/// the `current_dir()` failure arm of the message, which no other test can reach.
#[cfg(unix)]
#[test]
fn a_relative_pin_is_refused_even_when_the_cwd_is_gone() {
    crate::platform::test_child::run(
        module_path!(),
        "a_relative_pin_is_refused_even_when_the_cwd_is_gone",
        &[],
        || {
            let cwd = tempdir().unwrap();
            let cwd_root = std::fs::canonicalize(cwd.path()).unwrap();
            std::env::set_current_dir(&cwd_root).unwrap();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (_home, server) = make_server().await;
                    std::fs::remove_dir(&cwd_root).unwrap();
                    assert!(std::env::current_dir().is_err(), "premise: the cwd is gone");
                    let result = dispatch_pinned(
                        &server,
                        "memory",
                        serde_json::json!({"action": "list"}),
                        "x",
                    )
                    .await
                    .expect("a refused pin must be an Ok result");
                    assert_pin_refused("memory list", &result, "x");
                    let body: serde_json::Value =
                        serde_json::from_str(&result_text(&result)).unwrap();
                    let error = body["error"].as_str().unwrap();
                    assert!(
                        error.ends_with("working directory)"),
                        "with no readable cwd the note must end at the words, not name a path; \
                         got: {error}"
                    );
                });
        },
    );
}

/// The write lock is taken on the PINNED project, not the session default. `call_tool_inner`
/// resolves the pin once and hands it to the lock acquisition; this holds the DEFAULT project's
/// write mutex and requires a pinned write to another project to complete anyway. Handing the
/// lock acquisition no pin (or re-deriving it wrongly) makes the pinned write queue behind the
/// default's lock, and the call never returns.
#[tokio::test]
async fn a_pinned_write_takes_the_pinned_projects_write_lock_not_the_defaults() {
    let (_home, server) = make_server().await;
    let pinned = tempdir().unwrap();
    std::fs::create_dir_all(pinned.path().join(".codescout")).unwrap();
    let pin = std::fs::canonicalize(pinned.path())
        .unwrap()
        .to_string_lossy()
        .to_string();

    let default_lock = server
        .agent
        .with_project(|p| Ok(p.write_lock.clone()))
        .await
        .unwrap();
    let _default_held = default_lock.lock_owned().await;

    let call = dispatch_pinned(
        &server,
        "memory",
        serde_json::json!({"action": "write", "topic": "lock-target", "content": "x"}),
        &pin,
    );
    let result = tokio::time::timeout(std::time::Duration::from_secs(20), call)
        .await
        .expect("a pinned write must not queue behind the DEFAULT project's write lock")
        .expect("dispatch");
    assert_ne!(result.is_error, Some(true), "got: {}", result_text(&result));
    assert!(
        pinned
            .path()
            .join(".codescout/memories/lock-target.md")
            .exists(),
        "the write landed in the pinned project"
    );
}

// ---------------------------------------------------------------------------------------------
// Review round 2026-10-18 (F2 to F9). Each of these guards a branch the first round left open.
// ---------------------------------------------------------------------------------------------

/// The refusal body of a RECOVERABLE answer: `Ok`, not `is_error`, `ok: false`.
fn recoverable_body(label: &str, result: &CallToolResult) -> serde_json::Value {
    let text = result_text(result);
    assert_ne!(
        result.is_error,
        Some(true),
        "{label}: must be recoverable; got: {text}"
    );
    let body: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|_| panic!("{label}: must be a JSON refusal body; got: {text}"));
    assert_eq!(body["ok"], false, "{label}: must be refused; got: {text}");
    body
}

/// Makes a directory unreadable for the life of the guard, restoring the mode on drop (also on
/// a panic, so the tempdir can be removed). `None` when the mode has no effect, which is the
/// case for a process that may read anything (root): the caller then has no premise to test.
#[cfg(unix)]
struct Denied(std::path::PathBuf);

#[cfg(unix)]
impl Denied {
    fn new(path: &std::path::Path) -> Option<Self> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
        let guard = Self(path.to_path_buf());
        if std::fs::read_dir(path).is_ok() {
            eprintln!("mode 000 does not bind this process; skipping");
            return None;
        }
        Some(guard)
    }
}

#[cfg(unix)]
impl Drop for Denied {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

const WRITE_TOPIC: &str = r#"{"action": "write", "topic": "t", "content": "c"}"#;

/// F9: a `workspace` value that is not a string was silently "no pin", so `workspace: 5` was
/// answered by the DEFAULT project. Absent and `null` stay "no pin".
#[tokio::test]
async fn a_workspace_value_that_is_not_a_string_is_refused() {
    let (_home, server) = make_server().await;
    for (label, kind, value) in [
        ("number", "a number", serde_json::json!(5)),
        ("bool", "a boolean", serde_json::json!(true)),
        ("array", "an array", serde_json::json!(["x"])),
        ("object", "an object", serde_json::json!({"path": "x"})),
    ] {
        let mut args = serde_json::json!({"action": "list"});
        args["workspace"] = value;
        let result = dispatch_tool(&server, "memory", args).await;
        let body = recoverable_body(label, &result);
        let error = body["error"].as_str().unwrap_or_default();
        assert!(
            error.contains("workspace") && error.contains("string") && error.contains(kind),
            "{label}: must say `workspace` has to be a string and what it was; got: {error}"
        );
        assert!(
            body["hint"]
                .as_str()
                .is_some_and(|h| h.contains("absolute")),
            "{label}: must say what to do; got: {body}"
        );
    }
    let mut null_pin = serde_json::json!({"action": "list"});
    null_pin["workspace"] = serde_json::Value::Null;
    assert_eq!(
        result_text(&dispatch_tool(&server, "memory", null_pin).await),
        "0 topics",
        "null is no pin"
    );
    assert_eq!(
        result_text(&dispatch_tool(&server, "memory", serde_json::json!({"action": "list"})).await),
        "0 topics",
        "absent is no pin"
    );
}

/// F3 (M4): the pin refusal is the answer, not the write gate's. A default project that is
/// read-only refuses every write; a write pinned at a missing path must still be told about
/// the PIN, because the write gate's text would send the caller to the wrong remedy.
#[tokio::test]
async fn a_bad_pin_is_refused_before_the_write_gate_speaks() {
    let (home, server) = make_server().await;
    let home_root = std::fs::canonicalize(home.path()).unwrap();
    let armed = dispatch_tool(
        &server,
        "workspace",
        serde_json::json!({
            "action": "activate", "path": home_root.to_string_lossy(), "read_only": true
        }),
    )
    .await;
    assert_ne!(
        armed.is_error,
        Some(true),
        "precondition: {}",
        result_text(&armed)
    );
    let control = dispatch_tool(
        &server,
        "memory",
        serde_json::from_str(WRITE_TOPIC).unwrap(),
    )
    .await;
    assert_eq!(
        control.is_error,
        Some(true),
        "control: an unpinned write is refused by the read-only gate; got: {}",
        result_text(&control)
    );

    let host = tempdir().unwrap();
    let missing = std::fs::canonicalize(host.path())
        .unwrap()
        .join("does-not-exist");
    let result = dispatch_pinned(
        &server,
        "memory",
        serde_json::from_str(WRITE_TOPIC).unwrap(),
        &missing.to_string_lossy(),
    )
    .await
    .expect("a refused pin is an Ok result");
    assert_pin_refused(
        "pinned write under read-only",
        &result,
        &missing.to_string_lossy(),
    );
    assert!(
        !result_text(&result).contains("disabled"),
        "the write gate must not have spoken; got: {}",
        result_text(&result)
    );
}

/// F5: a refused pin leaves a usage record, against the session's DEFAULT project, and builds
/// nothing at the pinned path. The incident that found this bug was diagnosed from exactly
/// such a row; before this, a refused call vanished without one.
#[tokio::test]
async fn a_refused_pin_is_recorded_against_the_default_project_and_nowhere_else() {
    let (home, server) = make_server().await;
    let home_root = std::fs::canonicalize(home.path()).unwrap();
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let missing = host_root.join("does-not-exist");

    let result = dispatch_pinned(
        &server,
        "memory",
        serde_json::json!({"action": "list"}),
        &missing.to_string_lossy(),
    )
    .await
    .expect("a refused pin is an Ok result");
    assert_pin_refused("memory list", &result, &missing.to_string_lossy());

    let conn = crate::usage::db::open_db(&home_root).unwrap();
    let rows: Vec<(String, String, String)> = conn
        .prepare("SELECT tool_name, outcome, project_root FROM tool_calls")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        rows,
        vec![(
            "memory".to_string(),
            "recoverable_error".to_string(),
            home_root.to_string_lossy().to_string()
        )],
        "the refused call is one recoverable row in the DEFAULT project's usage.db"
    );
    assert_eq!(
        tree_listing(&host_root),
        Vec::<String>::new(),
        "recording must not build anything at the pinned path"
    );
}

/// F2 (route 1): a pin accepted at dispatch can stop being usable before the write lock is
/// taken (the directory vanishes in between). That is a recoverable refusal, not a protocol
/// error, and it must not re-create the directory. Driven at the lock acquisition itself,
/// because the window is too narrow to hit through the full dispatch.
#[tokio::test]
async fn a_pin_that_vanishes_before_the_write_lock_is_a_recoverable_refusal() {
    let (_home, server) = make_server().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let gone = host_root.join("vanished");

    let outcome = server
        .acquire_write_guard_if_writing(
            "memory",
            &serde_json::from_str(WRITE_TOPIC).unwrap(),
            Some(&gone),
        )
        .await
        .unwrap_or_else(|e| panic!("must not be a protocol error: {e:?}"));
    let refusal = match outcome {
        Err(refusal) => refusal,
        Ok(_) => panic!("a missing pin must not yield a write guard"),
    };
    let body = recoverable_body("write gate", &refusal);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|e| e.contains(&gone.to_string_lossy().to_string())),
        "must name the pin; got: {body}"
    );
    assert_eq!(tree_listing(&host_root), Vec::<String>::new());
}

/// F2 (route 2) and F4: an existing directory this process may not enter is accepted at
/// dispatch and fails in residency. The write must be a recoverable refusal (it was a protocol
/// error, `write gate: Permission denied`), and neither it nor the read may blame project.toml.
#[cfg(unix)]
#[tokio::test]
async fn a_pin_to_an_unreadable_directory_is_recoverable_and_does_not_blame_project_toml() {
    let (_home, server) = make_server().await;
    let host = tempdir().unwrap();
    let noperm = std::fs::canonicalize(host.path()).unwrap().join("noperm");
    std::fs::create_dir(&noperm).unwrap();
    let Some(_denied) = Denied::new(&noperm) else {
        return;
    };
    let pin = noperm.to_string_lossy().to_string();

    for (label, tool, args) in [
        (
            "memory list",
            "memory",
            serde_json::json!({"action": "list"}),
        ),
        (
            "memory write",
            "memory",
            serde_json::from_str::<serde_json::Value>(WRITE_TOPIC).unwrap(),
        ),
    ] {
        let result = dispatch_pinned(&server, tool, args, &pin)
            .await
            .unwrap_or_else(|e| panic!("{label}: must not be a protocol error: {e:?}"));
        let body = recoverable_body(label, &result);
        let error = body["error"].as_str().unwrap_or_default();
        assert!(
            error.contains(&pin) && error.to_lowercase().contains("permission denied"),
            "{label}: must name the pin and the real cause; got: {body}"
        );
        assert!(
            !body.to_string().contains("project.toml"),
            "{label}: an unreadable directory is not a project.toml problem; got: {body}"
        );
    }
}

/// F2 (route 3), the case the existing project.toml hint is FOR: an unparseable config. The
/// write was a protocol error here (`write gate: parsing .../project.toml`); both calls must
/// be recoverable and both must point at project.toml.
#[tokio::test]
async fn a_pin_to_an_unparseable_project_toml_is_recoverable_and_names_project_toml() {
    let (_home, server) = make_server().await;
    let host = tempdir().unwrap();
    let broken = std::fs::canonicalize(host.path()).unwrap().join("broken");
    std::fs::create_dir_all(broken.join(".codescout")).unwrap();
    std::fs::write(
        broken.join(".codescout/project.toml"),
        "this is = = not toml [[",
    )
    .unwrap();
    let pin = broken.to_string_lossy().to_string();

    for (label, args) in [
        (
            "memory write",
            serde_json::from_str::<serde_json::Value>(WRITE_TOPIC).unwrap(),
        ),
        ("memory list", serde_json::json!({"action": "list"})),
    ] {
        let result = dispatch_pinned(&server, "memory", args, &pin)
            .await
            .unwrap_or_else(|e| panic!("{label}: must not be a protocol error: {e:?}"));
        let body = recoverable_body(label, &result);
        assert!(
            body["error"].as_str().is_some_and(|e| e.contains(&pin)),
            "{label}: must name the pin; got: {body}"
        );
        assert!(
            body["hint"]
                .as_str()
                .is_some_and(|h| h.contains("project.toml")),
            "{label}: an unparseable config must point at project.toml; got: {body}"
        );
    }
}
