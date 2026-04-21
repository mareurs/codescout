use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Frontmatter {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub owners: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub time_scope: Option<String>,
}

pub fn parse(doc: &str) -> Result<(Option<Frontmatter>, &str)> {
    let looks_like_fm = doc.starts_with("---\n") || doc.starts_with("---\r\n");
    if !looks_like_fm {
        return Ok((None, doc));
    }
    let after_open = if doc.starts_with("---\r\n") { 5 } else { 4 };
    let rest = &doc[after_open..];
    let mut idx = 0usize;
    let mut close = None;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            close = Some((idx, idx + line.len()));
            break;
        }
        idx += line.len();
    }
    let (yaml_end, body_start) =
        close.ok_or_else(|| anyhow::anyhow!("frontmatter missing closing `---`"))?;
    let yaml_src = &rest[..yaml_end];
    let fm: Frontmatter = serde_yml::from_str(yaml_src)
        .map_err(|e| anyhow::anyhow!("malformed frontmatter YAML: {e}"))?;
    Ok((Some(fm), &rest[body_start..]))
}

pub fn write(fm: &Frontmatter, body: &str) -> String {
    let yaml = serde_yml::to_string(fm).expect("frontmatter serializes");
    format!("---\n{yaml}---\n{body}")
}

pub fn update_in_place(doc: &str, edit: impl FnOnce(&mut Frontmatter)) -> Result<String> {
    let (fm_opt, body) = parse(doc)?;
    let mut fm = fm_opt.unwrap_or_default();
    edit(&mut fm);
    Ok(write(&fm, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_frontmatter() {
        let doc = "---\nkind: spec\nstatus: active\ntitle: Example\n---\n\nBody here\n";
        let (fm, body) = parse(doc).unwrap();
        let fm = fm.expect("frontmatter present");
        assert_eq!(fm.kind.as_deref(), Some("spec"));
        assert_eq!(fm.status.as_deref(), Some("active"));
        assert_eq!(fm.title.as_deref(), Some("Example"));
        assert_eq!(body, "\nBody here\n");
    }

    #[test]
    fn returns_none_for_no_frontmatter() {
        let doc = "# just a heading\n\nbody\n";
        let (fm, body) = parse(doc).unwrap();
        assert!(fm.is_none());
        assert_eq!(body, doc);
    }

    #[test]
    fn handles_trailing_crlf() {
        let doc = "---\r\nkind: plan\r\n---\r\n\r\nbody\r\n";
        let (fm, _) = parse(doc).unwrap();
        assert_eq!(fm.unwrap().kind.as_deref(), Some("plan"));
    }

    #[test]
    fn rejects_missing_closing_delimiter() {
        let doc = "---\nkind: spec\n\nbody without close\n";
        let err = parse(doc).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("closing"));
    }

    #[test]
    fn rejects_malformed_yaml() {
        let doc = "---\nkind: [unclosed\n---\nbody\n";
        assert!(parse(doc).is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        let doc = "---\nkind: spec\nbogus: nope\n---\nbody\n";
        assert!(parse(doc).is_err());
    }

    #[test]
    fn round_trip_preserves_body() {
        let fm = Frontmatter {
            kind: Some("spec".into()),
            status: Some("active".into()),
            title: Some("X".into()),
            ..Default::default()
        };
        let body = "\nBody text\n";
        let doc = write(&fm, body);
        let (parsed, parsed_body) = parse(&doc).unwrap();
        assert_eq!(parsed.unwrap(), fm);
        assert_eq!(parsed_body, body);
    }

    #[test]
    fn update_in_place_preserves_untouched_fields() {
        let doc = "---\nkind: spec\nstatus: draft\ntitle: Original\n---\n\nbody\n";
        let updated = update_in_place(doc, |fm| {
            fm.status = Some("active".into());
        })
        .unwrap();
        assert!(updated.contains("status: active"));
        assert!(updated.contains("title: Original"));
        assert!(updated.ends_with("\nbody\n"));
    }

    #[test]
    fn update_in_place_inserts_frontmatter_if_absent() {
        let doc = "# Heading\n\nbody\n";
        let updated = update_in_place(doc, |fm| {
            fm.kind = Some("doc".into());
        })
        .unwrap();
        assert!(updated.starts_with("---\n"));
        assert!(updated.contains("kind: doc"));
        assert!(updated.ends_with("# Heading\n\nbody\n"));
    }
}
