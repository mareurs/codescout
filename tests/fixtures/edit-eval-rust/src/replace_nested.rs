pub fn outer() -> i32 {
    fn inner() -> i32 {
        9
    }
    inner()
}
