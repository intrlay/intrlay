//! Compile-time annotation contracts and dependency-path regressions.

#[test]
fn annotations() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass/*.rs");
    cases.compile_fail("tests/ui/fail/*.rs");
}
