use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};

fn checkout() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    crate::tools::trusted_formatter::materialize_fixture(dir.path());
    assert!(std::process::Command::new("git")
        .args(["init", "-q"])
        .arg(dir.path())
        .status()
        .unwrap()
        .success());
    dir
}

fn transient_contention(error: &anyhow::Error) -> bool {
    error
        .to_string()
        .to_ascii_lowercase()
        .replace(' ', "")
        .contains("wouldblock")
}
async fn quiescent_generation(root: &Path) -> u64 {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match crate::tools::native_provenance_fence::read_generation(root) {
            Ok(generation) => return generation,
            Err(error)
                if error
                    .to_string()
                    .to_ascii_lowercase()
                    .replace(' ', "")
                    .contains("wouldblock")
                    && std::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            }
            Err(error) => panic!("generation snapshot did not become quiescent: {error}"),
        }
    }
}

#[tokio::test]
async fn a_competing_dispatch_is_refused_before_its_body_runs() {
    let dir = checkout();
    let fence = crate::tools::native_provenance_fence::Fence::try_acquire(dir.path()).unwrap();
    let ran = AtomicBool::new(false);
    let result = scope_root(
        dir.path(),
        true,
        "create_file",
        &json!({}),
        None,
        None,
        async {
            ran.store(true, Ordering::SeqCst);
            Ok(Vec::new())
        },
    )
    .await;
    assert!(result.is_err());
    assert!(!ran.load(Ordering::SeqCst));
    drop(fence);
}

#[tokio::test]
async fn readers_do_not_claim_the_provenance_write_fence() {
    let dir = checkout();
    let _fence = crate::tools::native_provenance_fence::Fence::try_acquire(dir.path()).unwrap();
    scope_root(
        dir.path(),
        false,
        "symbols",
        &json!({}),
        None,
        None,
        async { Ok(Vec::new()) },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn run_command_participates_even_when_effect_is_declared_read() {
    let dir = checkout();
    let _fence = crate::tools::native_provenance_fence::Fence::try_acquire(dir.path()).unwrap();
    let ran = AtomicBool::new(false);
    let result = scope_root(
        dir.path(),
        false,
        "run_command",
        &json!({"command":"anything","effect":"read"}),
        None,
        None,
        async {
            ran.store(true, Ordering::SeqCst);
            Ok(Vec::new())
        },
    )
    .await;
    assert!(result.is_err());
    assert!(!ran.load(Ordering::SeqCst));
}

#[tokio::test]
async fn the_actual_shell_inherits_the_request_scoped_fence() {
    let dir = checkout();
    let mut child = None;
    scope_root(
        dir.path(),
        false,
        "run_command",
        &json!({"command":"anything"}),
        None,
        None,
        async {
            let mut command = tokio::process::Command::new("python3");
            command
                .args([
                    "-u",
                    "-c",
                    "import sys;print('ready',flush=True);sys.stdin.readline()",
                ])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped());
            crate::tools::invocation_context::apply_to_shell(&mut command)?;
            let mut process = command.spawn()?;
            use tokio::io::AsyncBufReadExt;
            let mut line = String::new();
            tokio::io::BufReader::new(process.stdout.take().unwrap())
                .read_line(&mut line)
                .await?;
            assert_eq!(line.trim(), "ready");
            child = Some(process);
            Ok(Vec::new())
        },
    )
    .await
    .unwrap();
    assert!(crate::tools::native_provenance_fence::Fence::try_acquire(dir.path()).is_err());
    let domain = crate::tools::native_provenance_fence::coordination_root().unwrap();
    assert!(crate::tools::native_provenance_fence::Fence::try_acquire(&domain).is_err());
    let mut child = child.unwrap();
    drop(child.stdin.take());
    assert!(child.wait().await.unwrap().success());
}

#[tokio::test]
async fn native_formatter_sanitizes_the_actual_shell_environment() {
    let dir = checkout();
    let client = ClientIdentity {
        name: "codex-mcp-client".to_owned(),
        version: None,
    };
    let domain = crate::tools::native_provenance_fence::coordination_root().unwrap();
    let generation = quiescent_generation(&domain).await;
    let metadata = json!({
        "sessionId":"root","threadId":"thread","codex/toolCallId":"formatter",
        "codex/nativeExecutionProvenance":{
            "checkpoint":{"identity":{"root":"root","thread":"thread"},"call_id":"formatter"},
            "fence_capability":true,"coordination_root":domain,
            "coordination_generation":generation,"fence":{"root":dir.path(),"generation":0}
        }
    });
    quiescent_formatter_call(dir.path(),&metadata,&client,||async {
        let mut command=tokio::process::Command::new("python3");
        command.args(["-I","-c","import os; print(os.environ.get('BASH_ENV','absent')); print(os.environ.get('FMT_MINE_PROVENANCE','absent')); print(os.environ.get('PYTHONNOUSERSITE','absent')); print(bool(os.environ.get('CODESCOUT_NATIVE_PROVENANCE')))"]);
        command.env("BASH_ENV","untrusted-startup").env("FMT_MINE_PROVENANCE","untrusted-reader");
        crate::tools::invocation_context::apply_to_shell(&mut command)?;
        let output=command.output().await?;
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(),"absent\nabsent\n1\nTrue\n");
        Ok(vec![])
    }).await.unwrap();
}

