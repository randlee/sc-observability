"""Shared strict schema compiler support. Inputs are canonical JSON only."""
from __future__ import annotations
import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

KEYWORDS = {'$schema','$id','$defs','$ref','title','description','type','properties','required','additionalProperties','items','enum','const','oneOf','anyOf','allOf','default','format','minimum','maximum','pattern','minLength','maxLength','minItems','maxItems','x-sc-integer-domain'}

def inspect_schema(schema):
    def inspect(node):
        if isinstance(node,bool): return
        if not isinstance(node,dict): raise ValueError('schema node must be object or boolean')
        unknown=set(node)-KEYWORDS
        if unknown: raise ValueError(f'unsupported schema keywords: {sorted(unknown)}')
        if '$ref' in node:
            ref=node['$ref']
            if not ref.startswith('#/$defs/') or ref[8:] not in schema['$defs']: raise ValueError(f'unresolved local reference: {ref}')
        for key in ('properties','$defs'):
            for child in node.get(key,{}).values():inspect(child)
        for key in ('items','additionalProperties'):
            if key in node:inspect(node[key])
        for key in ('oneOf','anyOf','allOf'):
            for child in node.get(key,[]):inspect(child)
    for node in schema['$defs'].values():inspect(node)
    for node in schema['x-sc-entrypoints'].values():inspect(node)
    codes=[entry['code'] for entry in schema['x-sc-error-registry']]
    if len(codes)!=len(set(codes)):raise ValueError('duplicate error registry code')

def run(generate):
    parser=argparse.ArgumentParser()
    parser.add_argument('--schema',required=True,type=Path)
    parser.add_argument('--output-dir',required=True,type=Path)
    parser.add_argument('--check',action='store_true')
    args=parser.parse_args()
    if sys.version_info[:3] != (3,12,10):raise SystemExit('generation requires Python 3.12.10')
    schema=json.loads(args.schema.read_text(encoding='utf-8'));inspect_schema(schema)
    files=generate(schema)
    for name,content in files.items():
        path=args.output_dir/name
        data=content.encode('utf-8')
        if args.check:
            if not path.is_file() or path.read_bytes()!=data:raise SystemExit(f'generated drift: {path}')
        else:
            path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)

def name_of(ref):return ref.rsplit('/',1)[-1]
def ts_type(node):
    if node is True:return 'unknown'
    if node is False:return 'never'
    if '$ref' in node:return name_of(node['$ref'])
    if 'const' in node:return json.dumps(node['const'])
    if 'enum' in node:return ' | '.join(json.dumps(v) for v in node['enum'])
    for key,join in [('oneOf',' | '),('anyOf',' | '),('allOf',' & ')]:
        if key in node:return '('+join.join(ts_type(v) for v in node[key])+')'
    typ=node.get('type')
    if isinstance(typ,list):return '('+' | '.join(ts_type({**node,'type':v}) for v in typ)+')'
    if typ=='integer' and 'minimum' in node and node['minimum']==node.get('maximum'):return str(node['minimum'])
    if typ=='object':
        if 'properties' not in node:return f'Record<string, {ts_type(node.get("additionalProperties",True))}>'
        required=node.get('required',[])
        return '{ '+ ' '.join(f'{json.dumps(k)}'+('' if k in required else '?')+': '+ts_type(v)+';' for k,v in node['properties'].items())+' }'
    if typ=='array':return f'Array<{ts_type(node["items"])}>'
    return {'string':'string','integer':'number','number':'number','boolean':'boolean','null':'null',None:'unknown'}[typ]

# This same validator is embedded in generated Python runtime and used by compiler tests.
def validate(schema, node, value, path='$'):
    import math
    if node is True:return
    if node is False:raise ValueError(f'{path}: forbidden')
    if '$ref' in node:validate(schema,schema['$defs'][name_of(node['$ref'])],value,path)
    if node.get('x-sc-integer-domain')=='unsigned' and (not isinstance(value,str) or not value.isascii() or not value.isdecimal()):raise ValueError(f'{path}: expected unsigned decimal')
    if 'const' in node and (type(value)!=type(node['const']) or value!=node['const']):raise ValueError(f'{path}: const')
    if 'enum' in node and value not in node['enum']:raise ValueError(f'{path}: enum')
    for key in ('oneOf','anyOf'):
        if key in node:
            matches=0
            for candidate in node[key]:
                try:validate(schema,candidate,value,path);matches+=1
                except ValueError:pass
            if not matches or (key=='oneOf' and matches!=1):raise ValueError(f'{path}: {key}')
    for candidate in node.get('allOf',[]):validate(schema,candidate,value,path)
    typ=node.get('type')
    if isinstance(typ,list):
        for candidate in typ:
            try:validate(schema,{**node,'type':candidate},value,path);return
            except ValueError:pass
        raise ValueError(f'{path}: type')
    if typ=='null' and value is not None:raise ValueError(f'{path}: null')
    if typ=='boolean' and type(value) is not bool:raise ValueError(f'{path}: boolean')
    if typ=='string':
        if not isinstance(value,str):raise ValueError(f'{path}: string')
        if 'pattern' in node and re.fullmatch(node['pattern'],value) is None:raise ValueError(f'{path}: pattern')
        if 'pattern' in node and '0|[1-9]' in node['pattern'] and not -(2**63)<=int(value)<2**64:raise ValueError(f'{path}: integer range')
        if not node.get('minLength',0)<=len(value)<=node.get('maxLength',float('inf')):raise ValueError(f'{path}: string length')
    if typ in ('integer','number'):
        if type(value) not in (int,float) or (type(value) is float and not math.isfinite(value)) or (typ=='integer' and type(value) is not int):raise ValueError(f'{path}: number')
        if not node.get('minimum',-float('inf'))<=value<=node.get('maximum',float('inf')):raise ValueError(f'{path}: range')
    if typ=='array':
        if not isinstance(value,list):raise ValueError(f'{path}: array')
        if not node.get('minItems',0)<=len(value)<=node.get('maxItems',float('inf')):raise ValueError(f'{path}: array length')
        for i,item in enumerate(value):validate(schema,node['items'],item,f'{path}[{i}]')
    if typ=='object':
        if not isinstance(value,dict) or any(not isinstance(k,str) for k in value):raise ValueError(f'{path}: object')
        if not set(node.get('required',[]))<=set(value):raise ValueError(f'{path}: missing field')
        for key,item in value.items():
            if key in node.get('properties',{}):validate(schema,node['properties'][key],item,f'{path}.{key}')
            else:validate(schema,node.get('additionalProperties',True),item,f'{path}.{key}')
