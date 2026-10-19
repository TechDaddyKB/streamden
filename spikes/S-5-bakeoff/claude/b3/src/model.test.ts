import { describe, expect, it } from "vitest";
import { buildTree, countLeaves, flattenVisible, fuzzyMatch, matchItem, type SubActionDef } from "./model";
import { SUB_ACTIONS } from "./fixture";

const item = (name: string, extra: Partial<SubActionDef> = {}): SubActionDef => ({
  id: name.toLowerCase().replace(/\s+/g, "-"),
  name,
  category: "Core",
  group: "Logic",
  description: "",
  keywords: [],
  ...extra,
});

describe("fixture", () => {
  it("has about 200 unique sub-actions", () => {
    expect(SUB_ACTIONS.length).toBeGreaterThanOrEqual(190);
    expect(SUB_ACTIONS.length).toBeLessThanOrEqual(210);
    expect(new Set(SUB_ACTIONS.map((s) => s.id)).size).toBe(SUB_ACTIONS.length);
  });
});

describe("fuzzyMatch", () => {
  it("matches subsequences and rejects missing characters", () => {
    expect(fuzzyMatch("scs", "Set Current Scene")).not.toBeNull();
    expect(fuzzyMatch("xyz", "Set Current Scene")).toBeNull();
  });

  it("is case-insensitive and highlights a contiguous substring", () => {
    expect(fuzzyMatch("SCENE", "Set Current Scene")?.indices).toEqual([12, 13, 14, 15, 16]);
  });

  it("prefers word starts for acronym-style queries", () => {
    expect(fuzzyMatch("scs", "Set Current Scene")?.indices).toEqual([0, 4, 12]);
  });

  it("falls back to a plain pass when jumping to word starts would miss", () => {
    expect(fuzzyMatch("ae", "Enable Action")).not.toBeNull();
    expect(fuzzyMatch("nb", "Run Action by Name")).not.toBeNull();
  });

  it("ranks prefix and word-start hits above scattered hits", () => {
    const prefix = fuzzyMatch("del", "Delay")!.score;
    const scattered = fuzzyMatch("del", "Disable Reward Level")!.score;
    expect(prefix).toBeGreaterThan(scattered);
  });
});

describe("matchItem", () => {
  it("requires every term to match name, keyword, group or category", () => {
    const delay = item("Delay", { keywords: ["sleep"] });
    expect(matchItem("sleep", delay)).not.toBeNull();
    expect(matchItem("core del", delay)).not.toBeNull();
    expect(matchItem("del nothing", delay)).toBeNull();
  });
});

describe("buildTree", () => {
  const items = [item("Delay"), item("Send Message", { category: "Twitch", group: "Chat" })];

  it("adds favourite and recent sections only when not searching", () => {
    const tree = buildTree({ items, query: "", favourites: ["delay"], recent: ["send-message"] });
    expect(tree.map((n) => n.id)).toEqual(["section:favourites", "section:recent", "cat:Core", "cat:Twitch"]);
    const searched = buildTree({ items, query: "del", favourites: ["delay"], recent: [] });
    expect(searched.map((n) => n.id)).toEqual(["cat:Core"]);
    expect(countLeaves(searched)).toBe(1);
  });

  it("flattens with ARIA level, position and set size", () => {
    const tree = buildTree({ items, query: "", favourites: [], recent: [] });
    const rows = flattenVisible(tree, () => true);
    expect(rows.map((r) => [r.node.id, r.level, r.posInSet, r.setSize])).toEqual([
      ["cat:Core", 1, 1, 2],
      ["grp:Core/Logic", 2, 1, 1],
      ["item:delay", 3, 1, 1],
      ["cat:Twitch", 1, 2, 2],
      ["grp:Twitch/Chat", 2, 1, 1],
      ["item:send-message", 3, 1, 1],
    ]);
    expect(flattenVisible(tree, () => false)).toHaveLength(2);
  });
});
