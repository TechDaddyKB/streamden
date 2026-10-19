import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { STORAGE_KEY, SubActionPicker, type StorageLike } from "./SubActionPicker";
import { SUB_ACTIONS } from "./fixture";

function memoryStorage(initial?: object): StorageLike & { data: Map<string, string> } {
  const data = new Map<string, string>();
  if (initial) data.set(STORAGE_KEY, JSON.stringify(initial));
  return { data, getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) };
}

function setup(opts: { storage?: ReturnType<typeof memoryStorage>; typeAheadMs?: number } = {}) {
  const onSelect = vi.fn();
  const onClose = vi.fn();
  const storage = opts.storage ?? memoryStorage();
  const user = userEvent.setup();
  render(
    <SubActionPicker items={SUB_ACTIONS} onSelect={onSelect} onClose={onClose} storage={storage} typeAheadMs={opts.typeAheadMs} />,
  );
  const search = screen.getByRole("searchbox", { name: "Search sub-actions" });
  const tree = screen.getByRole("tree", { name: "Add sub-action" });
  return { user, onSelect, onClose, storage, search, tree };
}

const item = (name: RegExp | string) => screen.getByRole("treeitem", { name });
const focusedName = () => (document.activeElement as HTMLElement).textContent;

describe("SubActionPicker — ARIA", () => {
  it("renders a labelled tree with collapsed top-level categories", () => {
    const { search, tree } = setup();
    expect(search).toHaveFocus();
    expect(search).toHaveAttribute("aria-controls", tree.id);
    const top = within(tree).getAllByRole("treeitem");
    expect(top).toHaveLength(12);
    const core = item("Core");
    expect(core).toHaveAttribute("aria-level", "1");
    expect(core).toHaveAttribute("aria-expanded", "false");
    expect(core).toHaveAttribute("aria-setsize", String(top.length));
    // Roving tabindex: exactly one row is in the tab order.
    expect(top.filter((t) => t.tabIndex === 0)).toHaveLength(1);
  });
});

describe("SubActionPicker — search", () => {
  it("filters fuzzily, expands matches and announces the count", async () => {
    const { user, search } = setup();
    await user.type(search, "scs");
    // Present under both OBS and Streamlabs Desktop; each sits below its category and group.
    const hits = screen.getAllByRole("treeitem", { name: /^Set Current Scene/ });
    expect(hits).toHaveLength(2);
    const hit = hits[0]!;
    expect(hit).toHaveAttribute("aria-level", "3");
    expect(screen.getByRole("status")).toHaveTextContent(/\d+ sub-actions? match/);
    expect(screen.queryByRole("treeitem", { name: /^Ban User/ })).toBeNull();
    // The matched characters are highlighted.
    expect(hit.querySelectorAll("mark").length).toBeGreaterThan(0);
  });

  it("matches keywords and multiple terms", async () => {
    const { user, search } = setup();
    await user.type(search, "tts");
    expect(item(/^Speak Text/)).toBeInTheDocument();
    await user.clear(search);
    await user.type(search, "obs mute");
    expect(item(/^Toggle Input Mute/)).toBeInTheDocument();
    expect(screen.queryByRole("treeitem", { name: /^Timeout User/ })).toBeNull();
  });

  it("shows an empty state", async () => {
    const { user, search } = setup();
    await user.type(search, "qqqzzz");
    expect(screen.getByRole("status")).toHaveTextContent("No sub-actions match");
    expect(screen.queryAllByRole("treeitem")).toHaveLength(0);
  });

  it("Enter in the search box adds the best match", async () => {
    const { user, search, onSelect } = setup();
    await user.type(search, "delay{Enter}");
    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: "core.delays.delay" }));
  });

  it("Escape clears the query first, then closes", async () => {
    const { user, search, onClose } = setup();
    await user.type(search, "obs");
    await user.keyboard("{Escape}");
    expect(search).toHaveValue("");
    expect(onClose).not.toHaveBeenCalled();
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});

