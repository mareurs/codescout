use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "librarian-mcp", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// One-shot import of workspace roots from codescout's project registry.
    ImportCodescout,
    /// Reindex the workspace without starting the MCP server.
    Reindex {
        #[arg(long)]
        repo: Option<String>,
        /// Wipe existing rows for affected repos before re-walking (forces re-title of all files).
        #[arg(long)]
        force: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        None => librarian_mcp::run_stdio_server().await,
        Some(Cmd::ImportCodescout) => librarian_mcp::import_codescout(),
        Some(Cmd::Reindex { repo, force }) => {
            librarian_mcp::reindex_cli(repo.as_deref(), force).await
        }
    }
}
