//! Top-level `fn handle` plus a local closure `let handle = ...` inside
//! another fn. Name-only search must return the top-level fn; the closure
//! binding is a local, not a top-level symbol.

pub fn handle(_x: u32) -> u32 { 0 }

pub fn caller() {
    let handle = |x: u32| x + 1;
    let _ = handle(2);
}
