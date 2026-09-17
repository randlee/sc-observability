#!/usr/bin/env python3
"""Generate TypeScript declarations and wire validators from canonical JSON."""
import json
from _binding_schema import run, ts_type

RUNTIME = r'''
export function validate(name: string, value: unknown): boolean {
  const root = (schema as any)["x-sc-entrypoints"][name];
  if (!root) throw new Error(`Unknown schema entrypoint: ${name}`);
  const visit = (s: any, v: any): boolean => {
    if (typeof s === "boolean") return s;
    if (s.$ref && !visit((schema as any).$defs[s.$ref.slice(8)], v)) return false;
    if (s["x-sc-integer-domain"] === "unsigned" && (typeof v !== "string" || !/^(0|[1-9][0-9]*)$/.test(v))) return false;
    if ("const" in s && v !== s.const) return false;
    if (s.enum && !s.enum.includes(v)) return false;
    if (s.oneOf && s.oneOf.filter((n: any) => visit(n,v)).length !== 1) return false;
    if (s.anyOf && !s.anyOf.some((n: any) => visit(n,v))) return false;
    if (s.allOf && !s.allOf.every((n: any) => visit(n,v))) return false;
    if (Array.isArray(s.type)) return s.type.some((t: string) => visit({...s,type:t},v));
    switch (s.type) {
      case "null": return v === null;
      case "boolean": return typeof v === "boolean";
      case "string":
        if (typeof v !== "string" || (s.pattern && !(new RegExp(s.pattern)).test(v))) return false;
        if (s.pattern && s.pattern.includes("0|[1-9]")) {const n=BigInt(v);if(n < -(1n<<63n) || n >= (1n<<64n)) return false;}
        return v.length >= (s.minLength ?? 0) && v.length <= (s.maxLength ?? Infinity);
      case "integer": case "number": return typeof v === "number" && Number.isFinite(v) && (s.type !== "integer" || Number.isInteger(v)) && v >= (s.minimum ?? -Infinity) && v <= (s.maximum ?? Infinity);
      case "array": return Array.isArray(v) && v.length >= (s.minItems ?? 0) && v.length <= (s.maxItems ?? Infinity) && v.every((n: any) => visit(s.items,n));
      case "object":
        if (v === null || typeof v !== "object" || Array.isArray(v)) return false;
        if ((s.required ?? []).some((k: string) => !Object.hasOwn(v,k))) return false;
        return Object.entries(v).every(([k,n]) => visit(s.properties?.[k] ?? s.additionalProperties ?? true,n));
      default: return true;
    }
  };
  return visit(root,value);
}
'''
def generate(schema):
    lines=['// Generated from canonical schema. Do not edit.']
    for name,node in schema['$defs'].items():lines.append(f'export type {name} = {ts_type(node)};')
    for name,node in schema['x-sc-entrypoints'].items():
        if name not in schema['$defs']:lines.append(f'export type {name} = {ts_type(node)};')
        if name.startswith('Output'):lines.append(f'export type {name[6:]} = {name};')
    for projection in schema['x-sc-bindings']['generic_projections']:
        reference=schema['x-sc-entrypoints'][projection['source']]
        node=schema['$defs'][reference['$ref'][8:]]
        rendered=ts_type(node).replace(projection['parameter_ref'],'T')
        lines.append(f'export type {projection["name"]}<T> = {rendered};')
    lines.append('export const errorRegistry = '+json.dumps(schema['x-sc-error-registry'],ensure_ascii=False,separators=(',',':'))+' as const;')
    for entry in schema['x-sc-error-registry']:lines.append(f'export const {entry["code"]} = {json.dumps(entry["code"])} as const;')
    lines.append('export const schema = '+json.dumps(schema,ensure_ascii=False,separators=(',',':'))+' as const;')
    lines.append(RUNTIME)
    return {'index.ts':'\n'.join(lines)+'\n'}
if __name__=='__main__':run(generate)
