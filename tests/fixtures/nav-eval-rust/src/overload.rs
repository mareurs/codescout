//! Three structs each define `fn new` with different signatures.
//! Trap: which `new` does a name-only search resolve to?

pub struct Foo;
pub struct Bar;
pub struct Baz;

impl Foo {
    pub fn new() -> Foo { Foo }
}

impl Bar {
    pub fn new(_label: &str) -> Bar { Bar }
}

impl Baz {
    pub fn new(_n: usize, _flag: bool) -> Baz { Baz }
}
