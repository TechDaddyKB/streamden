import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { after, test } from 'node:test';

const root = fileURLToPath(new URL('..', import.meta.url));
const fixtures = join(root, '../../crates/den-protocol/tests/fixtures');
const protocol = JSON.parse(readFileSync(join(root, '../../schema/protocol.json'), 'utf8'));
const require = createRequire(import.meta.url);
const compiler = join(dirname(require.resolve('typescript/package.json')), 'bin/tsc');
// Keep temporary sources on the same volume as imports, including on Windows. Never touch src/gen.
const temporary = mkdtempSync(join(root, 'node_modules/.conformance-'));
after(() => rmSync(temporary, { recursive: true, force: true }));
writeFileSync(join(temporary, 'package.json'), JSON.stringify({ type: 'module' }));
const read = (path) => JSON.parse(readFileSync(path, 'utf8'));
const modulePath = (path) => './' + relative(temporary, path).split('\\').join('/');

const directoryTypes = {
  action: 'Action', action_ref: 'ActionRef', arg_writes: 'ArgWrites', auth: 'AuthContext',
  code_source: 'CodeSource', event: 'Event', filter: 'Filter', fragment: 'Fragment',
  origin: 'Origin', pending_change: 'PendingChange', principal: 'Principal', queue: 'Queue',
  stored_document: 'StoredDocument', sub_action: 'SubAction', trigger: 'Trigger',
  variable: 'VariableRecord',
};
const fileTypes = {
  'manifest/valid_acme.json': 'PluginManifest',
  'results/dry_run.json': 'DryRunResult',
  'results/draft.json': 'DraftResult',
  'results/draft_action.json': 'DraftResult',
  'results/plugin_call.json': 'PluginCallResult',
  'results/plugin_call_params.json': 'PluginCallParams',
  'rpc/batch_params.json': 'BatchExecuteParams',
  'rpc/batch_result.json': 'BatchExecuteResult',
  'rpc/events_dropped.json': 'EventsDroppedNotification',
  'rpc/events_subscribe.json': 'EventsSubscribeParams',
  'rpc/events_unsubscribe.json': 'EventsUnsubscribeParams',
  'rpc/hello.json': 'Hello',
  'rpc/identify_params.json': 'SessionIdentifyParams',
  'rpc/identify_result.json': 'SessionIdentifyResult',
  'rpc/reidentify_params.json': 'SessionReidentifyParams',
  'rpc/notification_event.json': 'RpcNotification',
  'rpc/request.json': 'RpcRequest',
  'rpc/request_string_id.json': 'RpcRequest',
  'rpc/response_ok.json': 'RpcResponse',
  'rpc/response_error.json': 'RpcResponse',
  'rpc/response_parse_error.json': 'RpcResponse',
  'rpc/response_invalid_params.json': 'RpcResponse',
  'rpc/response_null_result.json': 'RpcResponse',
};
const rejected = {
  'invalid/action_null_group.json': 'Action',
  'invalid/action_unknown_key.json': 'Action',
  'invalid/dry_run_false.json': 'DryRunResult',
  'invalid/rpc_response_neither.json': 'RpcResponse',
  'invalid/sub_action_delay_without_ms.json': 'KnownSubAction',
  'manifest/invalid_unknown_key.json': 'PluginManifest',
  'manifest/invalid_wasm_runtime.json': 'PluginManifest',
};
// Constraints TS cannot express; serde/JSON Schema tests own these rejections.
const schemaOnly = {
  'invalid/rpc_response_both.json': 'union members allow extra properties structurally',
  'invalid/sub_action_op_core_with_plugin.json': 'core/plugin namespace constraint',
  'invalid/trigger_core_with_plugin.json': 'core/plugin namespace constraint',
  'invalid/trigger_plugin_without_plugin.json': 'core/plugin namespace constraint',
  'manifest/invalid_reserved_slug.json': 'reserved string values',
};

