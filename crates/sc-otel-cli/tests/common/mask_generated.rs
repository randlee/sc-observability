pub fn mask_generated(envelope: &mut serde_json::Value, input: &serde_json::Value) {
    for &(signal, field) in crate::d29_system_generated_fields::D29_SYSTEM_GENERATED_FIELDS {
        if let Some(records) = envelope[signal].as_array_mut() {
            for (index, record) in records.iter_mut().enumerate() {
                if input[signal][index][field].is_null() && record["record"][field].is_string() {
                    record["record"][field] = "<generated>".into();
                }
            }
        }
    }
}
