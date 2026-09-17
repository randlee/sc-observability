import { createClient, createTauriTransport, encodeEvent, encodeValue } from '@sc-observability/client';

// This fixture runs inside the real OS webview. No mockIPC/test runtime is used.
const invoke = window.__TAURI__.core.invoke;
const records = [];
const uncaught = [];
window.addEventListener('unhandledrejection', (event) => uncaught.push(String(event.reason)));
window.addEventListener('error', (event) => uncaught.push(event.message));
const command = (operation, request) => invoke(`plugin:sc-observability|sc_observability_${operation}`, { request });
const request = { schema_version: 1 };
function check(name, condition, actual) {
  records.push({ name, passed: Boolean(condition), actual });
  if (!condition) throw new Error(`qualification failed: ${name}: ${JSON.stringify(actual)}`);
}
function value(name, result) {
  check(name, result?.kind === 'ok', result);
  return result.value;
}
function failure(name, result, kind) {
  check(name, result?.kind === 'error' && result.error?.kind === kind, result);
}
const wireEvent = (overrides = {}) => ({ schema_version: 1, level: 'info', target: 'tauri-example', action: 'qualification', ...overrides });
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

async function run() {
  const forbidden = new URLSearchParams(location.search).has('forbidden');
  if (forbidden) {
    for (const [operation, fields] of [['try_log', { event: wireEvent() }], ['query', { query: request }], ['health', {}], ['flush', { timeout_ms: 2000 }]]) {
      failure(`forbidden-window-${operation}`, await command(operation, { ...request, ...fields }), 'permission_denied');
    }
    failure('forbidden-window-level', await invoke('app_observability_level_change', { request: { ...request, change: { kind: 'reset' } } }), 'permission_denied');
    return;
  }
  const transport = value('tauri-transport-factory', createTauriTransport(invoke));
  const client = value('factory', createClient(transport));
  const initial = value('initial-local-status', client.client_status());
  check('initial-idle', initial.in_flight === 0 && initial.last_result.value.kind === 'idle', initial);
  const fields = { max: 18446744073709551615n, min: -9223372036854775808n, zero: -0, nil: null, fraction: 1.25, nested: { secret: 'must-not-survive' } };
  const event = value('ergonomic-event', encodeEvent({ level: 'info', target: 'tauri-example', action: 'frontend', correlation_id: 'tauri-qualification', fields }));
  check('admission-accepted', value('try-log', await client.tryLog(event)).kind === 'accepted');
  const scheduled = value('log', client.log({ ...event, action: 'scheduled' }));
  check('dispatch-is-scheduled', scheduled.kind === 'scheduled', scheduled);
  while (value('status-poll', client.client_status()).in_flight) await tick();
  value('flush', await client.flush(2000));
  const snapshot = value('query', await client.query({ schema_version: 1, correlation_id: 'tauri-qualification' }));
  const frontend = snapshot.events.find((row) => row.action === 'frontend');
  const rust = snapshot.events.find((row) => row.action === 'rust-host');
  check('correlated-rust-frontend', Boolean(frontend && rust), snapshot);
  check('bigint-exact-query', frontend.fields.max.value === '18446744073709551615' && frontend.fields.min.value === '-9223372036854775808', frontend);
  check('recursive-redaction', frontend.fields.nested.value.secret.value === '[REDACTED]', frontend);
  for (const [row, language, channel] of [[frontend, 'typescript', 'tauri'], [rust, 'rust', 'native']]) {
    check(`trusted-${language}-provenance`, row.fields['sc_observability.binding.language'].value === language && row.fields['sc_observability.binding.channel'].value === channel, row);
  }
  const health = value('health', await client.health());
  check('bridge-health-coherence', health.bridge !== null && JSON.stringify(health.logging) === JSON.stringify(health.bridge.logging) && health.level_state.configured_level === health.bridge.configured_level && health.level_state.effective_level === health.bridge.effective_level && health.level_state.level_revision === health.bridge.level_revision, health);
  check('baseline-info', health.level_state.configured_level === 'info' && health.level_state.effective_level === 'info', health);
  check('admission-filtered', value('filtered', await client.tryLog({ ...event, level: 'debug' })).kind === 'filtered');
  const level = (change) => invoke('app_observability_level_change', { request: { ...request, change } });
  value('elevate-trace', await level({ kind: 'elevate', level: 'trace' }));
  check('health-trace', value('health-elevated', await client.health()).level_state.effective_level === 'trace');
  check('repeat-unchanged', value('repeat', await level({ kind: 'elevate', level: 'trace' })).kind === 'unchanged');
  value('reduce-debug', await level({ kind: 'elevate', level: 'debug' }));
  failure('below-baseline', await level({ kind: 'elevate', level: 'error' }), 'below_baseline');
  failure('off-below-baseline', await level({ kind: 'elevate', level: 'off' }), 'below_baseline');
  value('reset', await level({ kind: 'reset' }));
  check('health-reset', value('health-reset-result', await client.health()).level_state.effective_level === 'info');
  const beforeContention = value('health-before-owner-contention', await client.health()).level_state;
  await invoke('qualification_owner_gate', { held: true });
  try {
    const busy = await level({ kind: 'elevate', level: 'debug' });
    failure('owner-contention-queue-full', busy, 'queue_full');
    check('owner-contention-code', busy.error.code === 'SC_OBSERVABILITY_BINDING_DISPATCH_FULL', busy);
    check('owner-contention-state-preserved', JSON.stringify(value('health-during-owner-contention', await client.health()).level_state) === JSON.stringify(beforeContention));
  } finally {
    await invoke('qualification_owner_gate', { held: false });
  }
  failure('level-forged-source', await level({ kind: 'reset', source: 'application' }), 'validation');
  failure('level-invalid-tag', await level({ kind: 'unknown' }), 'validation');
  failure('direct-denied-target', await command('try_log', { ...request, event: wireEvent({ target: 'forbidden' }) }), 'validation');
  failure('direct-denied-query', await command('query', { ...request, query: { ...request, target: 'forbidden' } }), 'validation');
  for (const key of ['sc_observability.binding.language', 'sc_observability.binding.future', 'sc_observability::binding::language']) {
    for (const nested of [false, true]) {
      const forged = { [key]: { kind: 'string', value: 'rust' } };
      const fields = nested ? { nested: { kind: 'object', value: forged } } : forged;
      failure(`provenance-${key}-${nested}`, await command('try_log', { ...request, event: wireEvent({ fields }) }), 'validation');
    }
  }
  failure('unknown-event-field', await command('try_log', { ...request, event: wireEvent({ authority: true }) }), 'validation');
  failure('unknown-request-field', await command('health', { ...request, authority: true }), 'validation');
  const exactRequest = { ...request, event: wireEvent({ message: '' }) };
  exactRequest.event.message = 'x'.repeat(65536 - new TextEncoder().encode(JSON.stringify(exactRequest)).length);
  value('exact-64k-request', await command('try_log', exactRequest));
  failure('one-byte-oversize-request', await command('try_log', { ...exactRequest, event: { ...exactRequest.event, message: exactRequest.event.message + 'x' } }), 'validation');
  failure('unknown-wire-version', await command('health', { schema_version: 2 }), 'unsupported_version');
  failure('oversized-request', await command('try_log', { ...request, event: wireEvent({ message: 'x'.repeat(65536) }) }), 'validation');
  let deep = { kind: 'null' };
  for (let i = 0; i < 33; i++) deep = { kind: 'array', value: [deep] };
  failure('deep-request', await command('try_log', { ...request, event: wireEvent({ fields: { deep } }) }), 'validation');
  for (const timeout of [-1, 0.5, 60001, true]) failure(`timeout-${timeout}`, await command('flush', { ...request, timeout_ms: timeout }), 'validation');
  const cyclic = {}; cyclic.self = cyclic;
  failure('cyclic-value', encodeValue(cyclic), 'validation');
  failure('getter-value', encodeValue({ get value() { throw new Error('foreign getter'); } }), 'validation');
  for (const input of [NaN, Infinity, 9007199254740992, 18446744073709551616n, -9223372036854775809n]) failure(`invalid-number-${input}`, encodeValue(input), 'validation');
  await tick();
  check('no-hidden-rejection', uncaught.length === 0, uncaught);
}

run().then(() => invoke('qualification_report', { report: { passed: true, records, uncaught } }))
  .catch((error) => invoke('qualification_report', { report: { passed: false, records, uncaught, error: String(error) } }));
