//! Top-level `fn add` plus a `fn add` inside a `#[cfg(test)] mod tests`
//! helper. Default search scope must include the top-level fn; whether it
//! includes the test-module helper depends on tool semantics — we encode
//! the current expected behavior and let the report reveal drift.

pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    fn add(_x: i32) -> i32 { 0 }

    #[test]
    fn smoke() {
        let _ = add(1);
        assert_eq!(super::add(1, 2), 3);
    }
}
