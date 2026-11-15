//! The shape of an augmentation: its sidecar and its declaration (D-R24).
//!
//! [`plan_shape`] compares the committed sidecar with the catalog row instead of skipping an
//! existing one: a sidecar that says the same thing is already exported, one that says something
//! else is reported as drift and never overwritten.

use std::path::Path;

use crate::librarian::augmentation_sidecar::{
    self as sidecar, declaration_stamped, drifting_fields, render, AugmentationSidecar,
};
use crate::librarian::catalog::augmentation::AugmentationRow;
use crate::librarian::frontmatter;
use crate::librarian::tools::doctor::{parse_declaration, Declaration};

/// What export does with one augmentation's shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeExport {
    /// The committed sidecar equals the catalog's shape. `stamped_doc` is the document with its
    /// declaration set, when the document does not declare the sidecar yet.
    AlreadyExported { stamped_doc: Option<String> },
    /// No sidecar is committed: write it and stamp the declaration.
    Export {
        sidecar_rel: String,
        sidecar_text: String,
        stamped_doc: String,
    },
    /// The committed sidecar differs from the catalog's shape in these fields.
    Drift { fields: Vec<&'static str> },
    /// Nothing can be exported; the text says why.
    Refused(String),
}

/// Plan the export of `row`'s shape for the document `doc_rel` holding `doc_text`.
/// `head_sidecar` is the text of the sidecar at [`sidecar_rel`] in `HEAD`, when there is one.
pub fn plan_shape(
    doc_rel: &str,
    doc_text: &str,
    row: &AugmentationRow,
    head_sidecar: Option<&str>,
) -> ShapeExport {
    let rel = sidecar_rel(doc_rel);
    let declared = match frontmatter::parse(doc_text) {
        Err(e) => return ShapeExport::Refused(format!("frontmatter does not parse: {e}")),
        Ok((None, _)) => {
            return ShapeExport::Refused(
                "no frontmatter block: the declaration has nowhere to go".to_owned(),
            )
        }
        Ok((Some(fm), _)) => fm
            .extra
            .get("expects_augmentation")
            .is_some_and(|v| {
                matches!(parse_declaration(v), Declaration::Declared { sidecar: Some(s) } if s == rel)
            }),
    };
    let stamp = || declaration_stamped(doc_text, &rel);
    let shape = AugmentationSidecar::from_row(row);
    let Some(committed) = head_sidecar else {
        return match (render(&shape), stamp()) {
            (Ok(sidecar_text), Ok(stamped_doc)) => ShapeExport::Export {
                sidecar_rel: rel,
                sidecar_text,
                stamped_doc,
            },
            (Err(e), _) | (_, Err(e)) => ShapeExport::Refused(format!("{e:#}")),
        };
    };
    let committed: AugmentationSidecar = match serde_yml::from_str(committed) {
        Ok(parsed) => parsed,
        Err(_) => {
            return ShapeExport::Refused(format!("the committed sidecar {rel} does not parse"));
        }
    };
    let fields = drifting_fields(row, &committed);
    if !fields.is_empty() {
        return ShapeExport::Drift { fields };
    }
    if declared {
        return ShapeExport::AlreadyExported { stamped_doc: None };
    }
    match stamp() {
        Ok(stamped_doc) => ShapeExport::AlreadyExported {
            stamped_doc: Some(stamped_doc),
        },
        Err(e) => ShapeExport::Refused(format!("{e:#}")),
    }
}

/// The sidecar path the frontmatter of `doc_text` declares (`expects_augmentation: <path>.yaml`).
pub fn declared_sidecar(doc_text: &str) -> Option<String> {
    let (fm, _) = frontmatter::parse(doc_text).ok()?;
    let value = fm?.extra.get("expects_augmentation").cloned()?;
    match parse_declaration(&value) {
        Declaration::Declared { sidecar } => sidecar,
        _ => None,
    }
}

/// Compare the catalog shape with the committed sidecar `committed` that the document declares at
/// `rel`, a path other than the one its name gives. `None` when they agree; `Drift` or `Refused`
/// otherwise.
pub fn compare_declared(row: &AugmentationRow, committed: &str, rel: &str) -> Option<ShapeExport> {
    let parsed: AugmentationSidecar = match serde_yml::from_str(committed) {
        Ok(parsed) => parsed,
        Err(_) => {
            return Some(ShapeExport::Refused(format!(
                "the committed sidecar {rel} does not parse"
            )));
        }
    };
    let fields = drifting_fields(row, &parsed);
    (!fields.is_empty()).then_some(ShapeExport::Drift { fields })
}

