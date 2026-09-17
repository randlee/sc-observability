"""Public generated records remain typed on the minimum supported Python."""
from sc_observability.generated import (
    InputLogQuery,
    OutputFailureValidation,
    OutputLevelState,
    OutputRemediationRecoverable,
)

failure = OutputFailureValidation(
    at="1970-01-01T00:00:00Z",
    code="EXAMPLE",
    field="value",
    message="invalid",
    remediation=OutputRemediationRecoverable(steps=("fix",)),
)
assert failure.kind == "validation"
query = InputLogQuery(schema_version=1)
state = OutputLevelState(configured_level="info", effective_level="debug", level_revision=2**64 - 1)
revision: int = state.level_revision
