# Design: Index Scope Guard

**Status:** Approved
**Date:** 2026-04-18
**Author:** Claude (brainstormed with @mareurs)

## Context

An agent that calls `activate_project("~")` (or any other overly broad root) followed by `index_project` will walk the entire home directory, embedding thousands of files it has no business embedding. Depending on scope, this ranges from annoying (large disk + memory spike) to catastrophic (OOM, system freeze — see `docs/issues/memory-leak-x-session-freeze.md`).

Today the only guard is `security.indexing_enabled` in `.codescout/project.toml`. That doesn't help when the activated directory has no `.codescout/` folder — which is exactly the dangerous case.

## Goal

Before `index_project` commits to a large walk + embed, require explicit human confirmation when either:

1. The project root is a known-broad directory (home, system paths), **or**
2. The approximate raw source size exceeds a configurable threshold (default 500 MB).

Confirmation is per-call (never persistent), delivered via MCP elicitation.

## Non-Goals

- No hard blocks. The user can always proceed after confirming.
- No persistent acknowledgment. Every `index_project` call re-checks.
- No change to `semantic_search` — only `index_project` triggers the walk.
- No protection against pathologically small-but-weird directories. Good enough, not perfect.

## Design

### Module layout

```
src/embed/preflight.rs   ← new — scope check (pure, sync, no ctx)
src/tools/semantic.rs    ← IndexProject::call wires in preflight + elicitation
src/config/project.rs    ← adds max_index_bytes field to [security]
```

### Preflight function

```rust
// src/embed/preflight.rs

pub struct PreflightInfo {
    pub root: PathBuf,
    pub file_count: usize,
    pub approx_bytes: u64,
    pub suspicious_reason: Option<SuspiciousReason>,
    pub size_exceeds_threshold: bool,
}

pub enum SuspiciousReason {
    HomeDirectory,        // == dirs::home_dir()
    HomeParent,           // e.g. /home — parent of home
    SystemPath(PathBuf),  // /, /usr, /etc, /var, /tmp, /root, /opt, /proc, /sys
}

pub enum PreflightVerdict {
    Clear,
    RequiresConfirmation(PreflightInfo),
}

pub fn check_index_scope(
    root: &Path,
    max_bytes: u64,
) -> Result<PreflightVerdict> { /* ... */ }

impl PreflightInfo {
    pub fn elicitation_message(&self) -> String { /* ... */ }
}
```

The walk uses the **same `ignore::WalkBuilder` configuration** as `build_index` (gitignore + hidden file filtering). For each eligible file: `fs::metadata().len()` — no content read. Accumulate `approx_bytes` and `file_count`.

**Early exit:** if `approx_bytes > max_bytes` during the walk, we can set `size_exceeds_threshold = true` immediately and continue tallying (for the message), but could also short-circuit the walk at 10× threshold to bound worst-case scan time for truly pathological cases (e.g., a `/` walk). Start without short-circuit; add if scan latency becomes a problem.

