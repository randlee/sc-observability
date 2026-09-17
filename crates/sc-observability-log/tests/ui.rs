//! trybuild compile-fail cases: one per row of the rejected-forms tables in
//! `docs/compatibility.md` (event macros and `#[instrument]`), plus the
//! `Entered`-across-`.await` `!Send` case, each with its expected stderr checked in.
//! Does not install the logger.

#[test]
fn rejected_event_forms_fail_to_compile() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
