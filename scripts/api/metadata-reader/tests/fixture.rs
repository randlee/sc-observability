#[test]
fn mutation_fixture_is_built_by_the_normal_unit_build() {
    assert_eq!(
        sc_api_mutation_fixture::baseline::Value { field: 1 }.method(2),
        2
    );
}
