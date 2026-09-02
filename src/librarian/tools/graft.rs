//! `doc(action="graft")` — fold one catalog row's history into another.
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{RecoverableError, ToolContext};

#[derive(Deserialize)]
struct Args {
    from_id: String,
    into_id: String,
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args).map_err(|e| {
        RecoverableError::new(format!("graft requires 'from_id' and 'into_id': {e}"))
    })?;

    let mut cat = ctx.catalog.lock();
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

        let out = call(&ctx, json!({"from_id":"from","into_id":"into"}))
            .await
            .unwrap();
        assert_eq!(out["grafted"], true);
        assert_eq!(out["report"]["events_repointed"], 1);
    }

    #[tokio::test]
    async fn graft_action_requires_both_ids() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = TestToolContextBuilder::new(cat).build();
        let err = call(&ctx, json!({"from_id":"a"})).await.unwrap_err();
        assert!(err.to_string().contains("into_id"));
    }
}
