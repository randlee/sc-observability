import type { Result, Failure, ClientOutcome, ClientStatus, ValueDto } from '@sc-observability/client';
const unreachable = (value: never): never => { throw new Error(String(value)); };
export function failureTag(failure: Failure): string {
  switch (failure.kind) {
    case 'validation': return failure.field;
    case 'queue_full': case 'permission_denied': case 'closed': case 'unavailable':
    case 'io': case 'internal': return failure.code;
    case 'below_baseline': return failure.requested;
    case 'unsupported_level': return failure.requested;
    case 'timeout': case 'cancelled': return failure.operation;
    case 'unsupported_version': return String(failure.received);
    case 'unknown_remote': return failure.remote_kind;
    default: return unreachable(failure);
  }
}
export function resultTag(value: Result<ClientOutcome>): string {
  if (value.kind === 'error') return failureTag(value.error);
  switch (value.value.kind) {
    case 'idle': return 'idle';
    case 'scheduled': case 'accepted': case 'filtered': case 'completed': return value.value.operation;
    default: return unreachable(value.value);
  }
}
export function valueTag(value: ValueDto): string {
  switch (value.kind) {
    case 'null': return 'null';
    case 'integer': case 'string': return value.value;
    case 'boolean': case 'float': return String(value.value);
    case 'array': return value.value.map(valueTag).join(',');
    case 'object': return Object.keys(value.value).join(',');
    default: return unreachable(value);
  }
}
export function localStatus(status: ClientStatus): string { return resultTag(status.last_result); }
