//! Two `parse<T>` functions in different submodules, identical bounds.
//! Trap: symbol-name search must return BOTH, not just one.

pub mod left {
    use std::str::FromStr;
    pub fn parse<T: FromStr>(s: &str) -> Option<T> { s.parse().ok() }
}

pub mod right {
    use std::str::FromStr;
    pub fn parse<T: FromStr>(s: &str) -> Option<T> { s.parse().ok() }
}

pub fn use_both() {
    let _: Option<i32> = left::parse("1");
    let _: Option<u64> = right::parse("2");
}