#[test]
fn formatter_capability_rejects_background_interactive_and_string_booleans() {
    for flag in ["run_in_background", "interactive"] {
        for value in [json!(true), json!("true"), json!(1), json!(null)] {
            let mut input = json!({"command":"bash scripts/fmt-mine.sh"});
            input[flag] = value;
            assert!(!is_foreground_formatter("run_command", &input));
        }
    }
    assert!(is_foreground_formatter(
        "run_command",
        &json!({"command":"bash scripts/fmt-mine.sh"})
    ));
    assert!(!is_foreground_formatter(
        "run_command",
        &json!({"command":"bash scripts/fmt-mine.sh; echo altered"})
    ));
}
async fn quiescent_formatter_call<F, Fut>(
    root: &Path,
    meta: &Value,
    client: &ClientIdentity,
    mut future: F,
) -> anyhow::Result<Vec<Content>>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<Vec<Content>>>,
{
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let result = scope_root(
            root,
            true,
            "run_command",
            &json!({"command":"bash scripts/fmt-mine.sh"}),
            meta.as_object(),
            Some(client),
            future(),
        )
        .await;
        if result.as_ref().err().is_some_and(transient_contention)
            && std::time::Instant::now() < deadline
        {
            // Parallel test forks may briefly inherit unrelated CLOEXEC fds.
            // Only quiescent admission retries; held-lease refusal assertions do not.
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        } else {
            return result;
        }
    }
}

