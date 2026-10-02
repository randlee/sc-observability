//! Execute canonical.rs with two distinguishable `IdSource` implementations: only generated fields change.
mod common;
use sc_observability_types::{
    SpanId, Timestamp, TraceId,
    otlp::submission::{IdSource, SubmissionEnvelope},
};
use serde_json::Value;
use std::collections::BTreeSet;

struct Ids {
    digit: char,
    generated: Vec<Value>,
}
impl IdSource for Ids {
    fn trace_id(&mut self) -> TraceId {
        let value = TraceId::new(self.digit.to_string().repeat(32)).expect("trace");
        self.generated
            .push(serde_json::to_value(&value).expect("trace JSON"));
        value
    }
    fn span_id(&mut self) -> SpanId {
        let value = SpanId::new(self.digit.to_string().repeat(16)).expect("span");
        self.generated
            .push(serde_json::to_value(&value).expect("span JSON"));
        value
    }
    fn now(&mut self) -> Timestamp {
        let value: Timestamp = serde_json::from_value(serde_json::json!(format!(
            "2026-10-01T00:00:0{}Z",
            self.digit
        )))
        .expect("timestamp");
        self.generated
            .push(serde_json::to_value(value).expect("time JSON"));
        value
    }
}
fn differences(
    signal: &str,
    a: &Value,
    b: &Value,
    ids: &Ids,
    fields: &mut BTreeSet<(String, String)>,
) {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
            for (field, value) in a {
                if value != &b[field]
                    && ids.generated.iter().any(|generated| {
                        generated == value
                            || match (
                                serde_json::from_value::<Timestamp>(generated.clone()),
                                serde_json::from_value::<Timestamp>(value.clone()),
                            ) {
                                (Ok(a), Ok(b)) => a == b,
                                _ => false,
                            }
                    })
                {
                    fields.insert((signal.into(), field.clone()));
                } else {
                    differences(signal, value, &b[field], ids, fields);
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                differences(signal, a, b, ids, fields);
            }
        }
        _ => assert_eq!(a, b, "non-generated value changed"),
    }
}
#[test]
fn generated_field_allowance_equals_executed_canonical_contract() {
    let mut found = BTreeSet::new();
    for fixture in common::D29_FIXTURES {
        let root = common::golden_root().join(fixture);
        if !root.join("expected.envelope.json").is_file() {
            continue;
        }
        let input = std::fs::read_to_string(root.join("input.json")).expect("fixture");
        let mut first = Ids {
            digit: '1',
            generated: Vec::new(),
        };
        let mut second = Ids {
            digit: '2',
            generated: Vec::new(),
        };
        let a = serde_json::to_value(
            SubmissionEnvelope::from_json(&input, &mut first).expect("canonical first"),
        )
        .expect("JSON");
        let b = serde_json::to_value(
            SubmissionEnvelope::from_json(&input, &mut second).expect("canonical second"),
        )
        .expect("JSON");
        for signal in ["logs", "spans", "metrics", "profiles"] {
            differences(signal, &a[signal], &b[signal], &first, &mut found);
            if matches!(signal, "metrics" | "profiles") {
                assert_eq!(a[signal], b[signal], "no generated fields in {signal}");
            }
        }
    }
    assert_eq!(
        found,
        common::D29_SYSTEM_GENERATED_FIELDS
            .iter()
            .map(|(s, f)| (s.to_string(), f.to_string()))
            .collect()
    );
}
