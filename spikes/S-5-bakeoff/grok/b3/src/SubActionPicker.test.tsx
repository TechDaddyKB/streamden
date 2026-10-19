import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useRef, useState } from "react";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import type { SubAction } from "./model";
import { SubActionPicker } from "./SubActionPicker";

const items: SubAction[] = [
  { id: "core.logic.delay", name: "Delay", category: ["Core", "Logic"], description: "Wait before the next sub-action." },
  { id: "core.logic.comment", name: "Comment", category: ["Core", "Logic"], description: "Leave a note in the tree." },
  { id: "core.actions.enable-action", name: "Enable Action", category: ["Core", "Actions"], description: "Turn an action on." },
  { id: "obs.scenes.set-current-scene", name: "Set Current Scene", category: ["OBS", "Scenes"], description: "Switch the program scene." },
  { id: "streamlabs.scenes.set-current-scene", name: "Set Current Scene", category: ["Streamlabs", "Scenes"], description: "Switch the Streamlabs scene." },
  { id: "twitch.chat.send-announcement", name: "Send Announcement", category: ["Twitch", "Chat"], description: "Post a highlighted chat announcement." },
];

function Harness({ initialFavourites = [] as string[] }) {
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(true);
  const [favourites, setFavourites] = useState(initialFavourites);
  const [selected, setSelected] = useState<string[]>([]);
  return (
    <>
      <button ref={triggerRef} type="button" onClick={() => setOpen(true)}>
        Add sub-action
      </button>
      <output aria-label="Selected">{selected.join("|")}</output>
      {open ? (
        <SubActionPicker
          items={items}
          favourites={favourites}
          recents={[]}
          returnFocusRef={triggerRef}
          onToggleFavourite={(id) => setFavourites((current) => (current.includes(id) ? current.filter((entry) => entry !== id) : [...current, id]))}
          onSelect={(item) => {
            setSelected((current) => [...current, item.id]);
            setOpen(false);
          }}
          onClose={() => setOpen(false)}
        />
      ) : null}
    </>
  );
}

function tree() {
  return screen.getByRole("tree", { name: "Sub-actions" });
}

describe("SubActionPicker", () => {
  it("filters with fuzzy search and selects the best match on Enter", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.type(screen.getByRole("searchbox", { name: "Search sub-actions" }), "enable");
    const match = screen.getByRole("treeitem", { name: /Enable Action, Core \/ Actions/ });
    expect(match).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByRole("treeitem", { name: /Delay, Core \/ Logic/ })).not.toBeInTheDocument();
    await user.keyboard("{Enter}");
    expect(screen.getByLabelText("Selected")).toHaveTextContent("core.actions.enable-action");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("moves with the arrow keys and does not add a row from an empty search", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const search = screen.getByRole("searchbox", { name: "Search sub-actions" });
    search.focus();
    await user.keyboard("{Enter}");
    expect(screen.getByLabelText("Selected")).toHaveTextContent("");
    await user.keyboard("{ArrowDown}");
    const itemsInTree = within(tree()).getAllByRole("treeitem");
    const selected = itemsInTree.find((row) => row.getAttribute("aria-selected") === "true");
    expect(selected).toBeTruthy();
    expect(selected).toHaveAttribute("aria-expanded");
  });

  it("type-ahead jumps to Delay and Alt+F stars it without leaving the row", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    screen.getByRole("searchbox", { name: "Search sub-actions" }).focus();
    await user.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}del");
    expect(screen.getByRole("treeitem", { name: /Delay, Core \/ Logic$/ })).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{Alt>}f{/Alt}");
    const starred = screen.getAllByRole("treeitem", { name: /Delay, Core \/ Logic, favourite/ });
    expect(starred.some((row) => row.getAttribute("aria-selected") === "true")).toBe(true);
  });

  it("keeps focus when the focused favourite is removed", async () => {
    const user = userEvent.setup();
    render(<Harness initialFavourites={["core.logic.delay", "twitch.chat.send-announcement"]} />);
    screen.getByRole("searchbox", { name: "Search sub-actions" }).focus();
    await user.keyboard("{ArrowDown}{ArrowDown}{Alt>}f{/Alt}");
    expect(screen.getByRole("treeitem", { name: /Favourites/ })).toHaveAttribute("aria-expanded", "true");
    const still = within(tree())
      .getAllByRole("treeitem")
      .find((row) => row.getAttribute("aria-selected") === "true");
    expect(still?.getAttribute("aria-label") ?? "").toMatch(/Send Announcement/);
  });

  it("returns focus to the trigger on Escape", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.keyboard("{Escape}");
    expect(screen.getByRole("button", { name: "Add sub-action" })).toHaveFocus();
  });

  it("distinguishes the two Set Current Scene actions", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.type(screen.getByRole("searchbox", { name: "Search sub-actions" }), "set current scene");
    expect(screen.getByRole("treeitem", { name: /Set Current Scene, OBS \/ Scenes/ })).toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: /Set Current Scene, Streamlabs \/ Scenes/ })).toBeInTheDocument();
  });
});

describe("App", () => {
  it("records a selection and shows it under recently used", async () => {
    const user = userEvent.setup();
    window.localStorage.clear();
    render(<App />);
    await user.click(screen.getByRole("button", { name: "Add sub-action" }));
    const search = screen.getByRole("searchbox", { name: "Search sub-actions" });
    await user.type(search, "send announcement");
    await user.keyboard("{Enter}");
    expect(screen.getByRole("list")).toHaveTextContent("Send Announcement");
    expect(screen.getByRole("button", { name: "Add sub-action" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(screen.getByRole("treeitem", { name: /Recently used/ })).toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: /Send Announcement, Twitch \/ Chat/ })).toBeInTheDocument();
  });
});