#[tokio::test]
async fn certificate_binds_native_root_thread_call_and_fence_before_effect() {
    let dir = checkout();
    let client = ClientIdentity {
        name: "codex-mcp-client".to_owned(),
        version: None,
    };
    let domain = crate::tools::native_provenance_fence::coordination_root().unwrap();
    let initial_coordination = quiescent_generation(&domain).await;
    let metadata = json!({
        "sessionId":"root","threadId":"thread","codex/toolCallId":"formatter",
        "codex/nativeExecutionProvenance":{
            "checkpoint":{"identity":{"root":"root","thread":"thread"},"call_id":"formatter"},
            "fence_capability":true,"coordination_root":domain,
            "coordination_generation":initial_coordination,
            "fence":{"root":dir.path(),"generation":0}
        }
    });
    quiescent_formatter_call(dir.path(), &metadata, &client, || async {
        FENCE.with(|scope| {
            assert_eq!(
                scope.certificate.as_ref().unwrap()["invocation_id"],
                "formatter"
            );
            assert_eq!(scope.certificate.as_ref().unwrap()["fence_generation"], 0);
            assert_eq!(scope.fence.generation(), 1);
        });
        Ok(vec![])
    })
    .await
    .unwrap();
    for key in ["sessionId", "threadId", "codex/toolCallId"] {
        let mut invalid = metadata.clone();
        invalid[key] = json!("other");
        invalid["codex/nativeExecutionProvenance"]["fence"]["generation"] = json!(1);
        invalid["codex/nativeExecutionProvenance"]["coordination_generation"] =
            json!(initial_coordination + 1);
        let ran = AtomicBool::new(false);
        let result = quiescent_formatter_call(dir.path(), &invalid, &client, || async {
            ran.store(true, Ordering::SeqCst);
            Ok(vec![])
        })
        .await;
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("checkpoint is not bound"),
            "{key}"
        );
        assert!(!ran.load(Ordering::SeqCst), "{key}");
    }
    let ran = AtomicBool::new(false);
    let result = quiescent_formatter_call(dir.path(), &metadata, &client, || async {
        ran.store(true, Ordering::SeqCst);
        Ok(vec![])
    })
    .await;
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("checkpoint is not bound"));
    assert!(!ran.load(Ordering::SeqCst));
}
#[tokio::test]
async fn strong_formatter_checks_the_actual_helper_closure_before_its_body() {
    let dir = checkout();
    std::fs::write(dir.path().join("scripts/fmt_paths.py"), "alternate helper").unwrap();
    let domain = crate::tools::native_provenance_fence::coordination_root().unwrap();
    let generation = quiescent_generation(&domain).await;
    let client = ClientIdentity {
        name: "codex-mcp-client".to_owned(),
        version: None,
    };
    let metadata = json!({
        "sessionId":"root","threadId":"root","codex/toolCallId":"formatter",
        "codex/nativeExecutionProvenance":{
            "checkpoint":{"identity":{"root":"root","thread":"root"},"call_id":"formatter"},
            "fence_capability":true,"coordination_root":domain,"coordination_generation":generation,
            "fence":{"root":dir.path(),"generation":0}
        }
    });
    let ran = AtomicBool::new(false);
    let result = quiescent_formatter_call(dir.path(), &metadata, &client, || async {
        ran.store(true, Ordering::SeqCst);
        Ok(vec![])
    })
    .await;
    assert!(!ran.load(Ordering::SeqCst));
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("trusted formatter closure"));
}

/// Metadata that makes `bash scripts/fmt-mine.sh` a trusted native formatter for `root`: bound to
/// the current generations of `root` and of this test's coordination domain.
async fn native_formatter_meta(root: &Path) -> (Value, ClientIdentity) {
    let domain = crate::tools::native_provenance_fence::coordination_root().unwrap();
    let coordination_generation = quiescent_generation(&domain).await;
    let fence_generation = quiescent_generation(root).await;
    let meta = json!({
        "sessionId":"root","threadId":"thread","codex/toolCallId":"formatter",
        "codex/nativeExecutionProvenance":{
            "checkpoint":{"identity":{"root":"root","thread":"thread"},"call_id":"formatter"},
            "fence_capability":true,"coordination_root":domain,
            "coordination_generation":coordination_generation,
            "fence":{"root":root.canonicalize().unwrap(),"generation":fence_generation}
        }
    });
    let client = ClientIdentity {
        name: "codex-mcp-client".to_owned(),
        version: None,
    };
    (meta, client)
}

/// Metadata that makes a formatter call ELIGIBLE for the exclusive leases, with placeholder
/// generations and no lock reads. For a test that only needs the lease decision: the eligible
/// call is refused at the leases or never reaches `certificate` with these values. Reading real
/// generations takes the lock exclusively, and a fork in a parallel test can carry that
/// exclusive copy past the read, so a test that needs no certificate must not read them.
fn eligible_formatter_meta(root: &Path) -> (Value, ClientIdentity) {
    let meta = json!({
        "sessionId":"root","threadId":"thread","codex/toolCallId":"formatter",
        "codex/nativeExecutionProvenance":{
            "checkpoint":{"identity":{"root":"root","thread":"thread"},"call_id":"formatter"},
            "fence_capability":true,"coordination_root":"/unread","coordination_generation":0,
            "fence":{"root":root,"generation":0}
        }
    });
    let client = ClientIdentity {
        name: "codex-mcp-client".to_owned(),
        version: None,
    };
    (meta, client)
}

