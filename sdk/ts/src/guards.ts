import type { Fragment, Node, SubAction } from "./gen";

// Derived from schema/protocol.json: types.Unknown{Node,Fragment}.properties.kind.not.enum.
// The conformance test checks these lists against the canonical artifact on every run.
const nodeKinds: ReadonlySet<string> = new Set<KnownNode["kind"]>([
  "op", "group", "if", "switch", "while", "for_each", "break_loop", "continue_loop", "stop",
  "parallel", "delay", "run_action", "code", "placeholder",
]);
const fragmentKinds: ReadonlySet<string> = new Set<KnownFragment["kind"]>(["text", "emote", "mention", "link", "cheermote"]);

// Exclude<T, UnknownNode> would also remove known variants assignable to its index signature.
// Instead distribute over the union and discard the branch with an unrestricted string tag.
type Known<T extends { kind: string }> = T extends unknown
  ? string extends T["kind"] ? never : T
  : never;
export type KnownNode = Known<Node>;
export type KnownSubAction = Known<SubAction>;
export type KnownFragment = Known<Fragment>;

/** Classify an already decoded node; this does not validate arbitrary JSON. */
export function isKnownNode(node: SubAction): node is KnownSubAction;
export function isKnownNode(node: Node): node is KnownNode;
export function isKnownNode(node: Node): boolean {
  return nodeKinds.has(node.kind);
}

/**
 * Runtime kind test only: known variants are also assignable to UnknownNode, so an
 * unknown type predicate would erase the known else branch. Narrow with !isKnownNode.
 * Unknown nodes must remain read-only and retain their original members when saved.
 */
export function isUnknownNode(node: Node): boolean {
  return !nodeKinds.has(node.kind);
}

/** Classify an already decoded fragment; this does not validate arbitrary JSON. */
export function isKnownFragment(fragment: Fragment): fragment is KnownFragment {
  return fragmentKinds.has(fragment.kind);
}

/** Runtime kind test only. Narrow with !isKnownFragment; render unknown text as untrusted. */
export function isUnknownFragment(fragment: Fragment): boolean {
  return !fragmentKinds.has(fragment.kind);
}
