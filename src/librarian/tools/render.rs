//! MiniJinja rendering for augmentation `render_template`. Used by
//! `librarian_context` to project `params` into a markdown table/snippet so
//! the artifact body can stay prose-only.

use anyhow::{anyhow, Result};
use minijinja::Environment;
use serde_json::Value;

/// Render `template` over `params` (as the root context). Returns the
/// rendered markdown string. Errors are descriptive — they're surfaced to the
/// LLM via `librarian_context` so it can fix the template/params.
pub fn render_params(template: &str, params: &Value) -> Result<String> {
    let mut env = Environment::new();
    env.add_template("t", template)
        .map_err(|e| anyhow!("template parse error: {e}"))?;
    let tmpl = env.get_template("t").unwrap();
    tmpl.render(params)
        .map_err(|e| anyhow!("template render error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_simple_substitution() {
        let out = render_params("status: {{ status }}", &json!({"status": "green"})).unwrap();
        assert_eq!(out, "status: green");
    }

    #[test]
    fn renders_for_loop_over_list() {
        let template =
            "| id | status |\n|----|--------|\n{% for f in failures %}| {{ f.id }} | {{ f.status }} |\n{% endfor %}";
        let params = json!({
            "failures": [
                { "id": "F-1", "status": "fail" },
                { "id": "F-2", "status": "pass" }
            ]
        });
        let out = render_params(template, &params).unwrap();
        assert!(out.contains("| F-1 | fail |"));
        assert!(out.contains("| F-2 | pass |"));
    }

    #[test]
    fn parse_error_surfaces_message() {
        let err = render_params("{% for x in %}", &json!({})).unwrap_err();
        let s = err.to_string();
        assert!(s.contains("template parse error") || s.contains("template render error"));
    }

    #[test]
    fn missing_var_does_not_error_by_default() {
        // MiniJinja default: undefined renders as empty string, no error.
        let out = render_params("hello {{ who }}", &json!({})).unwrap();
        assert_eq!(out, "hello ");
    }

    #[test]
    fn renders_dict_iteration() {
        let template = "{% for k, v in flags|items %}{{ k }}={{ v }};{% endfor %}";
        let params = json!({ "flags": { "a": "on", "b": "off" } });
        let out = render_params(template, &params).unwrap();
        assert!(out.contains("a=on"));
        assert!(out.contains("b=off"));
    }
}
