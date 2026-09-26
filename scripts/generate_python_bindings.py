#!/usr/bin/env python3
"""Generate Python frozen tagged values, stubs and validators from canonical JSON."""
import inspect
import json
from _binding_schema import run,name_of,validate

def pyname(name):return name.replace('Dto','').replace('_for_','')
def typ(node):
    if node is True:return 'object'
    if node is False:return 'NoReturn'
    if '$ref' in node:return pyname(name_of(node['$ref']))
    if 'const' in node:return 'Literal['+repr(node['const'])+']'
    if 'enum' in node:return 'Literal['+', '.join(repr(x) for x in node['enum'])+']'
    for key in ['oneOf','anyOf']:
        if key in node:return ' | '.join(typ(v) for v in node[key])
    t=node.get('type')
    if isinstance(t,list):return ' | '.join(typ({**node,'type':x}) for x in t)
    if t=='integer' and 'minimum' in node and node['minimum']==node.get('maximum'):return f'Literal[{node["minimum"]}]'
    if t=='array':return 'tuple['+typ(node['items'])+', ...]'
    if t=='object':return 'Mapping[str, '+typ(node.get('additionalProperties',True))+']'
    return {'string':'str','integer':'int','number':'float','boolean':'bool','null':'None',None:'object'}[t]

RUNTIME='''
def validate_wire(name: str, value: object) -> None:
    validate(SCHEMA, SCHEMA['x-sc-entrypoints'][name], value)

def _decode(node, value, name=None):
    if '$ref' in node:
        name=name_of(node['$ref']);return _decode(SCHEMA['$defs'][name],value,name)
    if name and name.endswith('DecimalDto'):return int(value)
    for key in ('oneOf','anyOf'):
        if key in node:
            for i,child in enumerate(node[key]):
                try:validate(SCHEMA,child,value)
                except ValueError:continue
                return _decode(child,value,f'{name}#{i}' if name else None)
    if value is None:return None
    if node.get('type')=='array':return tuple(_decode(node['items'],v) for v in value)
    if node.get('type')=='object':
        props=node.get('properties',{})
        result={k:_decode(props.get(k,node.get('additionalProperties',{})),v) for k,v in value.items() if not props or k in props}
        cls=CLASSES.get(name)
        if cls:
            for k,s in props.items():
                if k not in result:result[k]=_decode(s,s.get('default')) if 'default' in s else None
            return cls(**{k:v for k,v in result.items() if k!='kind' or 'const' not in props[k]})
        return MappingProxyType(result)
    return value

def from_wire(name: str, value: object) -> object:
    """Validate a wire entrypoint then create immutable ergonomic values."""
    validate_wire(name,value)
    return _decode(SCHEMA['x-sc-entrypoints'][name],value)
'''
def generate(schema):
    header=['# Generated from canonical schema. Do not edit.','from __future__ import annotations','from dataclasses import dataclass, field as dataclass_field','from typing import Literal, Mapping, NoReturn, TypeAlias','from types import MappingProxyType','import json','import re','']
    declarations=[];aliases=[];classes={}
    for name,node in schema['$defs'].items():
        pname=pyname(name)
        if name.endswith('DecimalDto'):aliases.append(f'{pname}: TypeAlias = int');continue
        choices=node.get('oneOf',[node])
        if not all(choice.get('type')=='object' and 'properties' in choice for choice in choices):
            # Annotations are postponed so recursive records share these aliases.
            aliases.append(f'{pname}: TypeAlias = {typ(node)}');continue
        variant_names=[]
        for i,choice in enumerate(choices):
            kind=choice['properties'].get('kind',{}).get('const')
            cname=pname+(''.join(p.title() for p in str(kind or i).split('_')) if 'oneOf' in node else '')
            variant_names.append(cname);classes[name+(f'#{i}' if 'oneOf' in node else '')]=cname
            declarations.extend(['@dataclass(frozen=True, kw_only=True)',f'class {cname}:'])
            for key,prop in choice['properties'].items():
                if key=='kind' and 'const' in prop:declarations.append(f'    kind: {typ(prop)} = dataclass_field(default={prop["const"]!r}, init=False)')
                else:
                    suffix=''
                    if key not in choice.get('required',[]):
                        default=prop.get('default')
                        if isinstance(default,dict):suffix=' = dataclass_field(default_factory=lambda: MappingProxyType({}))'
                        elif isinstance(default,list):suffix=' = ()'
                        else:suffix=' = '+repr(default)
                    declarations.append(f'    {key}: {typ(prop)}'+suffix)
            declarations.append('')
        if 'oneOf' in node:aliases.append(f'{pname}: TypeAlias = '+(' | '.join(variant_names)))
    for name,node in schema['x-sc-entrypoints'].items():
        if name not in schema['$defs']:aliases.append(f'{pyname(name)}: TypeAlias = {typ(node)}')
        if name.startswith('Output'):aliases.append(f'{pyname(name[6:])}: TypeAlias = {pyname(name)}')
    # One traversal above emits both data and declaration source.
    body='\n'.join(header+declarations+aliases)+'\n'
    stub=body+'\ndef validate_wire(name: str, value: object) -> None: ...\ndef from_wire(name: str, value: object) -> object: ...\n'
    runtime=body+'\nSCHEMA = json.loads('+repr(json.dumps(schema,ensure_ascii=False,separators=(',',':')))+')\n'
    runtime+='CLASSES = {'+', '.join(repr(k)+': '+v for k,v in classes.items())+'}\n'
    runtime+=inspect.getsource(name_of)+'\n'+inspect.getsource(validate)+'\n'+RUNTIME
    runtime+='\nERROR_REGISTRY = tuple(MappingProxyType(entry) for entry in SCHEMA["x-sc-error-registry"])\n'
    for entry in schema['x-sc-error-registry']:
        runtime+=f'{entry["code"]} = {entry["code"]!r}\n';stub+=f'{entry["code"]}: str\n'
    return {'__init__.py':runtime,'__init__.pyi':stub,'py.typed':''}
if __name__=='__main__':run(generate)
