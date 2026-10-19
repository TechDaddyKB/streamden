# Tasks → cards on the codex-orchestrator board

Work items are **cards** on the codex-orchestrator board, project **`streamden`** (`http://localhost:8488`),
per [ADR-0005](../docs/adr/0005-work-management-and-agent-tooling.md). Every agent reaches them through the
orchestrator MCP. Lanes, labels and the claim/release protocol: [`docs/agents/board.md`](../docs/agents/board.md).

This folder keeps [`_TEMPLATE.md`](_TEMPLATE.md): the **card detail template** and the **handoff note template**.

## Protocol in one screen
1. **Lanes:** Backlog → Planning (plan) / Coding (code) → Review (review) → Verifying (owner) → Done. Plus Blocked.
2. **The owner dispatches** by setting the `agent:<vendor>` and `reviewer:<vendor>` labels and launching the
   agent (`clx`/`cdx`/`cgx`). The agent claims the card with its `worktree_path` and `username` and works on
   branch `agent/<vendor>/DEN-<n>-<slug>`.
3. **Scope (paths) is a write lease.** Two claimed cards must never have overlapping paths.
4. **Done when = commands.** Their output goes in the handoff note on `project_card_release`.
5. **The reviewer's vendor ≠ the author's vendor.** Only the owner moves cards to Done.
6. **Sizing:** one agent session, about 400 changed lines or fewer. Split anything bigger.
7. **Dependencies:** the card's `depends_on`. Cards with `ready: false` wait.
