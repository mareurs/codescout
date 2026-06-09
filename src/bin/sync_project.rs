use anyhow::Result;
use codescout::retrieval::{client::RetrievalClient, sync::SyncOpts};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let project_path = args
        .next()
        .expect("Usage: sync-project <project-path> [project-id]");
    let root = PathBuf::from(&project_path);
    let project_id = args
        .next()
        .unwrap_or_else(|| root.file_name().unwrap().to_string_lossy().into_owned());

    eprintln!("Connecting to retrieval stack...");
    let client = RetrievalClient::from_env().await?;

    let opts = SyncOpts {
        languages: None,
        force_reindex: false,
        record_index_state: true,
    };

    eprintln!(
        "Syncing project '{}' from {} ...",
        project_id,
        root.display()
    );
    let report = client.sync_project(&project_id, &root, opts).await?;

    println!(
        "done: +{} -{} ~{} chunks in {}ms",
        report.added, report.deleted, report.updated, report.elapsed_ms
    );
    Ok(())
}
