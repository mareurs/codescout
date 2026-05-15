//! Top-level `fn parse` shadowed by a local binding inside `caller`.
//! symbol_at on `parse(s)` inside `caller` must resolve to the local
//! binding, not the top-level fn.

pub fn parse(s: &str) -> usize { s.len() }

pub fn caller(s: &str) -> usize {
    let parse = |x: &str| x.len() * 2;
    parse(s)
}
