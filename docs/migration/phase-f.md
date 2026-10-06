# Phase F migration notes

## OTLP span assembly types

`sc_observability_otlp::CompleteSpan` and
`sc_observability_otlp::SpanAssembler` have no public v2 equivalents. Span
assembly is internal to the `Telemetry` implementation; applications that used
the released v1 assembly types have no drop-in public replacement for them.
