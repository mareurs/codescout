use super::ToolContext;
use crate::librarian::catalog::rekey::{self, RekeyMode};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct Args {
    id: String,
    from: String,
    to: String,
    #[serde(default)]
    force: bool,
}

/// Move a ledger's entry-id namespace from one prefix to another, atomically.
///
/// The counterpart `update_entry` deliberately refuses. That refusal
/// (`augmentation.rs:340`) is right about its reason — entry ids key `entry_cite` rows, so
/// re-keying one through a field patch strands its citations — and its *remedy* was the half
/// that did not hold: "append a new entry and mark this one superseded" doubles the ledger
/// and leaves the colliding headings defined, so it does not repair the collision it is
/// offered for. This action is the surface that moves the citations WITH the id.
///
/// **Dry run by default**, like `graft` and `delete`. The preview is the same code path with
/// writes suppressed, not a second implementation that counts — see
/// [`RekeyMode`](crate::librarian::catalog::rekey::RekeyMode) for why that distinction is
/// load-bearing here.
///
/// Motivating case: `docs/issues/2026-09-02-prefix-t-collides-again-with-the-zero-padding-protection-gone.md`.
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args).map_err(|e| {
        crate::tools::RecoverableError::with_hint(
            format!("doc(action=\"rekey_prefix\") requires 'id', 'from' and 'to': {e}"),
            "e.g. doc(action=\"rekey_prefix\", id=\"<16-hex>\", from=\"T\", to=\"SRI\"). \
             Dry run by default — read the report, then pass force=true to apply.",
        )
    })?;

    let mode = if a.force {
        RekeyMode::Apply
    } else {
        RekeyMode::Preview
    };
    let mut cat = ctx.catalog.lock();
    let target = super::worktree::resolve_write_target(&mut cat, ctx, &a.id)?;
    let report = rekey::rekey_prefix_rows(&mut cat, &target, &a.from, &a.to, mode)?;

    let mut out = json!({
        "artifact_id": target,
        "from": a.from,
        "to": a.to,
        "applied": a.force,
        "moved": report,
    });

    if !a.force {
        out["hint"] = json!(
            "Dry run — nothing was written. Every refusal this action can raise has already \
             been evaluated, so an Ok preview means the apply would also succeed. Re-run with \
             force=true to apply."
        );
    } else {
        // The external half this action deliberately does NOT touch, said at the moment the
        // reader can act on it. Citing files are other artifacts — on a shared checkout they
        // may be other sessions' live work, and across an umbrella they may be other REPOS.
        // `link_scan` re-derives the scan-origin edges from prose, so until that prose is
        // repointed the citations are genuinely unresolved and `doctor` will say so. That is
        // the honest intermediate state, not a defect to paper over.
        out["next_step"] = json!(
            "Citing files outside this ledger were not touched. Repoint them, then run \
             librarian(action=\"link_scan\", write=true) to re-derive the citation edges."
        );
    }
    Ok(out)
}
