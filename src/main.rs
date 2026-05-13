use anyhow::Result;
use clap::{Parser, Subcommand};

#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(Parser)]
#[command(name = "codescout", about = "High-performance coding agent MCP server")]
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

    /// Print the codescout git SHA, full SHA, and dirty status baked into this
    /// binary at build time. JSON output for use by the bench harness.
    Version,
}

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
            codescout::embed::index::build_index(&root, force, None).await?;
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
    }

    Ok(())
}
