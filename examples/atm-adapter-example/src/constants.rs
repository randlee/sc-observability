pub const OTLP_TIMEOUT_MS: u64 = 3_000;
#[expect(dead_code, reason = "reserved for the staged OTLP retry policy")]
pub const OTLP_MAX_RETRIES: u32 = 3;
#[expect(dead_code, reason = "reserved for the staged OTLP retry policy")]
pub const OTLP_INITIAL_BACKOFF_MS: u64 = 250;
#[expect(dead_code, reason = "reserved for the staged OTLP retry policy")]
pub const OTLP_MAX_BACKOFF_MS: u64 = 5_000;
