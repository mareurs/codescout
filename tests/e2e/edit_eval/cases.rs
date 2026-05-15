use crate::e2e::edit_eval::types::EditCase;
use std::sync::OnceLock;

static CASES: OnceLock<Vec<EditCase>> = OnceLock::new();

pub fn all() -> &'static [EditCase] {
    CASES.get_or_init(Vec::new)
}
