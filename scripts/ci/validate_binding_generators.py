#!/usr/bin/env python3
"""Check frozen schema conformance, complete generator outputs and determinism."""
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
from _binding_schema import inspect_schema,validate

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    schema=json.loads((ROOT/'bindings/schema/v1.json').read_text());inspect_schema(schema)
    cases=json.loads((ROOT/'bindings/conformance/v1/schema-cases.json').read_text())
    for case in cases:
        try:validate(schema,schema['x-sc-entrypoints'][case['entrypoint']],case['value']);actual=True
        except ValueError:actual=False
        if actual!=case['valid']:raise AssertionError(f'schema fixture mismatch: {case["id"]}')
    generated=ROOT/'bindings/python/sc-observability-py/python/sc_observability/generated/__init__.py'
    spec=importlib.util.spec_from_file_location('binding_generated',generated);module=importlib.util.module_from_spec(spec);sys.modules[spec.name]=module;spec.loader.exec_module(module)
    for case in cases:
        try:module.validate_wire(case['entrypoint'],case['value']);actual=True
        except ValueError:actual=False
        if actual!=case['valid']:raise AssertionError(f'generated Python fixture mismatch: {case["id"]}')
        if actual:module.from_wire(case['entrypoint'],case['value'])
    # Wire decimal data becomes exact Python ints in frozen records.
    result=module.from_wire('OutputLevelStateDto',{'configured_level':'info','effective_level':'trace','level_revision':str(2**64-1)})
    assert result.level_revision==2**64-1 and type(result.level_revision) is int
    try:result.level_revision=0
    except AttributeError:pass
    else:raise AssertionError('generated value is mutable')
    node_test='''import {readFileSync} from 'node:fs';import {validate} from './bindings/typescript/src/generated/index.ts';const cases=JSON.parse(readFileSync('bindings/conformance/v1/schema-cases.json','utf8'));for(const c of cases){if(validate(c.entrypoint,c.value)!==c.valid)throw new Error(c.id)}console.log(`TS_SCHEMA_CASES_PASSED: ${cases.length}`);'''
    subprocess.run(['node','--experimental-strip-types','--input-type=module','-e',node_test],cwd=ROOT,check=True)
    tools=[('generate_typescript_bindings.py','bindings/typescript/src/generated'),('generate_python_bindings.py','bindings/python/sc-observability-py/python/sc_observability/generated')]
    with tempfile.TemporaryDirectory(prefix='binding-generator-check-') as temp:
        temporary=Path(temp)
        schema_command=['cargo','run','--locked','--manifest-path',str(ROOT/'bindings/schema-generator/Cargo.toml'),'--bin','sc-observability-schema','--']
        for iteration in ['schema-a','schema-b']:
            destination=temporary/iteration
            subprocess.run(schema_command+['--output',str(destination/'v1.json'),'--errors-output',str(destination/'errors-v1.json')],cwd=ROOT,check=True,capture_output=True)
        for filename in ['v1.json','errors-v1.json']:
            assert (temporary/'schema-a'/filename).read_bytes()==(temporary/'schema-b'/filename).read_bytes()==(ROOT/'bindings/schema'/filename).read_bytes(),'nondeterministic canonical schema'
        target=temporary/'schema-a/v1.json';target.write_bytes(target.read_bytes()+b' ');before=target.read_bytes()
        rejected=subprocess.run(schema_command+['--output',str(target),'--errors-output',str(temporary/'schema-a/errors-v1.json'),'--check'],cwd=ROOT,capture_output=True)
        assert rejected.returncode!=0 and target.read_bytes()==before,'schema check mode overwrote drift'
        for tool,output in tools:
            command=[sys.executable,str(ROOT/'scripts'/tool),'--schema',str(ROOT/'bindings/schema/v1.json')]
            subprocess.run(command+['--output-dir',str(ROOT/output),'--check'],check=True)
            for iteration in ['a','b']:subprocess.run(command+['--output-dir',str(temporary/tool/iteration)],check=True)
            a,b=temporary/tool/'a',temporary/tool/'b'
            first={p.relative_to(a).as_posix():digest(p) for p in a.rglob('*') if p.is_file()};second={p.relative_to(b).as_posix():digest(p) for p in b.rglob('*') if p.is_file()}
            assert first==second,'nondeterministic generator output'
            # Check mode must reject drift and leave the changed bytes untouched.
            target=next(p for p in a.rglob('*') if p.is_file() and p.suffix in ('.py','.ts'))
            target.write_bytes(target.read_bytes()+b'\n# drift\n');before=target.read_bytes()
            rejected=subprocess.run(command+['--output-dir',str(a),'--check'],capture_output=True)
            assert rejected.returncode!=0 and target.read_bytes()==before,'check mode overwrote drift'
        unsupported=json.loads(json.dumps(schema));unsupported['$defs'][next(iter(unsupported['$defs']))]['unevaluatedProperties']=False
        path=temporary/'unsupported.json';path.write_text(json.dumps(unsupported))
        for tool,_ in tools:
            result=subprocess.run([sys.executable,str(ROOT/'scripts'/tool),'--schema',str(path),'--output-dir',str(temporary/'unsupported-output')],capture_output=True,text=True)
            assert result.returncode and 'unsupported schema keywords' in result.stderr
    print(f'GENERATOR_CONFORMANCE_PASSED: {len(cases)} frozen cases; two clean runs; drift and unsupported-keyword negatives')
if __name__=='__main__':main()