test('all protocol fixtures satisfy their generated TS types', () => {
  const statements = [`import type * as SDK from ${JSON.stringify(modulePath(join(root, 'src/index.js')))};`];
  let accepted = 0;
  let negative = 0;
  const seen = new Set();
  for (const directory of readdirSync(fixtures).sort()) {
    for (const file of readdirSync(join(fixtures, directory)).sort()) {
      assert.ok(file.endsWith('.json'), `unmapped fixture: ${directory}/${file}`);
      const path = `${directory}/${file}`;
      seen.add(path);
      const value = read(join(fixtures, path));
      if (path in schemaOnly) continue;
      let type = rejected[path] ?? fileTypes[path] ?? directoryTypes[directory];
      assert.ok(type, `fixture needs a type mapping: ${path}`);
      // Do not let the open-ended receiving fallback mask malformed known variants.
      if (directory === 'sub_action' && protocol.types.UnknownNode.properties.kind.not.enum.includes(value.kind)) {
        type = 'KnownSubAction';
      }
      if (directory === 'fragment' && protocol.types.UnknownFragment.properties.kind.not.enum.includes(value.kind)) {
        type = 'KnownFragment';
      }
      statements.push(`// ${path}`);
      if (path in rejected) {
        statements.push('// @ts-expect-error invalid fixture must be rejected');
        negative++;
      } else {
        accepted++;
      }
      // Inline literals preserve discriminants and exercise excess-property checks; JSON imports widen tags.
      const target = directory === 'manifest' && !(path in rejected)
        ? `SDK.${type} & { [key: \`x-\${string}\`]: SDK.JsonValue }`
        : `SDK.${type}`;
      statements.push(`(${JSON.stringify(value)}) satisfies (${target});`);
    }
  }
  for (const path of Object.keys({ ...fileTypes, ...rejected, ...schemaOnly })) {
    assert.ok(seen.has(path), `stale fixture mapping: ${path}`);
  }
  assert.ok(accepted > 0);
  writeFileSync(join(temporary, 'fixtures.ts'), statements.join('\n'));
  writeFileSync(join(temporary, 'tsconfig.json'), JSON.stringify({
    extends: modulePath(join(root, 'tsconfig.json')),
    compilerOptions: { rootDir: root, outDir: join(temporary, 'out'), noEmit: false },
    files: [join(temporary, 'fixtures.ts'), join(root, 'tests/types.ts'), join(root, 'src/index.ts')],
    include: [],
  }));
  const result = spawnSync(process.execPath, [compiler, '-p', join(temporary, 'tsconfig.json')], { encoding: 'utf8' });
  assert.equal(result.status, 0, `${result.error ?? ''}\n${result.stdout}\n${result.stderr}`);
  console.log(`Fixtures: ${accepted} valid, ${negative} negative, ${Object.keys(schemaOnly).length} schema-only constraints`);
});

test('guards classify every known schema kind and preserve future payloads', async () => {
  // Use compiler output so tests work on the minimum supported Node (22.12), without TS loaders.
  const guards = await import(pathToFileURL(join(temporary, 'out/src/guards.js')).href);
  for (const [type, known, unknown] of [
    ['UnknownNode', guards.isKnownNode, guards.isUnknownNode],
    ['UnknownFragment', guards.isKnownFragment, guards.isUnknownFragment],
  ]) {
    for (const kind of protocol.types[type].properties.kind.not.enum) {
      assert.equal(known({ kind }), true, `${type}: ${kind}`);
      assert.equal(unknown({ kind }), false, `${type}: ${kind}`);
    }
    for (const kind of ['future_kind', '', 'toString', '__proto__']) {
      const value = { kind, text: 'future', extra: { preserved: [1, 2] } };
      const before = JSON.stringify(value);
      assert.equal(known(value), false);
      assert.equal(unknown(value), true);
      assert.equal(JSON.stringify(value), before);
    }
  }
  for (const [directory, known, unknown] of [
    ['sub_action', guards.isKnownNode, guards.isUnknownNode],
    ['fragment', guards.isKnownFragment, guards.isUnknownFragment],
  ]) {
    for (const file of readdirSync(join(fixtures, directory))) {
      const value = read(join(fixtures, directory, file));
      assert.equal(known(value), !file.startsWith('unknown_'), file);
      assert.equal(unknown(value), file.startsWith('unknown_'), file);
    }
  }
});
