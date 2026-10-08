#!/usr/bin/env node
// Read the emitted declaration graph and runtime module; never invoke tsc/build.
import { createRequire } from 'node:module';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const project = resolve(root, 'bindings/typescript');
const require = createRequire(resolve(project, 'package.json'));
const ts = require('typescript');

export function projectApi(entry, runtime) {
  const program = ts.createProgram([entry], { noEmit: true, strict: true, target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, skipLibCheck: true });
  const errors = ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error);
  if (errors.length) throw new Error(ts.formatDiagnosticsWithColorAndContext(errors, { getCanonicalFileName: x => x, getCurrentDirectory: () => project, getNewLine: () => '\n' }));
  const checker = program.getTypeChecker();
  const source = program.getSourceFile(entry);
  const module = checker.getSymbolAtLocation(source);
  if (!module) throw new Error('emitted declaration entry is not a module');
  const rows = new Set();
  const printer = ts.createPrinter({ removeComments: true, newLine: ts.NewLineKind.LineFeed });
  const visited = new Set();
  function visit(symbol, path) {
    if (symbol.flags & ts.SymbolFlags.Alias) symbol = checker.getAliasedSymbol(symbol);
    rows.add('export ' + path);
    for (const declaration of symbol.declarations ?? []) {
      if (!declaration.getSourceFile().isDeclarationFile) throw new Error('non-declaration input in public graph');
      rows.add('declaration ' + path + ' ' + printer.printNode(ts.EmitHint.Unspecified, declaration, declaration.getSourceFile()).trim());
    }
    if (visited.has(symbol)) return;
    visited.add(symbol);
    // A named type in a public signature is part of the declaration graph,
    // even when it is not itself exported at the entry point.
    for (const declaration of symbol.declarations ?? []) {
      function references(node) {
        if (ts.isIdentifier(node)) {
          const referenced = checker.getSymbolAtLocation(node);
          if (referenced && referenced !== symbol) {
            const target = referenced.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(referenced) : referenced;
            const local = (target.declarations ?? []).filter(d => !program.isSourceFileDefaultLibrary(d.getSourceFile()) && !program.isSourceFileFromExternalLibrary(d.getSourceFile()));
            if (local.some(d => ts.isTypeAliasDeclaration(d) || ts.isInterfaceDeclaration(d) || ts.isClassDeclaration(d) || ts.isEnumDeclaration(d))) visit(target, 'referenced::' + target.name);
          }
        }
        ts.forEachChild(node, references);
      }
      references(declaration);
    }
  }
  for (const symbol of checker.getExportsOfModule(module)) visit(symbol, symbol.name);
  for (const name of Object.keys(runtime).sort()) rows.add('runtime ' + name + ' ' + typeof runtime[name]);
  return [...rows].sort();
}

export function differences(expected, current) {
  return JSON.parse(execFileSync(process.env.PYTHON ?? 'python3', [resolve(root, 'scripts/api/history.py'), '--row-differences'], {
    cwd: root,
    encoding: 'utf8',
    input: JSON.stringify({ reference: expected, candidate: current }),
  }));
}

function main() {
  const started = performance.now();
  const manifest = JSON.parse(readFileSync(resolve(project, 'package.json'), 'utf8'));
  const relative = 'schema/api/typescript-sc-observability/' + manifest.version + '.json';
  const path = resolve(root, relative);
  const base = process.env.SC_API_ACCEPTED_BASE;
  const accepted = new Set(JSON.parse(execFileSync(process.env.PYTHON ?? 'python3', [resolve(root, 'scripts/api/history.py'), '--accepted-base', base ?? '', '--check-accepted-history'], {
    cwd: root,
    encoding: 'utf8',
  })));
  const rows = projectApi(resolve(project, manifest.types), require(resolve(project, manifest.main)));
  const snapshot = { format: 'typescript-declarations/v1', package: manifest.name, version: manifest.version, rows };
  if (process.argv.includes('--capture')) {
    if (accepted.has(relative)) throw new Error('accepted history is immutable; increment package version');
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, JSON.stringify(snapshot, null, 2) + '\n');
  } else {
    const expected = JSON.parse(readFileSync(path, 'utf8'));
    if (expected.format !== snapshot.format || expected.package !== snapshot.package || expected.version !== snapshot.version) throw new Error('malformed TypeScript API snapshot');
    const changes = differences(expected.rows, rows);
    if (changes.length) throw new Error('TypeScript API differs for ' + manifest.version + '\n' + changes.slice(0, 25).join('\n') + '\nIncrement the package version and capture at release-cut; retain accepted history.');
  }
  const elapsed = (performance.now() - started) / 1000;
  if (elapsed >= 60) throw new Error('TypeScript API check exceeded 60s: ' + elapsed);
  console.log(`TypeScript API ${process.argv.includes('--capture') ? 'release-cut' : 'unit check'} PASS: ${elapsed.toFixed(3)}s; builds in check: 0; ${rows.length} rows`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
