use specta::{ts_export, ts_inline, Type};

#[derive(Type)]
enum SimpleEnum {
    #[specta(rename = "asdf")]
    A,
    B,
    C,
}

#[test]
fn test_empty() {
    #[derive(Type)]
    enum Empty {}

    assert_eq!(ts_inline::<Empty>(), "never");
}

#[test]
fn test_simple_enum() {
    assert_eq!(
        ts_export::<SimpleEnum>().unwrap(),
        r#"export type SimpleEnum = "asdf" | "B" | "C""#
    )
}
