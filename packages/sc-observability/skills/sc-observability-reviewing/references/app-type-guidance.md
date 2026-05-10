# App Type Guidance

## CLI

Expect:

- sparse informational logging
- one success event per successful command
- errors and warnings always emitted
- optional stderr console output for local/operator visibility

## Long-Running Service

Expect:

- startup and shutdown events
- file sink enabled by default
- console sink off by default unless local development needs it
- clear active log path verification

## Hybrid Tool

Expect:

- explicit choice of service-like or CLI-like lifecycle behavior
- avoid mixing verbose console output with quiet file logging by accident
- keep target/action naming consistent across both modes
