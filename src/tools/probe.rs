//! Debug-only probe tool to measure Claude Code's truncation cap on MCP
//! tool `description` fields. Registered only when env `CODESCOUT_PROBE=1`.
//!
//! The description embeds sentinel markers at known byte offsets so an
//! observer can ask the model which sentinels it sees and binary-search
//! the actual cap from outside the binary.

use anyhow::Result;
use serde_json::{json, Value};
use std::sync::LazyLock;

use crate::tools::core::{Tool, ToolContext};

pub struct ProbeTool;

static PROBE_DESCRIPTION: LazyLock<String> = LazyLock::new(build_probe_description);

fn build_probe_description() -> String {
    let sentinels: &[(usize, &str)] = &[
        (200, "SENTINEL_0200_AA"),
        (500, "SENTINEL_0500_BB"),
        (1000, "SENTINEL_1000_CC"),
        (1500, "SENTINEL_1500_DD"),
        (2000, "SENTINEL_2000_EE"),
        (2500, "SENTINEL_2500_FF"),
        (3000, "SENTINEL_3000_GG"),
        (4000, "SENTINEL_4000_HH"),
        (5000, "SENTINEL_5000_II"),
        (6000, "SENTINEL_6000_JJ"),
        (8000, "SENTINEL_8000_KK"),
    ];

    let mut s = String::with_capacity(9000);
    s.push_str(
        "PROBE_BEGIN: this is a diagnostic tool used to measure how much of an \
         MCP tool description reaches the model. The description embeds sentinel \
         markers at known byte offsets. Do NOT call this tool. If you are asked \
         which sentinels you can see in this description, list every SENTINEL_NNNN_XX \
         token you can find verbatim. ",
    );

    for (offset, marker) in sentinels {
        while s.len() + marker.len() + 4 < *offset {
            s.push_str("filler ");
        }
        s.push_str(marker);
        s.push(' ');
    }

    while s.len() < 8800 {
        s.push_str("filler ");
    }
    s.push_str("SENTINEL_END_C0FFEE");
    s
}

#[async_trait::async_trait]
impl Tool for ProbeTool {
    fn name(&self) -> &str {
        "__probe_description_cap__"
    }

    fn description(&self) -> &str {
        PROBE_DESCRIPTION.as_str()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn call(&self, _input: Value, _ctx: &ToolContext) -> Result<Value> {
        Ok(json!({
            "note": "probe tool — do not call. Inspect the tool's description field instead."
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_contains_all_sentinels() {
        let d = PROBE_DESCRIPTION.as_str();
        for marker in [
            "SENTINEL_0200_AA",
            "SENTINEL_0500_BB",
            "SENTINEL_1000_CC",
            "SENTINEL_1500_DD",
            "SENTINEL_2000_EE",
            "SENTINEL_2500_FF",
            "SENTINEL_3000_GG",
            "SENTINEL_4000_HH",
            "SENTINEL_5000_II",
            "SENTINEL_6000_JJ",
            "SENTINEL_8000_KK",
            "SENTINEL_END_C0FFEE",
        ] {
            assert!(d.contains(marker), "missing {marker}");
        }
    }

    #[test]
    fn sentinels_at_expected_offsets() {
        let d = PROBE_DESCRIPTION.as_str();
        for (target, marker) in [
            (200usize, "SENTINEL_0200_AA"),
            (500, "SENTINEL_0500_BB"),
            (1000, "SENTINEL_1000_CC"),
            (1500, "SENTINEL_1500_DD"),
            (2000, "SENTINEL_2000_EE"),
            (8000, "SENTINEL_8000_KK"),
        ] {
            let pos = d.find(marker).expect("marker present");
            assert!(
                pos.abs_diff(target) < 20,
                "marker {marker} at offset {pos}, expected near {target}"
            );
        }
    }
}
