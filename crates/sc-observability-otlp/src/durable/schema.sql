PRAGMA user_version = 1;
CREATE TABLE store_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE submissions (
  submission_id TEXT PRIMARY KEY,
  record_key TEXT UNIQUE,
  envelope_version INTEGER NOT NULL,
  envelope BLOB NOT NULL,              -- SubmissionEnvelope::to_canonical_json
  envelope_bytes INTEGER NOT NULL,
  admitted_at_unix_nano INTEGER NOT NULL
);
CREATE TABLE signal_deliveries (
  submission_id TEXT NOT NULL REFERENCES submissions(submission_id) ON DELETE CASCADE,
  signal TEXT NOT NULL CHECK (signal IN ('logs','traces','metrics','profiles')),
  state TEXT NOT NULL CHECK (state IN ('pending','claimed','retry','delivered','failed','evicted')),
  attempts INTEGER NOT NULL DEFAULT 0,
  claimed_by TEXT,
  claim_expires_at_unix_nano INTEGER,
  next_attempt_at_unix_nano INTEGER,
  last_error_code TEXT,
  delivered_at_unix_nano INTEGER,
  PRIMARY KEY (submission_id, signal)
);
CREATE INDEX signal_deliveries_ready ON signal_deliveries (signal, state, next_attempt_at_unix_nano);
CREATE TABLE record_keys (record_key TEXT PRIMARY KEY, submission_id TEXT NOT NULL,
  admitted_at_unix_nano INTEGER NOT NULL);
CREATE TABLE drain_lease (id INTEGER PRIMARY KEY CHECK (id = 1), holder TEXT NOT NULL,
  acquired_at_unix_nano INTEGER NOT NULL, expires_at_unix_nano INTEGER NOT NULL);
CREATE TABLE store_counters (name TEXT PRIMARY KEY, value INTEGER NOT NULL);
