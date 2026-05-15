pub mod proc;
pub mod report;
pub mod verdict;

pub use proc::{cargo_check, git_restore, read_fixture_file};
pub use report::{next_round_number, Report};
pub use verdict::Verdict;
