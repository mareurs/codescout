use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[cfg(unix)]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(Parser)]
#[command(
    name = "codescout",
    version,
    about = "High-performance coding agent MCP server"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP server
    Start {
        /// Project root path to activate on startup
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,

        /// Transport mode
        #[arg(long, default_value = "stdio", value_parser = ["stdio", "http"])]
        transport: String,

        /// Listen address (HTTP transport only)
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Listen port (HTTP transport only)
        #[arg(long, default_value_t = 8090)]
        port: u16,

        /// Bearer token for HTTP transport authentication.
        /// If not provided when using HTTP transport, a token is auto-generated.
        #[arg(long)]
        auth_token: Option<String>,

        /// Enable debug mode: verbose logging + detailed usage recording.
        /// Subsumes the former --diagnostic flag.
        #[arg(long)]
        debug: bool,

        /// Deprecated alias for --debug.
        #[arg(long, hide = true)]
        diagnostic: bool,
    },

    /// Index the current project for semantic search
    Index {
        /// Project root path (defaults to CWD)
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,

        /// Force full reindex (skip incremental)
        #[arg(long)]
        force: bool,
    },

    /// Launch the project dashboard web UI
    #[cfg(feature = "dashboard")]
    Dashboard {
        /// Project root path (defaults to CWD)
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,

        /// Listen address
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Listen port
        #[arg(long, default_value_t = 8099)]
        port: u16,

        /// Don't auto-open the browser
        #[arg(long)]
        no_open: bool,
    },

    /// Run the LSP multiplexer (internal — spawned automatically by codescout)
    #[cfg(unix)]
    #[command(hide = true)]
    Mux {
        /// Path to the Unix socket to listen on
        #[arg(long)]
        socket: std::path::PathBuf,

        /// Path to the lock file for ownership
        #[arg(long)]
        lock: std::path::PathBuf,

        /// Working directory for the LSP server (workspace root)
        #[arg(long)]
        cwd: std::path::PathBuf,

        /// Seconds to wait with 0 clients before shutting down
        #[arg(long, default_value_t = 300)]
        idle_timeout: u64,

        /// Environment variables to set on the LSP server process. Repeat
        /// flag per variable. Format: `KEY=VAL`.
        #[arg(long = "env", value_parser = parse_env_kv)]
        server_env: Vec<(String, String)>,

        /// LSP server command and arguments (after --)
        #[arg(last = true, required = true)]
        server_cmd: Vec<String>,
    },

    /// Serve this workspace's read tools to peer codescout instances over a Unix socket.
    #[cfg(unix)]
    #[command(hide = true)]
    PeerServe {
        /// Path to the Unix socket to listen on. Defaults to the per-user
        /// derived socket for `--workspace` (codescout-peer-<hash>.sock) when omitted.
        #[arg(long)]
        socket: Option<std::path::PathBuf>,
        /// Workspace root to serve
        #[arg(long)]
        workspace: std::path::PathBuf,
        /// Serve read-only (Phase 1 is read-only regardless; flag reserved)
        #[arg(long, default_value_t = true)]
        read_only: bool,
        /// Idle timeout in seconds (reserved; not yet enforced)
        #[arg(long, default_value_t = 300)]
        idle_timeout: u64,
    },

    /// Migrate legacy sqlite-vec memories at .codescout/embeddings.db into the
    /// Qdrant `memories` collection. Idempotent — re-running overwrites by
    /// deterministic point id rather than duplicating.
    ///
    /// With --in-place, imports nothing and instead re-embeds the memories
    /// already in the store from the content it already holds. Use that after any
    /// change of embedding convention (query-prefix policy, model, dimension):
    /// fixing the write path only fixes future writes, and vectors stored under
    /// the old convention are stranded once queries move to the new one.
    MigrateMemories {
        /// Project root path (defaults to CWD). Used both to locate the legacy
        /// db and to derive the project_id namespace in Qdrant.
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,

        /// Explicit path to the legacy embeddings db. Defaults to
        /// `<project>/.codescout/embeddings.db`.
        #[arg(long)]
        db_path: Option<std::path::PathBuf>,

        /// Read + count without embedding or writing to Qdrant.
        #[arg(long)]
        dry_run: bool,

        /// Re-derive every memory vector from current embedding config, instead of
        /// importing from a legacy db. Runs TWO passes, and the second is easy to
        /// miss: memories already in the store are re-embedded in place (point ids,
        /// content, anchors and timestamps preserved — only the vector changes), and
        /// memories on disk with NO point at all are embedded from disk, which the
        /// store-driven pass cannot see because it enumerates from the store. A
        /// failed row keeps its existing state, so a misconfigured embedder cannot
        /// degrade the corpus. Mutually exclusive with --db-path.
        #[arg(long, conflicts_with = "db_path")]
        in_place: bool,
    },

    /// Print the codescout git SHA, full SHA, and dirty status baked into this
    /// binary at build time. JSON output for use by the bench harness.
    Version,

    /// Read and mutate artifacts (find, get, graph, state-at, create, …).
    #[cfg(feature = "librarian")]
    Artifact {
        #[command(subcommand)]
        verb: codescout::cli::artifact::Verb,
    },

    /// Read and write artifact events (list, create).
    #[cfg(feature = "librarian")]
    ArtifactEvent {
        #[command(subcommand)]
        verb: codescout::cli::artifact_event::Verb,
    },

    /// Read and trigger artifact augmentation refreshes.
    #[cfg(feature = "librarian")]
    ArtifactRefresh {
        #[command(subcommand)]
        verb: codescout::cli::artifact_refresh::Verb,
    },

    /// Attach or merge augmentation (prompt + params) on an artifact.
    #[cfg(feature = "librarian")]
    ArtifactAugment(codescout::cli::artifact_augment::AugmentArgs),

    /// Audit markdown files for stale code references (file paths, symbols,
    /// line refs, link targets, module paths). Surfaces broken references
    /// against the current filesystem + LSP symbol index. Used by CI gates
    /// against `master`/PR docs to catch drift.
    #[cfg(feature = "librarian")]
    AuditDocRefs(codescout::cli::audit_doc_refs::AuditArgs),

    /// Read-only scan of the librarian catalog for invariant violations:
    /// non-forward-slash separators, NTFS ADS colons, `..` segments, and
    /// missing files on disk. Output is a JSON report with per-check
    /// violation counts. Manual cadence — run after large refactors or
    /// when downstream LIKE queries return unexpected empty sets.
    #[cfg(feature = "librarian")]
    Doctor(codescout::cli::doctor::DoctorArgs),

    /// Give every artifact with no chunk rows a chunked, embedded
    /// representation, WITHOUT going through the indexer's walk.
    ///
    /// The artifacts this reaches are the ones an ordinary reindex declines to
    /// process: their content is stamped as seen while unembedded, so
    /// `content_unchanged` is true and the embed is skipped forever. Resumable;
    /// safe to interrupt. Run it from a shell, never from the MCP server — it
    /// holds the catalog lock for the whole run.
    #[cfg(feature = "librarian")]
    BackfillChunks(codescout::cli::backfill_chunks::BackfillChunksArgs),

    /// Read-only query: which active constitution rules apply to a given
    /// path. Used by codescout-companion's PreToolUse hook — not meant for
    /// interactive use. Always exits 0; prints `[]` on any internal error.
    #[cfg(feature = "librarian")]
    ConstitutionCheck(codescout::cli::constitution_check::ConstitutionCheckArgs),

    /// Compile operator rules into each Claude Code profile's CLAUDE.md, or check for drift.
    OperatorRules {
        /// `compile` writes; `check` reports drift and exits 1 if any.
        #[arg(value_parser = ["compile", "check"])]
        mode: String,
        /// Ledger path. Defaults to docs/trackers/operator-rules.md.
        #[arg(long)]
        ledger: Option<std::path::PathBuf>,
    },
}