/// The repo-relative path of the sidecar of the document at `doc_rel`.
pub fn sidecar_rel(doc_rel: &str) -> String {
    sidecar::rel_path_for(Path::new(""), Path::new(doc_rel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::augmentation_sidecar::{render, AugmentationSidecar};
    use crate::librarian::frontmatter;

    const DOC: &str = "---\nkind: tracker\nstatus: open\n---\n\n# T\n\nbody\n";
    const DOC_REL: &str = "docs/trackers/t.md";
    const SIDECAR_REL: &str = "docs/augmentations/docs-trackers-t.yaml";

    fn row() -> AugmentationRow {
        AugmentationRow {
            artifact_id: "abc123".into(),
            prompt: "Maintain the T-N table.".into(),
            params: r#"{"observations":[{"id":"T-1"}]}"#.into(),
            last_refreshed_at: None,
            refresh_count: 7,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: "2026-08-30T00:00:00Z".into(),
            render_template: Some("{{ observations | length }}".into()),
            params_schema: Some(r#"{"type":"object"}"#.into()),
            append_mode: true,
            history_cap: Some(20),
            entry_collection: Some("observations".into()),
            refreshed_at_commit: None,
        }
    }

    fn sidecar_text(row: &AugmentationRow) -> String {
        render(&AugmentationSidecar::from_row(row)).unwrap()
    }

    /// The document's declaration, as `expects_augmentation` reads in its frontmatter.
    fn declared(doc: &str) -> Option<String> {
        let (fm, _) = frontmatter::parse(doc).unwrap();
        fm?.extra
            .get("expects_augmentation")
            .and_then(|v| v.as_str().map(str::to_owned))
    }

    #[test]
    fn a_shape_is_exported_and_declared() {
        let row = row();
        let ShapeExport::Export {
            sidecar_rel,
            sidecar_text: text,
            stamped_doc,
        } = plan_shape(DOC_REL, DOC, &row, None)
        else {
            panic!("no sidecar in HEAD: the shape is exported");
        };
        assert_eq!(sidecar_rel, SIDECAR_REL);
        assert_eq!(text, sidecar_text(&row));
        assert_eq!(declared(&stamped_doc).as_deref(), Some(SIDECAR_REL));
        assert!(stamped_doc.ends_with("\n# T\n\nbody\n"), "{stamped_doc}");
        // The sidecar rel is the one the planner asks HEAD for.
        assert_eq!(super::sidecar_rel(DOC_REL), SIDECAR_REL);
    }

    #[test]
    fn an_equal_declared_sidecar_is_already_exported() {
        let row = row();
        let doc = format!("---\nkind: tracker\nexpects_augmentation: {SIDECAR_REL}\n---\n\n# T\n");
        let text = sidecar_text(&row);
        assert_eq!(
            plan_shape(DOC_REL, &doc, &row, Some(&text)),
            ShapeExport::AlreadyExported { stamped_doc: None }
        );
        // Equal by what it says, not by bytes: a hand-formatted file with a comment, an older
        // schema version and the keys in another order is the same shape.
        let hand = "# kept by hand\nhistory_cap: 20\nschema_version: 0\nappend_mode: true\n\
                    render_template: '{{ observations | length }}'\nparams_schema:\n  type: object\n\
                    entry_collection: observations\nprompt: Maintain the T-N table.\n";
        assert_eq!(
            plan_shape(DOC_REL, &doc, &row, Some(hand)),
            ShapeExport::AlreadyExported { stamped_doc: None }
        );
    }

    #[test]
    fn an_equal_undeclared_sidecar_gets_only_the_stamp() {
        let row = row();
        let text = sidecar_text(&row);
        let ShapeExport::AlreadyExported {
            stamped_doc: Some(stamped),
        } = plan_shape(DOC_REL, DOC, &row, Some(&text))
        else {
            panic!("an equal sidecar is not rewritten; the declaration is stamped");
        };
        assert_eq!(declared(&stamped).as_deref(), Some(SIDECAR_REL));
        // A declaration that names another file is replaced the same way.
        let other = DOC.replace(
            "status: open",
            "expects_augmentation: docs/augmentations/old.yaml",
        );
        let ShapeExport::AlreadyExported {
            stamped_doc: Some(stamped),
        } = plan_shape(DOC_REL, &other, &row, Some(&text))
        else {
            panic!("the declaration names another sidecar");
        };
        assert_eq!(declared(&stamped).as_deref(), Some(SIDECAR_REL));
        // `expects_augmentation: true` declares no shape: it is stamped too.
        let bare = DOC.replace("status: open", "expects_augmentation: true");
        assert!(matches!(
            plan_shape(DOC_REL, &bare, &row, Some(&text)),
            ShapeExport::AlreadyExported {
                stamped_doc: Some(_)
            }
        ));
    }

    #[test]
    fn a_differing_sidecar_is_reported_and_not_overwritten() {
        let row = row();
        let mut committed = row.clone();
        committed.prompt = "An older prompt.".into();
        let text = sidecar_text(&committed);
        assert_eq!(
            plan_shape(DOC_REL, DOC, &row, Some(&text)),
            ShapeExport::Drift {
                fields: vec!["prompt"]
            }
        );
        committed.history_cap = None;
        committed.append_mode = false;
        let text = sidecar_text(&committed);
        assert_eq!(
            plan_shape(DOC_REL, DOC, &row, Some(&text)),
            ShapeExport::Drift {
                fields: vec!["prompt", "append_mode", "history_cap"]
            }
        );
        // A committed sidecar that does not parse is never replaced either.
        let refused = plan_shape(DOC_REL, DOC, &row, Some("prompt: [unclosed"));
        let ShapeExport::Refused(why) = refused else {
            panic!("a sidecar that does not parse is refused");
        };
        assert!(why.contains("does not parse"), "{why}");
    }

    #[test]
    fn a_document_without_frontmatter_is_refused() {
        let row = row();
        let ShapeExport::Refused(why) = plan_shape(DOC_REL, "# T\n\nbody\n", &row, None) else {
            panic!("a declaration needs a frontmatter block");
        };
        assert!(why.contains("no frontmatter block"), "{why}");
        let broken = "---\nkind: [unclosed\n---\n\nbody\n";
        let ShapeExport::Refused(why) = plan_shape(DOC_REL, broken, &row, None) else {
            panic!("a block that does not parse is refused");
        };
        assert!(why.contains("frontmatter"), "{why}");
        // A document that cannot take the declaration is refused before its sidecar is compared:
        // a differing committed sidecar is not reported as drift of a shape nobody can declare.
        let committed = sidecar_text(&AugmentationRow {
            prompt: "An older prompt.".into(),
            ..row.clone()
        });
        assert!(matches!(
            plan_shape(DOC_REL, "# T\n", &row, Some(&committed)),
            ShapeExport::Refused(_)
        ));
    }

    #[test]
    fn a_declared_sidecar_path_is_read_from_the_frontmatter() {
        let doc = |declaration: &str| format!("---\nkind: tracker\n{declaration}\n---\n\n# T\n");
        assert_eq!(
            declared_sidecar(&doc("expects_augmentation: docs/augmentations/x.yaml")).as_deref(),
            Some("docs/augmentations/x.yaml")
        );
        for none in [
            "expects_augmentation: true",
            "expects_augmentation: no",
            "status: open",
        ] {
            assert_eq!(declared_sidecar(&doc(none)), None, "{none}");
        }
        assert_eq!(declared_sidecar("# T\n"), None);
        assert_eq!(declared_sidecar("---\nkind: [unclosed\n---\n"), None);
    }

    #[test]
    fn a_sidecar_declared_at_another_path_is_compared_with_the_shape() {
        let row = row();
        let equal = sidecar_text(&row);
        assert_eq!(
            compare_declared(&row, &equal, "docs/augmentations/old.yaml"),
            None
        );
        let mut older = row.clone();
        older.prompt = "An older prompt.".into();
        assert_eq!(
            compare_declared(&row, &sidecar_text(&older), "docs/augmentations/old.yaml"),
            Some(ShapeExport::Drift {
                fields: vec!["prompt"]
            })
        );
        let Some(ShapeExport::Refused(why)) =
            compare_declared(&row, "prompt: [unclosed", "docs/augmentations/old.yaml")
        else {
            panic!("a sidecar that does not parse is refused");
        };
        assert!(why.contains("docs/augmentations/old.yaml"), "{why}");
        assert!(why.contains("does not parse"), "{why}");
    }
}
