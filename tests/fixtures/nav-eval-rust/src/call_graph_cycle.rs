//! Cycle a -> b -> c -> a. BFS callees from `a` at depth 5 must terminate
//! and deduplicate.

#![allow(dead_code)]

pub fn a() { b() }
pub fn b() { c() }
pub fn c() {
    if false { a() }
}
