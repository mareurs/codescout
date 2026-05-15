#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Correct,
    Partial,
    CleanError,
    SilentWrong,
    Corrupt,
    Hung,
    Panic,
}

impl Verdict {
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::Correct => "CORRECT",
            Verdict::Partial => "PARTIAL",
            Verdict::CleanError => "CLEAN_ERROR",
            Verdict::SilentWrong => "SILENT_WRONG",
            Verdict::Corrupt => "CORRUPT",
            Verdict::Hung => "HUNG",
            Verdict::Panic => "PANIC",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_labels_are_stable() {
        assert_eq!(Verdict::Correct.label(), "CORRECT");
        assert_eq!(Verdict::Partial.label(), "PARTIAL");
        assert_eq!(Verdict::CleanError.label(), "CLEAN_ERROR");
        assert_eq!(Verdict::SilentWrong.label(), "SILENT_WRONG");
        assert_eq!(Verdict::Corrupt.label(), "CORRUPT");
        assert_eq!(Verdict::Hung.label(), "HUNG");
        assert_eq!(Verdict::Panic.label(), "PANIC");
    }
}
