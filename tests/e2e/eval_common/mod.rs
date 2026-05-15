pub mod proc;
pub mod verdict;

#[allow(unused_imports)]
pub use proc::{cargo_check, git_restore, read_fixture_file};
pub use verdict::Verdict;
