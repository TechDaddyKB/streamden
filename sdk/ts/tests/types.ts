import {
  isKnownFragment, isKnownNode, isUnknownFragment, isUnknownNode,
  type Fragment, type Node, type RpcResponse, type SubAction,
  type UnknownFragment, type UnknownNode,
} from "../src/index.js";

// Compiled by both typecheck and the conformance suite. No casts may hide narrowing failures.
export function narrowNode(node: Node, subAction: SubAction): void {
  if (isKnownNode(node) && node.kind === "delay") {
    const ms: number | string = node.ms;
    // @ts-expect-error known delay nodes have no code source
    node.source;
    void ms;
  }
  if (isKnownNode(subAction) && subAction.kind === "delay") {
    const id: string = subAction.id;
    const ms: number | string = subAction.ms;
    void [id, ms];
  }
  if (isUnknownNode(node)) {
    const unknown: UnknownNode = node;
    void unknown;
  }
}

export function narrowFragment(fragment: Fragment): void {
  if (isKnownFragment(fragment) && fragment.kind === "cheermote") {
    const bits: number = fragment.bits;
    // @ts-expect-error cheermotes have no URL
    fragment.url;
    void bits;
  }
  if (isUnknownFragment(fragment)) {
    const unknown: UnknownFragment = fragment;
    void unknown;
  }
}

export function narrowResponse(response: RpcResponse): void {
  if ("error" in response) {
    const code: number = response.error.code;
    void code;
  } else {
    const result = response.result;
    void result;
  }
}
