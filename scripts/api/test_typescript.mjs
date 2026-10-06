import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { projectApi, differences } from '../ci/public_api_typescript.mjs';

test('actual emitted declaration signature/field/constraint/reexport and runtime mutations', () => {
  const directory = mkdtempSync(join(tmpdir(), 'sc-api-'));
  try {
    const path = join(directory, 'index.d.ts');
    const baseline = 'export interface Value<T extends string> { field: number; read(value: number): T; }\nexport { Value as Alias };\n';
    const rows = text => { writeFileSync(path, text); return projectApi(path, {}); };
    const expected = rows(baseline);
    assert.deepEqual(expected, rows(baseline));
    for (const changed of [baseline.replace('value: number', 'value: string'), baseline.replace('field: number', 'field: string'), baseline.replace('extends string', 'extends number'), baseline.replace('export { Value as Alias };', '')]) {
      assert.ok(differences(expected, rows(changed)).length);
    }
    writeFileSync(path, baseline);
    assert.ok(differences(expected, projectApi(path, { factory() {} })).length);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});
