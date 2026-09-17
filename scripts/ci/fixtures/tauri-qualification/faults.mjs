import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { createClient, createTauriTransport, encodeEvent, encodeValue, validate } from '@sc-observability/client';
const results = [];
const escaped = [];
process.on('unhandledRejection', (error) => escaped.push(String(error)));
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
async function test(name, run) {
  try { await run(); results.push({ name, passed: true }); }
  catch (error) { results.push({ name, passed: false, error: String(error) }); }
}
const ok = (result) => { assert.equal(result.kind, 'ok'); return result.value; };
const err = (result, kind) => { assert.equal(result.kind, 'error'); if (kind) assert.equal(result.error.kind, kind); return result.error; };
const event = ok(encodeEvent({ level: 'info', target: 'tauri-example', action: 'fault' }));
const diagnostic = { at: '2026-09-17T00:00:00.000Z', code: 'REMOTE', message: 'retained', remediation: { kind: 'recoverable', steps: ['first', 'second'] } };
const clientFor = (response) => ok(createClient({ request: async () => ({ kind: 'ok', value: response }) }));
const fixtureCases = JSON.parse(readFileSync('schema-cases.json', 'utf8'));
for (const fixture of fixtureCases) {
  await test(`schema-${fixture.id}`, () => assert.equal(validate(fixture.entrypoint, fixture.value), fixture.valid));
}
for (const fixture of fixtureCases.filter((row) => row.entrypoint === 'OutputFailure' && row.valid)) {
  await test(`failure-payload-${fixture.id}`, async () => {
    const client = clientFor({ schema_version: 1, kind: 'error', error: fixture.value });
    assert.deepEqual(err(await client.tryLog(event)), fixture.value);
  });
}
await test('maximum-health-revision-and-counter-roundtrip', async () => {
  const health = structuredClone(fixtureCases.find((row) => row.entrypoint === 'OutputLogHealthDto' && row.valid).value);
  const max = '18446744073709551615';
  health.level_state.level_revision = max;
  health.logging.dropped_events_total = max;
  health.logging.active_log_path = { kind: 'unrepresentable' };
  if (health.bridge) {
    health.bridge.level_revision = max;
    health.bridge.logging = structuredClone(health.logging);
  }
  const client = clientFor({ schema_version: 1, kind: 'ok', value: health });
  assert.deepEqual(ok(await client.health()), health);
});
await test('same-timestamp-snapshot-and-additive-output', async () => {
  const stored = structuredClone(fixtureCases.find((row) => row.entrypoint === 'OutputStoredEventDto' && row.valid).value);
  const snapshot = { schema_version: 1, truncated: false, events: [{ ...stored, action: 'first' }, { ...stored, action: 'second' }], future_field: true };
  const client = clientFor({ schema_version: 1, kind: 'ok', value: snapshot });
  assert.deepEqual(ok(await client.query({ schema_version: 1 })), snapshot);
});
await test('saturating-failure-counter', async () => {
  const client = clientFor({ schema_version: 1, kind: 'error', error: { kind: 'closed', ...diagnostic } });
  client.counts.closed = '18446744073709551615';
  err(await client.tryLog(event), 'closed');
  assert.equal(ok(client.client_status()).failures_by_kind.closed, '18446744073709551615');
});
await test('query-version-before-transport', async () => {
  let calls = 0;
  const client = ok(createClient({ request: async () => { calls++; return {}; } }));
  err(await client.query({ schema_version: 2 }), 'unsupported_version');
  assert.equal(calls, 0);
});
await test('additive-known-failure', async () => {
  const client = clientFor({ schema_version: 1, kind: 'error', error: { kind: 'closed', ...diagnostic, future: true } });
  const failure = err(await client.tryLog(event), 'closed');
  for (const key of ['code', 'message', 'at', 'remediation']) assert.deepEqual(failure[key], diagnostic[key]);
});
await test('unknown-remote-oversized-diagnostic', async () => {
  const client = clientFor({ schema_version: 1, kind: 'error', error: { kind: 'future', ...diagnostic, message: 'x'.repeat(5000) } });
  assert.equal(err(await client.tryLog(event), 'validation').code, 'SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE');
});
await test('unknown-remote-remediation-preserved', async () => {
  const client = clientFor({ schema_version: 1, kind: 'error', error: { kind: 'future', ...diagnostic } });
  const failure = err(await client.tryLog(event), 'unknown_remote');
  assert.deepEqual(failure.remediation, diagnostic.remediation);
  assert.equal(failure.remote_kind, 'future');
});
await test('unknown-remote-kind-overflow', async () => {
  const client = clientFor({ schema_version: 1, kind: 'error', error: { kind: 'x'.repeat(5000), ...diagnostic } });
  assert.equal(err(await client.tryLog(event), 'validation').code, 'SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE');
});
await test('response-schema-version', async () => {
  const client = clientFor({ schema_version: 2, kind: 'ok', value: { kind: 'accepted' } });
  assert.equal(err(await client.tryLog(event), 'unsupported_version').received, 2);
});
for (const method of ['query', 'tryLog', 'log', 'encodeEvent', 'encodeValue', 'createClient']) {
  await test(`revoked-proxy-${method}`, async () => {
    const { proxy, revoke } = Proxy.revocable({}, {}); revoke();
    const client = clientFor({});
    const result = method === 'encodeEvent' ? encodeEvent(proxy) : method === 'encodeValue' ? encodeValue(proxy) : method === 'createClient' ? createClient(proxy) : await client[method](proxy);
    err(result);
  });
}
await test('own-proto-key-preserved', () => {
  const encoded = ok(encodeValue(JSON.parse('{"__proto__":"sentinel"}')));
  assert.equal(Object.hasOwn(encoded.value, '__proto__'), true);
  assert.equal(encoded.value.__proto__.value, 'sentinel');
});
for (const mode of ['throw', 'reject', 'getter']) {
  for (const method of ['tryLog', 'query', 'health', 'flush', 'log']) {
    await test(`foreign-${mode}-${method}`, async () => {
      let calls = 0;
      const transport = { request() { calls++; if (mode === 'throw') throw new Error('foreign'); return Promise.reject(new Error('foreign')); } };
      const client = ok(createClient(transport));
      if (mode === 'getter') Object.defineProperty(transport, 'request', { get() { throw new Error('foreign getter'); } });
      const result = await client[method](method === 'flush' ? 2000 : method === 'query' ? { schema_version: 1 } : event);
      if (method !== 'log') err(result);
      await tick();
      const status = ok(client.client_status());
      assert.equal(status.in_flight, 0);
      assert.equal(status.last_result.kind, 'error');
      assert.ok(status.last_failure);
      const retained = JSON.stringify(status);
      assert.equal(JSON.stringify(ok(client.client_status())), retained);
      assert.ok(calls <= 1);
    });
  }
}
await test('failed-diagnostic-accounting-preserves-original', async () => {
  const client = clientFor({ schema_version: 1, kind: 'error', error: { kind: 'closed', ...diagnostic } });
  // Deliberate fault injection into emitted private counter storage.
  Object.freeze(client.counts);
  const failure = err(await client.tryLog(event), 'closed');
  assert.equal(failure.code, diagnostic.code);
  assert.equal(client.inFlight, 0);
});
await test('unavailable-status-is-result', () => {
  const client = clientFor({});
  Object.defineProperty(client, 'counts', { get() { throw new Error('accounting storage unavailable'); } });
  err(client.client_status(), 'internal');
});
await test('bounded-dispatch-delayed-failure', async () => {
  const releases = [];
  const client = ok(createClient({ request: () => new Promise((resolve) => releases.push(resolve)) }));
  for (let count = 0; count < 256; count++) ok(client.log(event));
  err(client.log(event), 'queue_full');
  assert.equal(ok(client.client_status()).in_flight, 256);
  for (const release of releases) release({ schema_version: 1, kind: 'error', error: { kind: 'closed', ...diagnostic } });
  await tick();
  const status = ok(client.client_status());
  assert.equal(status.in_flight, 0);
  assert.equal(status.failures_by_kind.closed, '256');
  assert.equal(status.failures_by_kind.queue_full, '1');
  assert.equal(Object.keys(status.failures_by_kind).length, 13);
});
// Exercise the unchanged example helper, linked to the installed npm client
// and the actual locked Tauri JavaScript API. Only foreign IPC is injected here;
// frontend.js also consumes this same helper through real desktop IPC.
globalThis.window = { __TAURI_INTERNALS__: { invoke: async () => ({ schema_version: 1, kind: 'ok', value: { kind: 'accepted' } }) } };
const { requestLevelChange } = await import('./host-client.mjs');
await tick();
const levelRequest = { kind: 'reset' };
for (const fixture of fixtureCases.filter((row) => row.entrypoint === 'OutputFailure' && row.valid)) {
  await test(`level-helper-payload-${fixture.id}`, async () => {
    window.__TAURI_INTERNALS__ = { invoke: async () => ({ schema_version: 1, kind: 'error', error: fixture.value }) };
    assert.deepEqual(err(await requestLevelChange(levelRequest)), fixture.value);
  });
}
for (const mode of ['throw', 'reject', 'getter']) {
  await test(`level-helper-foreign-${mode}`, async () => {
    window.__TAURI_INTERNALS__ = mode === 'getter'
      ? { get invoke() { throw new Error('foreign invoke getter'); } }
      : { invoke: mode === 'throw' ? () => { throw new Error('foreign invoke'); } : () => Promise.reject(new Error('foreign rejection')) };
    err(await requestLevelChange(levelRequest), 'internal');
  });
  await test(`tauri-transport-foreign-${mode}`, async () => {
    const callback = mode === 'getter' ? () => ({ get then() { throw new Error('foreign then'); } })
      : mode === 'throw' ? () => { throw new Error('foreign invoke'); } : () => Promise.reject(new Error('foreign rejection'));
    err(await ok(createTauriTransport(callback)).request('health', { schema_version: 1 }), 'internal');
  });
}
await test('tauri-transport-invalid-factory', () => err(createTauriTransport(null), 'validation'));
await test('level-helper-invalid-envelope', async () => {
  window.__TAURI_INTERNALS__ = { invoke: async () => ({ kind: 'ok', value: {} }) };
  err(await requestLevelChange(levelRequest), 'validation');
});
await test('level-helper-response-schema-version', async () => {
  window.__TAURI_INTERNALS__ = { invoke: async () => ({ schema_version: 2, kind: 'ok', value: {} }) };
  err(await requestLevelChange(levelRequest), 'unsupported_version');
});
await test('level-helper-unknown-remote', async () => {
  window.__TAURI_INTERNALS__ = { invoke: async () => ({ schema_version: 1, kind: 'error', error: { kind: 'future_remote', ...diagnostic } }) };
  const failure = err(await requestLevelChange(levelRequest), 'unknown_remote');
  assert.equal(failure.remote_kind, 'future_remote');
  assert.deepEqual(failure.remediation, diagnostic.remediation);
});
await test('level-helper-oversized-diagnostic', async () => {
  window.__TAURI_INTERNALS__ = { invoke: async () => ({ schema_version: 1, kind: 'error', error: { kind: 'closed', ...diagnostic, message: 'x'.repeat(5000) } }) };
  assert.equal(err(await requestLevelChange(levelRequest), 'validation').code, 'SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE');
});
await tick();
await test('no-unhandled-rejection' , () => assert.deepEqual(escaped, []));
writeFileSync('fault-results.json', JSON.stringify({ passed: results.every((result) => result.passed), results }, null, 2));
console.log(JSON.stringify({ cases: results.length, failures: results.filter((result) => !result.passed) }));
if (results.some((result) => !result.passed)) process.exitCode = 1;
