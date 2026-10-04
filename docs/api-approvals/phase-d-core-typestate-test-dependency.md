# Phase D core typestate test dependency decision

RULE-007 permits the existing workspace-pinned `trybuild` package as a
`sc-observability` development dependency for the stopped-logger compile-fail
fixtures and the running-to-stopped positive fixture. This is the same package
already used by the log crate; no runtime dependency or new package is added.
The exact core test allowlist gains `trybuild` beside `temp-env` and `tempfile`;
all runtime dependency bans remain unchanged.

Root's original task authorization is recorded in ATM message
`01M3RKTQRZNRG4H7STRETQG98B`. Team-lead adjudication
`01M3RMNFFMXANKAVW3HX900JX1` approved this bounded exception, which root
accepted on 2026-09-30 before integration. Expected compiler diagnostics are
part of the fixture contract, so a compile-fail doctest is not equivalent.

The shutdown fixture pins E0599, “no method named shutdown” on
`Logger<Stopped>`. Rust additionally describes the same-named private field;
that label does not replace the missing-method assertion. The expected stderr
remains unchanged.
