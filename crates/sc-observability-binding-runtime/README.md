# sc-observability-binding-runtime

Shared native logging backends for language hosts. The core owner controls
shutdown and level mutation; cloneable handles expose event admission, query,
health and flush. Operations share bounded workers, callbacks and deadlines.

See `docs/plans/phase-b/native-binding-runtime.md` in the source repository for
the complete lifetime, resource, timeout and provenance contract.