// `--env` is only parsed by the cfg(unix) `Mux` subcommand; dead on Windows.
#[cfg_attr(not(unix), allow(dead_code))]
fn parse_env_kv(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("--env expects KEY=VAL, got {s:?}"))?;
    Ok((k.to_string(), v.to_string()))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Logging init happens before CLI parsing so startup errors are captured.
    // We peek at raw args to detect --debug / --diagnostic before clap processes them.
    // Caveat: this fires for any subcommand that receives these flags as arguments.
    // Currently only `start` has them, so this is safe — revisit if other
    // subcommands add conflicting flags.
    let debug_mode = std::env::args().any(|a| a == "--debug" || a == "--diagnostic");
    let log_state = codescout::logging::init(debug_mode);
    let _log_guards = log_state.guards;

    // Install rustls' ring crypto provider for all TLS connections (smaller
    // than aws-lc-rs). Must happen before any rustls config is built — idempotent.
    codescout::install_default_crypto_provider();

    // Load a startup dotenv (opt-in) so the MCP launcher needs no env injection.
    codescout::config::load_startup_env();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            project,
            transport,
            host,
            port,
            auth_token,
            debug,
            diagnostic,
        } => {
            let debug = debug || diagnostic;
            tracing::info!("Starting codescout MCP server (transport={})", transport);
            codescout::server::run(
                project,
                &transport,
                &host,
                port,
                auth_token,
                debug,
                log_state.instance_id,
            )
            .await?;
        }
        Commands::Index { project, force } => {
            let root = project
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            tracing::info!("Indexing project at {}", root.display());

            // Resolve project_id via Agent activation, then drive the
            // retrieval-stack sync directly. Mirrors the MCP `index(action='build')`
            // path (src/tools/semantic/index.rs), minus the background spawn.
            let agent = codescout::agent::Agent::new(Some(root.clone())).await?;
            let project_id = agent
                .with_project(|p| Ok(p.project_id().to_string()))
                .await?;
            let client =
                codescout::retrieval::client::RetrievalClient::from_env(Some(&root)).await?;
            let opts = codescout::retrieval::sync::SyncOpts {
                force_reindex: force,
                record_index_state: true,
                ignore_patterns: codescout::config::project::ProjectConfig::load_or_default(&root)
                    .map(|c| c.ignored_paths.patterns)
                    .unwrap_or_default(),
                ..Default::default()
            };
            let report = client.sync_project(&project_id, &root, opts).await?;
            println!("{report}");
        }
        Commands::MigrateMemories {
            project,
            db_path,
            dry_run,
            in_place,
        } => {
            let root = project
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let db_path = db_path.unwrap_or_else(|| root.join(".codescout/embeddings/project.db"));

            // Activate the project to resolve project_id + bring up the
            // semantic memory store via the same path the MCP server uses.
            let agent = codescout::agent::Agent::new(Some(root.clone())).await?;
            let project_id = agent
                .with_project(|p| Ok(p.project_id().to_string()))
                .await?;
            let store = agent.semantic_memory_store().await?;

            // Build the embedder once — re-embedding happens per-row inside
            // migrate_memories. Uses the same env-driven config as the server.
            let client =
                codescout::retrieval::client::RetrievalClient::from_env(Some(&root)).await?;
            // The budget must describe the embedder actually being wrapped, so it comes
            // from `client.config` — the same config `client.embedder` was built from —
            // and NOT from `p.config.embeddings.model`, which is a second copy of the
            // same setting that can diverge from it. Feeding the wrong one would segment
            // on a budget the live backend does not have.
            let budget_chars = codescout::embed::chunk_size_for_model(&client.config.model);
            let embedder = codescout::migrate::memories::HttpMigrationEmbedder::new(
                client.embedder,
                budget_chars,
            );

            let report = if in_place {
                tracing::info!(
                    "migrate-memories --in-place: project_id={project_id} dry_run={dry_run}"
                );
                let mut r = codescout::migrate::memories::reembed_memories_in_place(
                    store.as_ref(),
                    &embedder,
                    &project_id,
                    dry_run,
                )
                .await?;

                // The store-driven pass above cannot see a memory whose point was never
                // written — it enumerates from the store, and that is exactly the damage
                // a failed cross-embed leaves. Disk is the only side that sees them, so
                // --in-place runs both passes and finally does what its name claims:
                // every memory's vector re-derived from current config.
                // docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md
                let disk = codescout::memory::MemoryStore::open(&root)?;
                let missing = codescout::migrate::memories::embed_missing_memories(
                    &disk,
                    store.as_ref(),
                    &embedder,
                    &project_id,
                    dry_run,
                )
                .await?;
                tracing::info!(
                    "migrate-memories --in-place: {} re-derived, {} newly embedded from disk",
                    r.upserted,
                    missing.upserted
                );
                r.read += missing.read;
                r.upserted += missing.upserted;
                r.skipped += missing.skipped;
                r
            } else {
                tracing::info!(
                    "migrate-memories: src={} project_id={} dry_run={}",
                    db_path.display(),
                    project_id,
                    dry_run,
                );
                codescout::migrate::memories::migrate_memories(
                    &db_path,
                    store.as_ref(),
                    &embedder,
                    &project_id,
                    dry_run,
                )
                .await?
            };

            println!(
                "{}",
                serde_json::json!({
                    "read": report.read,
                    "upserted": report.upserted,
                    "skipped": report.skipped,
                    "anchors_attached": report.anchors_attached,
                    "dry_run": report.dry_run,
                    "mode": if in_place { "in-place-reembed" } else { "legacy-import" },
                    "next_step": match (report.dry_run, in_place) {
                        (true, _) => "Re-run without --dry-run to perform the upserts.",
                        (false, true) => "Every memory vector was re-derived from current embedding config. Check `skipped` — those rows kept their previous vectors and are still on the old convention.",
                        (false, false) => "Verify recall works against the new store, then delete .codescout/embeddings.db when satisfied.",
                    },
                })
            );
        }
        Commands::Version => {
            let info = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "git_sha": env!("CODESCOUT_GIT_SHA"),
                "git_sha_full": env!("CODESCOUT_GIT_SHA_FULL"),
                "git_dirty": env!("CODESCOUT_GIT_DIRTY") == "1",
            });
            println!("{info}");
        }
        #[cfg(feature = "dashboard")]
        Commands::Dashboard {
            project,
            host,
            port,
            no_open,
        } => {
            let root = project
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            tracing::info!("Launching dashboard for {}", root.display());
            codescout::dashboard::serve(root, host, port, !no_open).await?;
        }
        #[cfg(feature = "librarian")]
        Commands::Artifact { verb } => {
            codescout::cli::artifact::dispatch(verb).await?;
        }
        #[cfg(feature = "librarian")]
        Commands::ArtifactEvent { verb } => {
            codescout::cli::artifact_event::dispatch(verb).await?;
        }
        #[cfg(feature = "librarian")]
        Commands::ArtifactRefresh { verb } => {
            codescout::cli::artifact_refresh::dispatch(verb).await?;
        }
        #[cfg(feature = "librarian")]
        Commands::ArtifactAugment(args) => {
            codescout::cli::artifact_augment::run(args).await?;
        }
        #[cfg(feature = "librarian")]
        Commands::AuditDocRefs(args) => {
            codescout::cli::audit_doc_refs::run(args).await?;
        }
        #[cfg(feature = "librarian")]
        Commands::Doctor(args) => {
            codescout::cli::doctor::run(args).await?;
        }
        #[cfg(feature = "librarian")]
        Commands::BackfillChunks(args) => {
            codescout::cli::backfill_chunks::run(args).await?;
        }
        #[cfg(feature = "librarian")]
        Commands::ConstitutionCheck(args) => {
            codescout::cli::constitution_check::run(args).await;
        }
        Commands::OperatorRules { mode, ledger } => {
            use codescout::operator_rules as ops;
            let path = ledger.unwrap_or_else(|| ops::LEDGER_PATH.into());
            let doc = std::fs::read_to_string(&path)
                .with_context(|| format!("reading ledger {}", path.display()))?;
            let profiles = ops::OperatorProfiles::from_env()?;
            match mode.as_str() {
                "compile" => {
                    let written = ops::compile(&doc, &profiles)?;
                    if written.is_empty() {
                        println!(
                            "operator-rules: already current in all {} profiles",
                            profiles.paths.len()
                        );
                    } else {
                        for p in &written {
                            println!("operator-rules: wrote {}", p.display());
                        }
                    }
                }
                "check" => {
                    let drift = ops::check(&doc, &profiles)?;
                    for d in &drift {
                        eprintln!("operator-rules: DRIFT {} — {}", d.path.display(), d.reason);
                    }
                    if drift.is_empty() {
                        println!(
                            "operator-rules: all {} profiles current",
                            profiles.paths.len()
                        );
                    }
                    std::process::exit(ops::exit_code(&drift));
                }
                _ => unreachable!("clap value_parser restricts mode"),
            }
        }
        #[cfg(unix)]
        Commands::Mux {
            socket,
            lock,
            cwd,
            idle_timeout,
            server_env,
            server_cmd,
        } => {
            codescout::lsp::mux::process::run(
                &socket,
                &lock,
                &cwd,
                idle_timeout,
                &server_cmd[0],
                &server_cmd[1..],
                &server_env,
            )
            .await?;
        }
        #[cfg(unix)]
        Commands::PeerServe {
            socket,
            workspace,
            read_only,
            idle_timeout,
        } => {
            let socket = socket.unwrap_or_else(|| {
                codescout::socket_discovery::peer_socket_path_for_workspace(&workspace)
            });
            codescout::peer::server::run(&socket, &workspace, read_only, idle_timeout).await?;
        }
    }

    Ok(())
}
