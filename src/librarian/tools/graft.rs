//! `doc(action="graft")` — fold one catalog row's history into another.
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{RecoverableError, ToolContext};

#[derive(Deserialize)]
struct Args {
    from_id: String,
    into_id: String,
    /// Omitted or false = dry run. See the gate in `call`.
    #[serde(default)]
    force: Option<bool>,
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args).map_err(|e| {
        RecoverableError::new(format!("graft requires 'from_id' and 'into_id': {e}"))
    })?;

    let mut cat = ctx.catalog.lock();

    // Dry-run gate. `graft` DELETES `from_id` — its events, links, observations and
    // augmentation move to `into_id` first, but the row itself is gone and the id stops
    // resolving. That is not visible from the call shape, where both ids read
    // symmetrically. Preview first; `force=true` applies.
    //
    // Measured 2026-09-03: `graft` ran once in 30 days. The round-trip is free and the
    // mistake it prevents is not — a graft in the wrong direction folds the row you meant
    // to keep into the one you meant to discard, and nothing rebuilds it.
    if !a.force.unwrap_or(false) {
        use crate::librarian::catalog::{artifact, augmentation, events, links, observations};
        let from = artifact::get(&cat, &a.from_id)?
            .ok_or_else(|| RecoverableError::new(format!("unknown from_id `{}`", a.from_id)))?;
        let into = artifact::get(&cat, &a.into_id)?
            .ok_or_else(|| RecoverableError::new(format!("unknown into_id `{}`", a.into_id)))?;
        return Ok(json!({
            "dry_run": true,
            "grafted": false,
            "from_id": a.from_id,
            "into_id": a.into_id,
            "would_delete_row": {
                "id": a.from_id,
                "title": from.title,
                "abs_path": from.abs_path.display().to_string(),
            },
            "would_survive": {
                "id": a.into_id,
                "title": into.title,
                "abs_path": into.abs_path.display().to_string(),
            },
            "would_move": {
                "augmentation": augmentation::get(&cat, &a.from_id)?.is_some(),
                "links_out": links::outgoing(&cat, &a.from_id)?.len(),
                "links_in": links::incoming(&cat, &a.from_id)?.len(),
                "observations": observations::list_for_artifact(&cat, &a.from_id)?.len(),
                "has_events": events::latest_for_artifact(&cat, &a.from_id)?.is_some(),
            },
            "hint": format!(
                "check the direction — `{}` disappears. Re-run with force=true to apply.",
                a.from_id
            ),
        }));
    }

    let report = crate::librarian::catalog::graft::graft_rows(&mut cat, &a.from_id, &a.into_id)?;
    drop(cat);

    Ok(json!({
        "grafted": true,
        "from_id": a.from_id,
        "into_id": a.into_id,
        "report": report,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::TestArtifactRowBuilder;
    use crate::librarian::catalog::events::{self, TestEventRowBuilder};
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;
    use serde_json::json;

    #[tokio::test]
    async fn graft_action_folds_and_reports() {
        let cat = Catalog::open_in_memory().unwrap();
        for (id, p) in [("from", "/wt/x.md"), ("into", "/main/x.md")] {
            let row = TestArtifactRowBuilder::new(id)
                .with_abs_path(p)
                .with_kind("tracker")
                .with_title(id)
                .build();
            crate::librarian::catalog::artifact::upsert(&cat, &row).unwrap();
        }
        events::insert(
            &cat,
            &TestEventRowBuilder::new("from", "note")
                .with_id("e1")
                .build(),
        )
        .unwrap();
        let ctx = TestToolContextBuilder::new(cat).build();

        let out = call(
            &ctx,
            json!({"from_id":"from","into_id":"into","force":true}),
        )
        .await
        .unwrap();
        assert_eq!(out["grafted"], true);
        assert_eq!(out["report"]["events_repointed"], 1);
    }

    /// The gate, asserted on the EFFECT — `from_id` must still resolve afterwards.
    ///
    /// The preview names which row disappears because the call shape does not: `from_id`
    /// and `into_id` read symmetrically, and a graft in the wrong direction folds the row
    /// you meant to keep into the one you meant to discard.
    #[tokio::test]
    async fn graft_without_force_is_a_dry_run_and_names_the_row_that_disappears() {
        let cat = Catalog::open_in_memory().unwrap();
        for (id, p) in [("from", "/wt/x.md"), ("into", "/main/x.md")] {
            let row = TestArtifactRowBuilder::new(id)
                .with_abs_path(p)
                .with_kind("tracker")
                .with_title(id)
                .build();
            crate::librarian::catalog::artifact::upsert(&cat, &row).unwrap();
        }
        let ctx = TestToolContextBuilder::new(cat).build();

        let out = call(&ctx, json!({"from_id":"from","into_id":"into"}))
            .await
            .unwrap();

        assert_eq!(out["dry_run"], true);
        assert_eq!(out["grafted"], false);
        assert_eq!(
            out["would_delete_row"]["id"], "from",
            "the preview's whole job is naming which of two symmetric ids disappears"
        );
        assert_eq!(out["would_survive"]["id"], "into");

        let cat = ctx.catalog.lock();
        assert!(
            crate::librarian::catalog::artifact::get(&cat, "from")
                .unwrap()
                .is_some(),
            "dry run must not delete the source row"
        );
    }

    #[tokio::test]
    async fn graft_action_requires_both_ids() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = TestToolContextBuilder::new(cat).build();
        let err = call(&ctx, json!({"from_id":"a"})).await.unwrap_err();
        assert!(err.to_string().contains("into_id"));
    }
}
