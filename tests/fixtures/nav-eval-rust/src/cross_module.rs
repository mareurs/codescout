//! Two modules each define `validate`. `a::validate` is called from
//! `use_a`; `b::validate` is dead. References for `a::validate` must NOT
//! include `b::validate`'s definition site.

pub mod a {
    pub fn validate(s: &str) -> bool { !s.is_empty() }
}

pub mod b {
    pub fn validate(s: &str) -> bool { !s.is_empty() }
}

pub fn use_a() {
    let _ = a::validate("hi");
}
