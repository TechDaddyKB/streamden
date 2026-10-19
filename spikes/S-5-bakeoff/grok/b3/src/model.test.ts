import { describe, expect, it } from "vitest";
import fixture from "./fixture/subactions.json";
import {
  bestLeafId,
  buildTree,
  defaultCollapsed,
  flatten,
  fuzzyScore,
  matchItem,
  neighbourId,
  onTreeKey,
  pushTypeAhead,
  typeAhead,
  type SubAction,
} from "./model";

const items: SubAction[] = [
  { id: "core.logic.delay", name: "Delay", category: ["Core", "Logic"], description: "Wait before the next sub-action." },
  { id: "core.actions.enable-action", name: "Enable Action", category: ["Core", "Actions"], description: "Turn an action on." },
  { id: "core.actions.disable-action", name: "Disable Action", category: ["Core", "Actions"], description: "Turn an action off." },
  { id: "obs.scenes.set-current-scene", name: "Set Current Scene", category: ["OBS", "Scenes"], description: "Switch the program scene." },
  { id: "streamlabs.scenes.set-current-scene", name: "Set Current Scene", category: ["Streamlabs", "Scenes"], description: "Switch the Streamlabs scene." },
  { id: "twitch.chat.send-announcement", name: "Send Announcement", category: ["Twitch", "Chat"], description: "Post a highlighted chat announcement." },
];

function rowsFor(query = "", favourites: string[] = [], recents: string[] = [], collapsed?: string[]) {
  const tree = buildTree(items, query, favourites, recents);
  const hidden = new Set(collapsed ?? defaultCollapsed(tree));
  return { tree, rows: flatten(tree, hidden, query.trim().length > 0) };
}

describe("fuzzyScore", () => {
  it("matches a subsequence and rejects a missing character", () => {
    expect(fuzzyScore("ae", "Enable Action")).not.toBeNull();
    expect(fuzzyScore("zzz", "Enable Action")).toBeNull();
  });

  it("ranks a prefix above a later subsequence", () => {
    const prefix = fuzzyScore("set", "Set Current Scene");
    const later = fuzzyScore("set", "Reset Timer");
    expect(prefix).not.toBeNull();
    expect(later).not.toBeNull();
    expect(prefix!).toBeGreaterThan(later!);
  });
});

describe("tree", () => {
  it("hides collapsed branches until search forces them open", () => {
    const tree = buildTree(items, "", [], []);
    const collapsed = new Set(defaultCollapsed(tree));
    const closed = flatten(tree, collapsed, false).map((row) => row.id);
    expect(closed).toContain("branch-catalog-core-logic");
    expect(closed).not.toContain("leaf-catalog-core.actions.enable-action");

    const open = flatten(tree, collapsed, true).map((row) => row.id);
    expect(open).toContain("leaf-catalog-core.actions.enable-action");
  });

  it("keeps both Set Current Scene leaves addressable by category", () => {
    const { rows } = rowsFor("set current scene");
    const leaves = rows.filter((row) => row.node.kind === "leaf" && row.node.item.name === "Set Current Scene");
    expect(leaves.map((row) => (row.node.kind === "leaf" ? row.node.item.category[0] : ""))).toEqual(["OBS", "Streamlabs"]);
  });

  it("puts the best catalog match ahead of an equally scored favourite copy", () => {
    const { rows } = rowsFor("announcement", ["twitch.chat.send-announcement"]);
    expect(bestLeafId(rows, "announcement")).toBe("leaf-catalog-twitch.chat.send-announcement");
  });
});

describe("keyboard model", () => {
  it("does not activate a row when nothing is focused", () => {
    const { rows } = rowsFor();
    const result = onTreeKey(rows, [], null, "Enter", false);
    expect(result.activateItemId).toBeUndefined();
    expect(result.focusId).toBe(rows[0]?.id);
  });

  it("expands with ArrowRight and activates a leaf with Enter", () => {
    const { rows } = rowsFor();
    const core = rows.find((row) => row.id === "branch-catalog-core");
    expect(core).toBeTruthy();
    const expanded = onTreeKey(rows, ["branch-catalog-core"], core!.id, "ArrowRight", false);
    expect(expanded.collapsed).not.toContain("branch-catalog-core");

    const flat = flatten(buildTree(items, "", [], []), new Set(), false);
    const delay = flat.find((row) => row.id === "leaf-catalog-core.logic.delay");
    const chosen = onTreeKey(flat, [], delay!.id, "Enter", false);
    expect(chosen.activateItemId).toBe("core.logic.delay");
  });

  it("type-ahead walks prefixes and the buffer expires", () => {
    const flat = flatten(buildTree(items, "", [], []), new Set(), false);
    const first = typeAhead(flat, null, "d");
    expect(first).toBe("leaf-catalog-core.actions.disable-action");
    const delay = typeAhead(flat, first, "del");
    expect(delay).toBe("leaf-catalog-core.logic.delay");

    const firstPress = pushTypeAhead(null, "d", 1_000);
    const extended = pushTypeAhead(firstPress, "e", 1_200);
    expect(extended.buffer).toBe("de");
    expect(pushTypeAhead(extended, "s", 3_000).buffer).toBe("s");
  });

  it("keeps a neighbour when the focused favourite row disappears", () => {
    const before = rowsFor("", ["core.logic.delay", "twitch.chat.send-announcement"]).rows;
    const after = rowsFor("", ["twitch.chat.send-announcement"]).rows;
    expect(neighbourId(before, after, "leaf-favourites-core.logic.delay")).toBe("leaf-favourites-twitch.chat.send-announcement");
  });
});

describe("fixture", () => {
  const catalog: SubAction[] = fixture;

  it("ships about 200 unique sub-actions across the main categories", () => {
    expect(catalog.length).toBeGreaterThanOrEqual(190);
    expect(catalog.length).toBeLessThanOrEqual(210);
    expect(new Set(catalog.map((item) => item.id)).size).toBe(catalog.length);
    const tops = new Set(catalog.map((item) => item.category[0]));
    expect(tops.has("Core")).toBe(true);
    expect(tops.has("Twitch")).toBe(true);
    expect(tops.has("OBS")).toBe(true);
    expect(matchItem("ae", catalog.find((item) => item.name === "Enable Action")!)).not.toBeNull();
  });
});
