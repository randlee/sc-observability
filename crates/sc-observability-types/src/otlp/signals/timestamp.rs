//! Canonical neutral JSON timestamps always use UTC and nine fractional digits.
use crate::Timestamp;
use serde::{Serialize, Serializer};
fn render(value: Timestamp) -> String {
    let time = value.into_inner();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        time.year(),
        u8::from(time.month()),
        time.day(),
        time.hour(),
        time.minute(),
        time.second(),
        time.nanosecond()
    )
}
pub(crate) fn serialize<S: Serializer>(
    value: &Timestamp,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&render(*value))
}
#[allow(
    clippy::ref_option,
    reason = "serde serialize_with passes a reference to the field"
)]
pub(crate) fn optional<S: Serializer>(
    value: &Option<Timestamp>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    value.map(render).serialize(serializer)
}