describe("SubActionPicker — keyboard navigation", () => {
  it("moves with arrows, expands and collapses, Home and End", async () => {
    const { user } = setup();
    await user.keyboard("{ArrowDown}");
    expect(item("Core")).toHaveFocus();

    await user.keyboard("{ArrowRight}");
    expect(item("Core")).toHaveAttribute("aria-expanded", "true");
    expect(item("Core")).toHaveFocus();

    await user.keyboard("{ArrowRight}");
    expect(item("Logic")).toHaveFocus();
    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(item(/^If \/ Else/)).toHaveFocus();
    expect(item(/^If \/ Else/)).toHaveAttribute("aria-selected", "true");

    await user.keyboard("{ArrowDown}");
    expect(item(/^Switch/)).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(item("Logic")).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(item("Logic")).toHaveAttribute("aria-expanded", "false");
    await user.keyboard("{ArrowLeft}");
    expect(item("Core")).toHaveFocus();

    await user.keyboard("{End}");
    expect(item("Integrations")).toHaveFocus();
    await user.keyboard("{Home}");
    expect(item("Core")).toHaveFocus();

    await user.keyboard("{ArrowUp}");
    expect(screen.getByRole("searchbox")).toHaveFocus();
  });

  it("type-ahead jumps to the next row starting with the typed text", async () => {
    const { user } = setup({ typeAheadMs: 10_000 });
    await user.keyboard("{ArrowDown}");
    await user.keyboard("o");
    expect(item("OBS")).toHaveFocus();
    await user.keyboard("v");
    expect(item("Overlays")).toHaveFocus();
  });

  it("repeating one letter cycles through matching rows", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { user } = setup({ typeAheadMs: 300 });
      await user.keyboard("{ArrowDown}");
      await user.keyboard("t");
      expect(item("Twitch")).toHaveFocus();
      await user.keyboard("t");
      // "tt" is a repeat, so it cycles to the next "t…" row and wraps to Twitch again.
      expect(item("Twitch")).toHaveFocus();
      await user.keyboard("{Home}");
      act(() => void vi.advanceTimersByTime(400));
      await user.keyboard("v");
      expect(item("Voice")).toHaveFocus();
      await user.keyboard("v");
      expect(item("VTubing")).toHaveFocus();
    } finally {
      vi.useRealTimers();
    }
  });

  it("ArrowDown from search lands on the first match", async () => {
    const { user, search } = setup();
    await user.type(search, "replay");
    await user.keyboard("{ArrowDown}");
    expect(focusedName()).toMatch(/Replay/);
  });
});

describe("SubActionPicker — selection, favourites, recents", () => {
  it("Enter on a leaf selects it and records it as recently used", async () => {
    const { user, search, onSelect, storage } = setup();
    await user.type(search, "set title");
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onSelect).toHaveBeenCalledTimes(1);
    const picked = onSelect.mock.calls[0]![0];
    expect(picked.name).toBe("Set Title");

    await user.keyboard("{Escape}");
    const recent = item("Recently used");
    expect(recent).toHaveAttribute("aria-expanded", "true");
    expect(screen.getAllByRole("treeitem", { name: /^Set Title/ })[0]).toHaveAttribute("aria-level", "2");
    expect(JSON.parse(storage.data.get(STORAGE_KEY)!).recent).toEqual([picked.id]);
  });

  it("clicking a leaf selects it; Enter on a branch only toggles it", async () => {
    const { user, onSelect } = setup();
    await user.keyboard("{ArrowDown}{Enter}");
    expect(item("Core")).toHaveAttribute("aria-expanded", "true");
    expect(onSelect).not.toHaveBeenCalled();
    await user.click(item("Delays"));
    await user.click(item(/^Random Delay/));
    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: "core.delays.random-delay" }));
  });

  it("Alt+F toggles a favourite, which shows in the Favourites section and persists", async () => {
    const { user, search, storage } = setup();
    await user.type(search, "speak text");
    await user.keyboard("{ArrowDown}");
    expect(focusedName()).toMatch(/^Speak Text/);
    await user.keyboard("{Alt>}f{/Alt}");
    expect(item(/^Speak Text.*\(favourite\)/)).toBeInTheDocument();
    expect(JSON.parse(storage.data.get(STORAGE_KEY)!).favourites).toEqual(["voice.text-to-speech.speak-text"]);

    await user.keyboard("{Escape}");
    expect(item("Favourites")).toHaveAttribute("aria-expanded", "true");
    expect(screen.getAllByRole("treeitem", { name: /^Speak Text/ })[0]).toHaveAttribute("aria-level", "2");
  });

  it("un-favouriting inside Favourites keeps focus in the tree", async () => {
    const storage = memoryStorage({ favourites: ["core.delays.delay", "core.logic.switch"], recent: [] });
    const { user, onSelect } = setup({ storage });
    await user.keyboard("{Enter}");
    expect(onSelect).not.toHaveBeenCalled();
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(focusedName()).toMatch(/^Delay/);
    await user.keyboard("{Alt>}f{/Alt}");
    expect(focusedName()).toMatch(/^Switch/);
    await user.keyboard("{Alt>}f{/Alt}");
    expect(screen.queryByRole("treeitem", { name: "Favourites" })).toBeNull();
    expect(document.activeElement).toHaveAttribute("role", "treeitem");
  });

  it("loads favourites and recents from storage and ignores junk", () => {
    setup({ storage: memoryStorage({ favourites: ["core.delays.delay", 42], recent: ["missing.id"] }) });
    expect(item("Favourites")).toBeInTheDocument();
    expect(screen.queryByRole("treeitem", { name: "Recently used" })).toBeNull();
  });
});