#[tokio::test]
async fn ordinary_edits_coexist_with_commands_but_native_formatting_requires_quiescence() {
    let dir = checkout();
    let (meta, client) = eligible_formatter_meta(dir.path());
    let _command =
        crate::tools::native_provenance_fence::Fence::try_participate(dir.path()).unwrap();
    let ran = AtomicBool::new(false);
    scope_root(
        dir.path(),
        true,
        "create_file",
        &json!({}),
        None,
        None,
        async {
            ran.store(true, Ordering::SeqCst);
            Ok(Vec::new())
        },
    )
    .await
    .unwrap();
    assert!(ran.load(Ordering::SeqCst));
    let formatter = scope_root(
        dir.path(),
        false,
        "run_command",
        &json!({"command":"bash scripts/fmt-mine.sh"}),
        meta.as_object(),
        Some(&client),
        async {
            panic!("native formatter dispatched during participating command");
            #[allow(unreachable_code)]
            Ok(Vec::new())
        },
    )
    .await;
    assert!(
        formatter
            .unwrap_err()
            .to_string()
            .contains("refused dispatch"),
        "the refusal must come from the leases, not from a later check"
    );
}

/// A formatter call that cannot use a native checkpoint shares the leases: a live participant
/// does not refuse it, and it does not refuse a participant. Before bug 91b5ffce06ede31a such a
/// call took the leases exclusively too; that is removed on purpose, so this pins the new scope.
#[tokio::test]
async fn a_formatter_without_a_native_proof_shares_the_leases() {
    let dir = checkout();
    let _command =
        crate::tools::native_provenance_fence::Fence::try_participate(dir.path()).unwrap();
    let ran = AtomicBool::new(false);
    let peer_ran = AtomicBool::new(false);
    scope_root(
        dir.path(),
        false,
        "run_command",
        &json!({"command":"bash scripts/fmt-mine.sh"}),
        None,
        None,
        async {
            ran.store(true, Ordering::SeqCst);
            scope_root(
                dir.path(),
                true,
                "create_file",
                &json!({}),
                None,
                None,
                async {
                    peer_ran.store(true, Ordering::SeqCst);
                    Ok(Vec::new())
                },
            )
            .await?;
            Ok(Vec::new())
        },
    )
    .await
    .unwrap();
    assert!(
        ran.load(Ordering::SeqCst),
        "a live participant refused the formatter"
    );
    assert!(
        peer_ran.load(Ordering::SeqCst),
        "the formatter refused a participant"
    );
}

/// The eligibility boundary of the exclusive leases: only `codex-mcp-client` with
/// `fence_capability` exactly `true` is a native formatter. Every near miss shares the leases,
/// so a live participant does not refuse it.
#[tokio::test]
async fn only_the_codex_client_with_the_fence_capability_takes_the_leases_exclusively() {
    let dir = checkout();
    let (native, codex) = eligible_formatter_meta(dir.path());
    let other = ClientIdentity {
        name: "claude-code".to_owned(),
        version: None,
    };
    let mut absent = native.clone();
    absent["codex/nativeExecutionProvenance"]
        .as_object_mut()
        .unwrap()
        .remove("fence_capability");
    let mut off = native.clone();
    off["codex/nativeExecutionProvenance"]["fence_capability"] = json!(false);
    let mut text = native.clone();
    text["codex/nativeExecutionProvenance"]["fence_capability"] = json!("true");
    let cases: [(&str, &Value, Option<&ClientIdentity>); 5] = [
        ("codex client, capability absent", &absent, Some(&codex)),
        ("codex client, capability false", &off, Some(&codex)),
        ("codex client, capability as a string", &text, Some(&codex)),
        (
            "another client claiming the capability",
            &native,
            Some(&other),
        ),
        ("no client, meta claiming the capability", &native, None),
    ];
    let _command =
        crate::tools::native_provenance_fence::Fence::try_participate(dir.path()).unwrap();
    for (case, meta, client) in cases {
        let ran = AtomicBool::new(false);
        scope_root(
            dir.path(),
            false,
            "run_command",
            &json!({"command":"bash scripts/fmt-mine.sh"}),
            meta.as_object(),
            client,
            async {
                ran.store(true, Ordering::SeqCst);
                Ok(Vec::new())
            },
        )
        .await
        .unwrap_or_else(|error| panic!("{case}: refused: {error}"));
        assert!(ran.load(Ordering::SeqCst), "{case}: body did not run");
    }
}

