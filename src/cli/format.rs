//! Output formatter for the CLI. Pretty by default, JSON via `--json`.

use anyhow::Result;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default)]
pub struct OutputOpts {
    pub json: bool,
    pub no_color: bool,
}

pub fn print(value: &Value, opts: &OutputOpts) -> Result<()> {
    // Phase 1 placeholder — replaced in CLI-3.
    let _ = (value, opts);
    Ok(())
}
