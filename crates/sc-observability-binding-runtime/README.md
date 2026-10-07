# sc-observability-binding-runtime

Shared native logging backends for language hosts. The core owner controls
shutdown and level mutation; cloneable handles expose event admission, query,
health and flush. Operations share bounded workers, callbacks and deadlines.

The 1.5.0 language bindings use canonical v2 failure projections while these
handles preserve host ownership. See the [Phase F migration
guide](../../docs/migration/phase-f.md) for the public API transition.

See `docs/plans/phase-b/native-binding-runtime.md` in the source repository for
the complete lifetime, resource, timeout and provenance contract.
