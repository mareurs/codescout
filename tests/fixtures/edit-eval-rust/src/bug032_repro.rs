pub enum E {
    A,
    B,
}

impl E {
    pub fn name(&self) -> &'static str {
        match self {
            E::A => "A",
            E::B => "B",
        }
    }
}

impl std::fmt::Display for E {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

pub fn marker() -> u32 {
    7
}
