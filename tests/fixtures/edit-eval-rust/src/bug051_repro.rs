// BUG-051 repro: long function with multi-line assert! blocks toward the end.
// If `editing_end_line` falls back to LSP's last-statement line under syntax
// errors (or even on clean syntax with multi-line trailing exprs), an
// `insert position=after` would land inside the function body.

pub fn long_target() -> bool {
    let v: Vec<u32> = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let s: String = v.iter().map(|n| format!("{n}")).collect();
    assert_eq!(s, String::from("12345678"));
    assert!(
        !s.is_empty(),
        "string should be non-empty: was {:?}",
        s,
    );
    assert!(
        s.contains('1'),
        "string should contain 1: was {:?}",
        s,
    );
    assert!(
        s.contains('2'),
        "string should contain 2: was {:?}",
        s,
    );
    assert!(
        s.contains('3'),
        "string should contain 3: was {:?}",
        s,
    );
    assert!(
        s.contains('4'),
        "string should contain 4: was {:?}",
        s,
    );
    assert!(
        s.contains('5'),
        "string should contain 5: was {:?}",
        s,
    );
    assert!(
        s.contains('6'),
        "string should contain 6: was {:?}",
        s,
    );
    assert!(
        s.contains('7'),
        "string should contain 7: was {:?}",
        s,
    );
    assert!(
        s.contains('8'),
        "string should contain 8: was {:?}",
        s,
    );
    assert!(
        s.len() == 8,
        "string should have length 8: was {:?}",
        s,
    );
    true
}

pub fn marker_after() {
    // A sibling function — must remain intact and not be split.
}
