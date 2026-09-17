import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { createClient, encodeEvent, encodeValue, validate } from '@sc-observability/client';
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
await tick();
await test('no-unhandled-rejection', () => assert.deepEqual(escaped, []));
writeFileSync('fault-results.json', JSON.stringify({ passed: results.every((result) => result.passed), results }, null, 2));
console.log(JSON.stringify({ cases: results.length, failures: results.filter((result) => !result.passed) }));
if (results.some((result) => !result.passed)) process.exitCode = 1;
