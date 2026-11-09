//! Local observation control and bounded hook ingestion, without server startup.
use clap::Subcommand;
use std::path::PathBuf;
#[derive(Debug, Subcommand)]
pub enum Verb {
    Enable {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Disable {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
    },
    IngestHook {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Status {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
    },
}
#[cfg(target_os = "linux")]
pub fn run(verb: Verb) -> anyhow::Result<()> {
    use crate::observations::store::{
        self, Limits, ObservationStore, SanitizedEvent, StoreAvailability,
    };
    use std::io::Read;
    let error = |code: String| anyhow::anyhow!("observation capture: {code}");
    match verb {
        Verb::Enable { workspace, .. } => {
            let epoch = store::enable(&workspace).map_err(error)?;
            println!(
                "{}",
                serde_json::json!({"schema_version":1,"enabled":true,"store_epoch":epoch,"coverage":"unknown"})
            );
        }
        Verb::Disable { workspace, .. } => {
            store::disable(&workspace).map_err(error)?;
            println!(
                "{}",
                serde_json::json!({"schema_version":1,"enabled":false,"coverage":"unknown"})
            );
        }
        Verb::Status { workspace, .. } => {
            let r = store::read_store(&workspace, Limits::default());
            println!(
                "{}",
                serde_json::json!({"schema_version":1,"enabled":r.enabled,"store_epoch":r.store_epoch,"record_count":r.records.len(),"diagnostics":r.diagnostics,"coverage":"unknown"})
            );
        }
        Verb::IngestHook { workspace, .. } => {
            // Check opt-in before consuming or parsing input; disabled means no journal write.
            let store = match ObservationStore::open_if_enabled(&workspace, Limits::default()) {
                StoreAvailability::Disabled => {
                    println!("{{\"capture\":\"disabled\"}}");
                    return Ok(());
                }
                StoreAvailability::Unavailable(d) => return Err(error(d)),
                StoreAvailability::Enabled(s) => s,
            };
            let mut bytes = Vec::new();
            std::io::stdin().take(65537).read_to_end(&mut bytes)?;
            if bytes.len() > 65536 {
                anyhow::bail!("observation capture: incomplete-input");
            }
            let event: SanitizedEvent =
                serde_json::from_slice(&bytes).map_err(|_| error("incomplete-input".into()))?;
            if event.producer_kind != "codex-hook"
                || !["hook-pre", "hook-post", "lifecycle", "observer-diagnostic"]
                    .contains(&event.event.as_str())
            {
                anyhow::bail!("observation capture: invalid-context");
            }
            let receipt = store.append(event).map_err(error)?;
            println!(
                "{}",
                serde_json::json!({"capture":"recorded","observation_id":receipt.observation.observation_id})
            );
        }
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
pub fn run(_verb: Verb) -> anyhow::Result<()> {
    anyhow::bail!("observation capture unavailable on this platform")
}
