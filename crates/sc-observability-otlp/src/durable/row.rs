//! Private database encodings; schema strings and nanosecond units have one owner.
use rusqlite::{
    ToSql,
    types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RowState {
    Pending,
    Claimed,
    Retry,
    Delivered,
    Failed,
    Evicted,
}
impl RowState {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Claimed => "claimed",
            Self::Retry => "retry",
            Self::Delivered => "delivered",
            Self::Failed => "failed",
            Self::Evicted => "evicted",
        }
    }
    pub(super) fn parse(value: &str) -> rusqlite::Result<Self> {
        [
            Self::Pending,
            Self::Claimed,
            Self::Retry,
            Self::Delivered,
            Self::Failed,
            Self::Evicted,
        ]
        .into_iter()
        .find(|state| state.as_str() == value)
        .ok_or(rusqlite::Error::InvalidQuery)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct UnixNanos(pub(super) i64);
impl UnixNanos {
    pub(super) fn saturating_add(self, duration: i64) -> Self {
        Self(self.0.saturating_add(duration))
    }
    pub(super) fn saturating_sub(self, duration: i64) -> Self {
        Self(self.0.saturating_sub(duration))
    }
}
impl FromSql for UnixNanos {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self)
    }
}
impl ToSql for UnixNanos {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LeaseHolder(pub(super) String);
impl FromSql for LeaseHolder {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        String::column_result(value).map(Self)
    }
}
impl ToSql for LeaseHolder {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}
impl std::fmt::Display for LeaseHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// Only trusted SQL templates enter here. Values remain bound SQL parameters;
// state spellings are supplied by the enum instead of copied string lists.
pub(super) fn sql(template: &str) -> String {
    let mut sql = template.to_owned();
    for (placeholder, state) in [
        ("{pending}", RowState::Pending),
        ("{claimed}", RowState::Claimed),
        ("{retry}", RowState::Retry),
        ("{delivered}", RowState::Delivered),
        ("{failed}", RowState::Failed),
        ("{evicted}", RowState::Evicted),
    ] {
        sql = sql.replace(placeholder, state.as_str());
    }
    sql
}
