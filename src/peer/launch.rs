//! Requester-side spawn-on-demand for `peer-serve`. Mirrors
//! `lsp::manager::get_or_start_via_mux`: flock-check -> spawn detached
//! `codescout peer-serve` -> wait for `ready` -> connect with retries.

use crate::peer::client::PeerClient;
use anyhow::{Context, Result};
use std::path::Path;

/// Idle timeout (seconds) for auto-spawned peer-serve processes. Matches the
/// LSP mux default; an auto-spawned serve reaps itself after this much idle.
pub const PEER_IDLE_TIMEOUT_SECS: u64 = 300;

/// Build the CLI argv for a spawned `codescout peer-serve` child. Factored out
/// for unit-testability (mirrors `lsp::manager::build_mux_args`). Phase 1 always
/// serves read-only, so `--read-only` is omitted (the CLI default is `true`).
pub(crate) fn build_peer_serve_args(
    socket_path: &Path,
    workspace: &Path,
    idle_timeout_secs: u64,
) -> Vec<String> {
    vec![
        "peer-serve".to_string(),
        "--socket".to_string(),
        socket_path.to_string_lossy().to_string(),
        "--workspace".to_string(),
        workspace.to_string_lossy().to_string(),
        "--idle-timeout".to_string(),
        idle_timeout_secs.to_string(),
    ]
}

/// Connect to the peer-serve owning `target`, spawning it on demand if not
/// running. Mirrors `LspManager::get_or_start_via_mux`:
///
/// 1. Derive the per-workspace socket + lock paths.
/// 2. Acquire the flock to decide whether a serve is already running.
/// 3. If we got the lock (none running), drop it and spawn a detached
///    `codescout peer-serve` child; wait for its `ready\n` line (120s).
/// 4. Connect as a client with retries.
///
/// `read_only` is reserved for Phase 1.5+ RW peers; Phase 1 always spawns
/// read-only, so it is logged but not yet passed as a CLI flag.
pub async fn ensure_peer_serve(target: &Path, read_only: bool) -> Result<PeerClient> {
    use fs4::fs_std::FileExt;

    let socket_path = crate::socket_discovery::peer_socket_path_for_workspace(target);
    let lock_path = crate::socket_discovery::peer_lock_path_for_workspace(target);

    let need_spawn = {
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let lock_file = opts
            .open(&lock_path)
            .with_context(|| format!("failed to open peer lock file: {}", lock_path.display()))?;
        match lock_file.try_lock_exclusive() {
            Ok(()) => {
                drop(lock_file);
                true
            }
            Err(_) => {
                tracing::info!("peer-serve already running for {}", target.display());
                false
            }
        }
    };

    if need_spawn {
        tracing::info!(
            "spawning peer-serve for {} (read_only={read_only})",
            target.display()
        );
        let exe = std::env::current_exe().context("failed to determine codescout binary path")?;
        let args = build_peer_serve_args(&socket_path, target, PEER_IDLE_TIMEOUT_SECS);

        let mut child = tokio::process::Command::new(&exe)
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("failed to spawn peer-serve process")?;

        let stdout = child.stdout.take().expect("stdout piped");
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut line = String::new();
        match tokio::time::timeout(
            std::time::Duration::from_secs(120),
            tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line),
        )
        .await
        {
            Ok(Ok(_)) if line.trim().starts_with("ready") => {
                tracing::info!("peer-serve ready for {}", target.display());
            }
            Ok(Ok(_)) => {
                tracing::warn!(
                    "peer-serve produced no ready line for {} ({:?}); trying to connect anyway",
                    target.display(),
                    line.trim()
                );
            }
            Ok(Err(e)) => {
                tracing::warn!("peer-serve stdout error for {}: {e}", target.display());
            }
            Err(_) => {
                return Err(crate::tools::RecoverableError::with_hint(
                    format!(
                        "peer-serve timed out waiting for ready (120s) for {}",
                        target.display()
                    ),
                    "The peer workspace may be slow to index. Retry in a moment, or check \
                     for a stale lock file in the per-user runtime dir.",
                )
                .into());
            }
        }
    }

    // Connect with retries. The spawn path connects on the first attempt (the
    // child already signalled `ready`). A NON-spawning racer — found the lock held
    // because a sibling just spawned a still-initializing child — relies on this
    // budget to span the child's build_server_for / Agent::new startup, so keep it
    // generous (≈10s) lest concurrent first-use fail spuriously. (Review finding I-2.)
    let mut last_err = None;
    for attempt in 0..50u32 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        match PeerClient::connect(&socket_path).await {
            Ok(client) => return Ok(client),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err
        .unwrap_or_else(|| anyhow::anyhow!("failed to connect to peer-serve for {}", target.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn build_peer_serve_args_derives_socket_and_workspace() {
        let args = build_peer_serve_args(
            &PathBuf::from("/run/user/1000/codescout-peer-abc.sock"),
            &PathBuf::from("/home/u/proj"),
            300,
        );
        assert_eq!(args[0], "peer-serve");
        let socket_idx = args.iter().position(|a| a == "--socket").unwrap();
        assert_eq!(
            args[socket_idx + 1],
            "/run/user/1000/codescout-peer-abc.sock"
        );
        let ws_idx = args.iter().position(|a| a == "--workspace").unwrap();
        assert_eq!(args[ws_idx + 1], "/home/u/proj");
        let idle_idx = args.iter().position(|a| a == "--idle-timeout").unwrap();
        assert_eq!(args[idle_idx + 1], "300");
        assert!(!args.iter().any(|a| a == "--read-only"));
    }
}
