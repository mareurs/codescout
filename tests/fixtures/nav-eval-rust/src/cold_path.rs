//! `cold` is only reachable from `#[cfg(test)]` code. references must
//! return at least one ref (the cfg-test caller) — confirms the scope
//! includes test-config code.

#![allow(dead_code)]

pub fn cold() -> u32 { 7 }

#[cfg(test)]
mod tests {
    use super::cold;
    #[test]
    fn smoke() { assert_eq!(cold(), 7); }
}
