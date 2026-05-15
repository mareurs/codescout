pub fn referenced() -> u32 {
    100
}

pub fn caller() -> u32 {
    referenced() + 1
}
