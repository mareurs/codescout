//! The declarative probe-row table.
//!
//! DATA, not logic, and in its own file on purpose: `tests/result_caps.rs`
//! scans this file as TEXT to cross-check ids against `RESULT_CAP`
//! annotations. A text scan is not fastidiousness — `librarian` is a default
//! feature, so rows behind `#[cfg(feature = "librarian")]` compile out under
//! `--no-default-features`, and a gate reading compiled symbols would red on
//! the lean lane while passing on the default one. That is a failure reached
//! by FOLLOWING `CLAUDE.md`'s gate order.
