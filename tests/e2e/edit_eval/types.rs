use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditAction {
    Replace,
    Insert,
    Remove,
    Rename,
}

#[derive(Debug, Clone)]
pub enum ReturnExpected {
    Ok,
    CleanError, // RecoverableError downcast
}

#[derive(Debug, Clone)]
pub enum CompilerExpected {
    Builds,
    Breaks, // intentional — case demonstrates tool faithfulness, not semantic protection
    DontCare,
}

/// A content invariant the post-edit fixture file must satisfy.
/// Multiple invariants are AND-ed.
#[derive(Debug, Clone)]
pub enum ContentInvariant {
    /// The post-edit content of `file` must contain `needle` exactly `count` times.
    Contains {
        file: &'static str,
        needle: &'static str,
        count: usize,
    },
    /// The post-edit content of `file` must NOT contain `needle`.
    NotContains {
        file: &'static str,
        needle: &'static str,
    },
    /// A specific byte-range or line-range must equal exact text.
    /// Used sparingly — narrow assertions over broad ones.
    LineEquals {
        file: &'static str,
        line: u32,
        text: &'static str,
    },
}

#[derive(Debug, Clone)]
pub struct Expected {
    pub return_: ReturnExpected,
    pub disk: Vec<ContentInvariant>,
    pub compiler: CompilerExpected,
}

#[derive(Debug, Clone)]
pub struct EditCase {
    pub id: &'static str,
    pub action: EditAction,
    pub input: Value,
    /// Fixture file the edit targets — used so the grader knows which file
    /// to read from disk. Relative to fixture src/.
    pub target_file: &'static str,
    pub expected: Expected,
    pub rationale: &'static str,
    /// If Some, this case is exempt from H1 hard-gate failure with the given
    /// LIMIT-comment reason. Used for BUG-054 sentinel R-02.
    pub h1_exempt: Option<&'static str>,
}
