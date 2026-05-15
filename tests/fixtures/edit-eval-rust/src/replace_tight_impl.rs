pub struct Counter(u32);

impl Counter {
    pub fn a(&self) -> u32 { self.0 }
    pub fn b(&self) -> u32 { self.0 * 4 }
    pub fn c(&self) -> u32 { self.0 * 3 }
}
