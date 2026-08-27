//! Operator rules — engine 5 of the rules programme.
//!
//! Rules that hold across every project, tool and model for one operator. Rules
//! bound `always` compile into a delimited block in each Claude Code profile's
//! `CLAUDE.md`; rules bound `triggered` are routed at runtime in Phase 2.
//!
//! Spec: `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md`.

pub mod budget;
pub mod rule;
pub mod validate;
