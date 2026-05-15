//! Inherent `Counter::next` AND `Iterator::next` impl on the same type.
//! Trap: call `counter.next()` and ask symbol_at which `next` it resolves to.

pub struct Counter { value: u32 }

impl Counter {
    pub fn new() -> Self { Counter { value: 0 } }
    /// Inherent method — same name as Iterator::next.
    pub fn next(&mut self) -> u32 {
        self.value += 1;
        self.value
    }
}

impl Iterator for Counter {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        let v = self.value + 100;
        self.value = v;
        Some(v)
    }
}

pub fn use_counter() {
    let mut c = Counter::new();
    let _ = c.next();
}
