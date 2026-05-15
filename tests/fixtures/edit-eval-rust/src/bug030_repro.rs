pub fn original() -> u32 {
    42
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_returns_42() {
        assert_eq!(original(), 42);
    }
}

pub fn after_tests_block() -> u32 {
    99
}