#[tokio::test]
async fn formatter_excludes_a_foreign_workspace_shell_targeting_its_checkout() {
    let a = checkout();
    let b = checkout();
    let target = b.path().join("probe.txt");
    std::fs::write(&target, "before").unwrap();
    // The exclusion is the native formatter's: it is the one that holds the leases exclusively.
    let (meta, client) = native_formatter_meta(b.path()).await;
    quiescent_formatter_call(b.path(), &meta, &client, || async {
        let result = scope_root(
            a.path(),
            false,
            "run_command",
            &json!({"command":"foreign target"}),
            None,
            None,
            async {
                let mut command = tokio::process::Command::new("python3");
                command
                    .arg("-c")
                    .arg("import pathlib,sys;pathlib.Path(sys.argv[1]).write_text('changed')")
                    .arg(&target);
                crate::tools::invocation_context::apply_to_shell(&mut command)?;
                assert!(command.status().await?.success());
                Ok(vec![])
            },
        )
        .await;
        assert!(
            result.is_err(),
            "foreign workspace wrote while another formatter held its lease"
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "before");
        Ok(vec![])
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn a_terminal_shell_result_does_not_certify_detached_descendants() {
    let dir = checkout();
    scope_root(
        dir.path(),
        true,
        "run_command",
        &json!({"command":"opaque script"}),
        None,
        None,
        async { Ok(vec![]) },
    )
    .await
    .unwrap();
    let domain = crate::tools::native_provenance_fence::coordination_root().unwrap();
    let global_generation = quiescent_generation(&domain).await;
    let checkout_generation = quiescent_generation(dir.path()).await;
    let client = ClientIdentity {
        name: "codex-mcp-client".to_owned(),
        version: None,
    };
    let meta = json!({
        "sessionId":"root","threadId":"root","codex/toolCallId":"formatter",
        "codex/nativeExecutionProvenance":{
            "checkpoint":{"identity":{"root":"root","thread":"root"},"call_id":"formatter"},
            "fence_capability":true,"coordination_root":domain,
            "coordination_generation":global_generation,
            "fence":{"root":dir.path(),"generation":checkout_generation}
        }
    });
    let ran = AtomicBool::new(false);
    // Through the retrying helper: the generation reads above take the leases exclusively, and a
    // fork in a parallel test can carry that exclusive copy past the read, which refused this call
    // at the lease ("would block") instead of at the marker (15 of 200 stress runs, 2026-11-10).
    let result = quiescent_formatter_call(dir.path(), &meta, &client, || async {
        ran.store(true, Ordering::SeqCst);
        Ok(vec![])
    })
    .await;
    assert!(!ran.load(Ordering::SeqCst));
    let error = result.unwrap_err().to_string();
    assert!(error.contains("uncertified shell"), "{error}");
}

#[tokio::test]
async fn arbitrary_shell_uncertainty_is_durable_before_the_body_and_after_cancellation() {
    let dir = checkout();
    let domain = crate::tools::native_provenance_fence::coordination_root().unwrap();
    let entered = AtomicBool::new(false);
    {
        let input = json!({"command":"opaque script"});
        let future = scope_root(
            dir.path(),
            false,
            "run_command",
            &input,
            None,
            None,
            async {
                assert!(crate::tools::native_provenance_fence::has_unverified_shell(
                    &domain
                )?);
                entered.store(true, Ordering::SeqCst);
                std::future::pending::<()>().await;
                Ok(vec![])
            },
        );
        tokio::pin!(future);
        tokio::select! {
            result=&mut future=>panic!("shell unexpectedly completed: {result:?}"),
            _=async {while !entered.load(Ordering::SeqCst) {tokio::task::yield_now().await;}}=>{}
        }
        // Dropping the in-flight request models caller cancellation.
    }
    assert!(entered.load(Ordering::SeqCst));
    assert!(crate::tools::native_provenance_fence::has_unverified_shell(&domain).unwrap());
    assert!(FENCE.try_with(|_| ()).is_err());
}
