# ADR-0005: Work management and agent control plane: codex-orchestrator (board + runtime)

- **Status:** Accepted (owner, 2026-10-18: "switch"). This supersedes the earlier proposal to use Plane as the board.
- **Date:** 2026-10-18

## Context
The owner runs two local tools:
- **codex-orchestrator** (`http://localhost:8488`), a control plane for the Claude Code, Codex and Grok Build
  CLIs. It provides the `clx`/`cdx`/`cgx` launch wrappers (sync auth, config, skills and fleet instructions) and
  one MCP server with a **project board**, Git Director merge arbitration, schedules, messaging, shared/project
  memory, secrets and watchdog/recovery;
- **Plane** (`http://localhost:8088`), a general kanban/PM tool, also reachable over MCP.

Two live boards would mean two sources of truth. We compared them on what agents need:

| Need | Orchestrator board | Plane (this install) |
|---|---|---|
| Claims tied to the agent session (auto-release when it ends) | ✅ `worktree_path` + `username`, 30-min TTL renewed by use | ❌ manual state; stale items need a human |
| Dependencies with readiness | ✅ `depends_on` → `ready` / `waiting_on` | ❌ relations API not available (had to use comments) |
| Role lanes (plan → code → review → verify) | ✅ built in, with WIP and role advisories | ➖ custom states only |
| Same MCP as Git Director, schedules, messaging | ✅ | ❌ separate server |
| Human PM UI (views, modules, cycles) | ➖ console board page | ✅ richer (some features unavailable via API here) |

None of the orchestrator's other features *require* its board, but the board is the agent-native option.

## Decision
| Concern | Source of truth | Notes |
|---|---|---|
| Work items: status, holder, dependencies, handoffs | **Orchestrator board**, project **`streamden`** | Conventions: [`docs/agents/board.md`](../agents/board.md). Card #n = `DEN-n` (numbering kept from the brief Plane period). Vendor is a label (`agent:*`, `reviewer:*`); handoffs are `project_card_release` notes. |
| Specs, contracts, ADRs, decisions, research, docs | **git repo** | Cards link to repo paths and never duplicate spec content. |
| Launching agents | Orchestrator wrappers `clx` / `cdx` / `cgx` | Fleet-level instructions stay generic; project rules live only in the repo's `AGENTS.md`. |
| Merge serialization on this machine | **Git Director** | `git_register` → `git_merge_request` (`client_request_id` = `DEN-<n>-<short-sha>`) → `allow` / `wait` (poll `git_merge_status`) / `deny` → `git_release`. Advisory; CI and the owner remain arbiters. GitHub PR merges by the owner don't need it. |
| Recurring jobs | Orchestrator **schedules** | e.g. the weekly Grok ecosystem brief (DEN-18). |
| Agent ↔ agent questions | Orchestrator **messaging** | Anything decided is copied into the card or repo. |
| Long unattended runs | Orchestrator **watchdog / persistent recovery** | Opt-in per run. |
| Orchestrator project memory / shared memory | Pointers and lessons learned only | Decisions live in `docs/decisions.md` and ADRs (the `cxx` wrapper injects a memory-routing block into engine memory files; for StreamDen, roster rule 6 applies). |
| Secrets | OS keychain / password manager | Never in cards, notes, orchestrator memory, or the repo. |

**Plane:** the "StreamDen / DEN" Plane project was **archived** (reversible). The owner may keep using Plane for
personal roadmapping, but agents never write to it.

**Path leases:** each card's detail has a *Scope (paths)* section. The owner checks for overlaps when dispatching.
A later helper (`scripts/den-claim`) can compare the paths of claimed cards automatically.

## Current setup (2026-10-18)
- Project `streamden` was created by the owner with the roster from `docs/agents/DELEGATION.md`. Its `about`
  block points at the repo and key docs. Board template "software": Backlog · Planning · Coding · Review ·
  Verifying · Done · Blocked.
- Cards DEN-1…DEN-19 were migrated with their dependencies; DEN-20 (cross-platform gate) was added and parked (D-035).
- Agents reach the MCP through the stdio server the wrappers install (`cxx communicator mcp <engine>`; the `clx`
  entry for Claude Code). The communicator capability is off on this host, so the adapter uses the HTTPS MCP
  endpoint, which works.
- Agents are launched with `scripts/dispatch.sh` (D-036). This host has no `cxx daemon` and no agent messaging yet,
  so sessions can't be started or messaged from the console or MCP; enabling those is optional.

## Consequences
- One MCP (the orchestrator's) covers board, merges, schedules and messaging for all three vendors.
- The orchestrator is local-only, so cloud agents (e.g. Codex cloud) can't see cards. Use the local CLIs, or paste
  the card into the prompt.
- If the orchestrator's board features change, update `docs/agents/board.md` (single place).
