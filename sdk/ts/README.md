# @streamden/sdk

Private source package for StreamDen's generated protocol types. Workspace consumers can import types
and guards from `@streamden/sdk`; exports point at `src/index.ts`, following `@streamden/kit`.
No build step is required. Regenerate `src/gen/` only with `cargo xtask codegen`.

## Extensible unions

`Node`, `SubAction` and `Fragment` include receiving-only fallbacks with `kind: string`. Comparing `kind`
alone cannot narrow away those branches. Classify decoded values with the guards before switching:

```ts
import { isKnownNode, type SubAction } from "@streamden/sdk";

function renderNode(node: SubAction): string {
  if (!isKnownNode(node)) {
    // Render a read-only placeholder; retain the original node unchanged when saving.
    return `Unsupported node: ${node.kind}`;
  }
  switch (node.kind) {
    case "delay":
      return `Delay: ${node.ms}`;
    default:
      // Other known kinds are handled by the renderer's remaining cases.
      return node.kind;
  }
}
```

`isKnownFragment` narrows fragments in the same way as `isKnownNode`. Use `!isKnownNode` or
`!isKnownFragment` to narrow the unknown arm; sub-actions retain their `id`.
`isUnknownNode` and `isUnknownFragment` return booleans for runtime classification. They cannot be
sound unknown type predicates: known variants are structurally assignable to the fallback, which
would make the known `else` branch `never`. After a runtime unknown check, use the corresponding
known guard before reading variant-specific fields.
Unknown fragments render their plain `text` as untrusted text. These guards classify kinds of already
decoded protocol values; they do not validate arbitrary JSON. Their kind lists are derived from
`schema/protocol.json` (`types.UnknownNode` / `types.UnknownFragment`, `properties.kind.not.enum`), and
tests check every canonical kind. They preserve the original object and its future fields.

For `RpcResponse`, use `if ("error" in response)` to select the error branch; otherwise read
`response.result`, which may be `null`.

## Checks

```sh
pnpm -C sdk/ts typecheck
pnpm -C sdk/ts test
cargo xtask codegen --check
# Run the runtime guard test independently:
node --test --test-name-pattern guards sdk/ts/tests/conformance.test.mjs
```

The Node test runner discovers every JSON fixture under `crates/den-protocol/tests/fixtures/`, embeds
literal values in temporary TypeScript with `satisfies`, and compiles against the package exports.
Known node/fragment fixtures use the narrowed union so the unknown fallback cannot conceal missing
required fields. New directories and individually mapped fixtures fail until assigned a type.
Manifest literals allow the protocol's `x-*` extension keys; the v0 generated manifest intentionally
omits them. Seven invalid fixtures must fail compilation. Eight schema-only constraints (namespace
rules, reserved slugs, nonempty arrays, numeric bounds and the response union's exactly-one-member
rule) remain the Rust/schema suite's responsibility. TypeScript structural types cannot replace runtime validation.

Temporary compiler inputs and output live under this package's ignored `node_modules/` and are removed
at the end of the test run. No generated source is modified. Each test compiles its own inputs into a
separate output directory, so the guard test runs independently of fixture compilation. Tests run on
Node 22.12+ without relying on Node's later TypeScript stripping support.

## Client and licensing status

Catalog-generated client methods, `client.call`, dry-run overloads, `DraftResult` return unions and
incoming-result validation are deferred from DEN-39. This package exports the generated contract types
and narrowing helpers; it provides no transport or callable client yet.

The private package is GPL-3.0-or-later for now, as required by DEN-39. Generated SDK artifacts follow
D-016 / Q-B6's MIT OR Apache-2.0 license policy. TypeScript is pinned to the workspace's 7.0.2
(Apache-2.0). Tests use Node's built-in `node:test` (Node.js: MIT), adding no test-runner dependency.
