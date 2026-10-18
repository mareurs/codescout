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
                    let text = result_text(&result);
                    assert_ne!(result.is_error, Some(true), "got: {text}");
                    assert!(
                        !text.contains("not an existing directory"),
                        "an existing relative pin must not be refused; got: {text}"
                    );
                    assert!(
                        cwd_root.join("proj").join(".codescout").is_dir(),
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
                        let text = result_text(&result);
                        assert_ne!(result.is_error, Some(true), "{blank:?}: {text}");
                        assert!(
                            !text.contains("not an existing directory"),
                            "{blank:?} is no pin and must not be refused; got: {text}"
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