**Per-file errors:** `ignore::WalkBuilder::flatten()` swallows per-file IO errors silently (same as `build_index`). We match that behavior — a file that can't be stat'd is skipped and not counted. Only a failure to construct the walker itself (e.g., root doesn't exist) propagates as an error.

### Suspicious path detection

```rust
fn classify_path(root: &Path) -> Option<SuspiciousReason> {
    let canon = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

    // Home directory
    if let Some(home) = crate::platform::home_dir() {
        if canon == home {
            return Some(SuspiciousReason::HomeDirectory);
        }
        if Some(canon.as_path()) == home.parent() {
            return Some(SuspiciousReason::HomeParent);
        }
    }

    // Fixed system paths
    const SYS: &[&str] = &[
        "/", "/usr", "/etc", "/var", "/tmp", "/root", "/opt", "/proc", "/sys", "/home",
    ];
    for sys in SYS {
        if canon == Path::new(sys) {
            return Some(SuspiciousReason::SystemPath(canon.clone()));
        }
    }
    None
}
```

Canonicalization handles symlinks (e.g., `/home/user` → `/home/user`, `/tmp` → `/private/tmp` on macOS).

### Config

Add to `[security]` in `src/config/project.rs::SecuritySection`:

```rust
/// Approximate source-byte threshold above which `index_project` requires
/// user confirmation via MCP elicitation. Default: 500 MB.
#[serde(default = "default_max_index_bytes")]
pub max_index_bytes: u64,

fn default_max_index_bytes() -> u64 { 500 * 1024 * 1024 }
```

Propagated into `PathSecurityConfig` alongside existing fields.

### Elicitation

```rust
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
struct IndexConfirm {
    /// Confirm indexing this directory
    confirm: bool,
}
rmcp::elicit_safe!(IndexConfirm);
```

Message format (fields omitted when not triggered):

```
⚠ Large index scope detected

Root: <canonical path>  [ (home directory) | (system directory) | ... ]
Eligible files: ~<human-formatted count>
Approx source content: ~<human-formatted bytes>
Estimated chunks: ~<approx_bytes / 4000>

This will use significant RAM and CPU time.
Confirm indexing this directory?
```

Inline formatter for human-readable sizes (short helper, no new dep): 1024² → GB, 1024 → MB, etc.

### Integration in `IndexProject::call`

Before the existing `spawn_blocking(move || build_index(...))`:

```rust
let security = ctx.agent.security_config().await;
let verdict = tokio::task::spawn_blocking({
    let root = root.clone();
    let max_bytes = security.max_index_bytes;
    move || crate::embed::preflight::check_index_scope(&root, max_bytes)
}).await??;

if let PreflightVerdict::RequiresConfirmation(info) = verdict {
    let msg = info.elicitation_message();
    match ctx.elicit::<IndexConfirm>(&msg).await? {
        Some(IndexConfirm { confirm: true }) => {
            tracing::info!(root = ?info.root, "index scope confirmed by user");
        }
        Some(IndexConfirm { confirm: false }) | None => {
            return Err(RecoverableError::with_hint(
                "Indexing aborted — user did not confirm the scope",
                "Activate a more specific project root, or lower the scope, then retry.",
            ).into());
        }
    }
}
```

**Elicitation unsupported:** `ctx.elicit` returns `Ok(None)` if the client doesn't support elicitation. For this guard, `None` means we cannot get confirmation → abort with `RecoverableError` ("client does not support elicitation; set `security.max_index_bytes` higher or activate a smaller root"). **Never silently proceed** — defeats the guard.

Only `semantic_search`'s scope for index-on-demand is unaffected; this check is strictly on `index_project`.

## Error handling

| Condition | Outcome |
|---|---|
| Preflight OK | Proceed to `build_index` unchanged |
| Path suspicious OR size > threshold | Elicit |
| User confirms (`confirm: true`) | Proceed |
| User declines (`confirm: false`) | `RecoverableError` |
| User cancels elicitation | `RecoverableError` (already wrapped by `ctx.elicit`) |
| Client lacks elicitation capability | `RecoverableError` — **do not proceed** |
| Preflight itself errors (e.g., IO fail) | `RecoverableError` with underlying cause |

All outcomes route through `RecoverableError` → `isError: false` at MCP level, so sibling tool calls survive.

## Testing

**Unit tests (`src/embed/preflight.rs`):**

- `check_index_scope_clear_for_normal_project` — small tempdir, under threshold → `Clear`
- `check_index_scope_flags_home_directory` — point at `home_dir()` → `RequiresConfirmation(HomeDirectory)`
- `check_index_scope_flags_system_paths` — each of `/`, `/usr`, ... → `SystemPath(_)`
- `check_index_scope_flags_oversized_normal_dir` — tempdir stuffed with > threshold bytes → triggers on size alone, suspicious_reason = `None`
- `check_index_scope_respects_gitignore` — tempdir with ignored large files → under threshold (proves walker config matches)
- `classify_path_canonicalizes` — symlink to home → still detected as `HomeDirectory`
- `elicitation_message_omits_empty_reasons` — verify size-only and path-only messages both render correctly

**Integration tests (`src/tools/semantic.rs` tests module):**

- `index_project_aborts_when_elicit_returns_none` — no peer → `RecoverableError`, `build_index` never called
- `index_project_aborts_when_user_declines` — `MockPeer` returning `confirm: false` → `RecoverableError`
- `index_project_proceeds_when_user_confirms` — `MockPeer` returning `confirm: true` → reaches `build_index`
- `index_project_skips_elicitation_for_normal_project` — small project → no elicit call, proceeds directly

Reuse the existing `ToolContext` builder pattern from tests. Add a mock peer that records whether `elicit` was called and returns a fixed response.

## Rollout

Single commit on `experiments`. Feature is additive — default threshold (500 MB) won't trigger on any normal project. Suspicious-path check only triggers for pathological activations. No migration.

Add doc page `docs/manual/src/experimental/index-scope-guard.md` per project convention for feature commits on `experiments`.

## Open questions

None blocking. Future considerations:

- Should `force=true` on `index_project` skip the preflight? (Currently no — `force` means "reindex from scratch", not "skip safety.") Leave as-is; user who wants automation can raise `max_index_bytes`.
- Should the preflight cap scan time (e.g., 30 s budget)? Only if we see it hurt in practice on huge filesystems.
- Could eventually reuse the preflight walk's results to feed into `build_index` itself (avoid walking twice). Defer — separate optimization.
