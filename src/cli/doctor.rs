//! `cargo run -- doctor` — invoke the librarian catalog drift scanner.
//!
//! Thin CLI wrapper over `crate::librarian::tools::doctor::call`. Identical
//! discovery surface (project override, --json, --no-color); no
//! doctor-specific args yet because the scanner takes no input.

use anyhow::Result;
use clap::Args;
use serde_json::{Map, Value};

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[command(flatten)]
    pub common: CommonOpts,

    /// Exit 1 when the scanner reports any DEFECT — `summary.defects`, which is
    /// `summary.total` minus the rows the scanner itself declares informational
    /// (today: `claim_held_by_live_session`, which fires when a bug is correctly
    /// claimed by a session that is still running). An informational-only report
    /// exits 0: a healthy repo must not fail a gate. Default is to exit 0
    /// regardless — useful for monitoring without breaking CI.
    #[arg(long = "fail-on-violations")]
    pub fail_on_violations: bool,
}

/// The `--fail-on-violations` decision, extracted from [`run`] because `run` ends in
/// `std::process::exit(1)`, which no in-process test can observe. Inlined, the exit path
/// had no reachable assertion at all.
///
/// Gates on `summary.defects`, **not** `summary.total`: `total` counts every emitted row,
/// including the ones the scanner itself declares informational. Gating on `total` made
/// this command exit 1 on a healthy repo where one bug was correctly claimed by a live
/// session — the feature's success state failing a gate-shaped command.
///
/// `unwrap_or(0)`, and deliberately no fallback to `total`: a missing field must not
/// silently restore the behaviour this function exists to remove.
fn fails_the_gate(report: &Value, fail_on_violations: bool) -> bool {
    if !fail_on_violations {
        return false;
    }
    report
        .get("summary")
        .and_then(|s| s.get("defects"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
        > 0
}

pub async fn run(args: DoctorArgs) -> Result<()> {
    let common = args.common.clone();
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let v = crate::librarian::tools::doctor::call(&ctx, Value::Object(Map::new())).await?;

    // `summary.defects`, NOT `summary.total` — see `fails_the_gate`.
    crate::cli::format::print(&v, &output)?;

    if fails_the_gate(&v, args.fail_on_violations) {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The exit-1 path, which `run` reaches through `std::process::exit` and no
    /// in-process test can observe. `fails_the_gate` exists so this assertion is
    /// reachable at all.
    ///
    /// The first case is the regression: a report whose ONLY row is
    /// `claim_held_by_live_session` — a bug correctly claimed by a running session, the
    /// feature working — used to exit 1, because the gate read `summary.total`.
    #[test]
    fn an_informational_only_report_does_not_trip_the_exit_1_path() {
        // Shaped exactly as `doctor::call` emits it: one emitted row, zero defects.
        let informational_only = json!({
            "summary": { "total": 1, "defects": 0, "informational": 1, "shown": 1 }
        });
        assert!(
            !fails_the_gate(&informational_only, true),
            "a healthy repo whose only finding is informational must exit 0"
        );

        // Positive control: without it, `fails_the_gate` hardwired to `false` passes.
        let one_defect = json!({
            "summary": { "total": 2, "defects": 1, "informational": 1, "shown": 2 }
        });
        assert!(
            fails_the_gate(&one_defect, true),
            "a real defect must still fail the gate"
        );

        // The flag still gates everything: default is exit 0 regardless.
        assert!(!fails_the_gate(&one_defect, false));

        // A report missing the field must not fall back to `total` — that fallback is
        // precisely the behaviour this function was written to remove.
        let no_field = json!({ "summary": { "total": 9 } });
        assert!(!fails_the_gate(&no_field, true));
    }
}
