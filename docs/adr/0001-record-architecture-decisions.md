# ADR-0001: Record architecture decisions

- **Status:** Accepted
- **Date:** 2026-10-18
- **Deciders:** Owner

## Context
Several AI agents from different vendors (Anthropic, OpenAI, xAI) will work in this repository, often in
parallel and with no shared memory. Architectural choices must be discoverable, reviewable and stable.

## Decision
- Architecturally significant decisions are recorded as ADRs in `docs/adr/NNNN-kebab-title.md` (MADR-style:
  Context, Decision, Consequences, Alternatives).
- Agents may write ADRs with status **Proposed**. Only the owner sets **Accepted**, **Rejected** or
  **Superseded by ADR-XXXX**.
- Every accepted ADR gets a one-line entry in `docs/decisions.md`.
- Changes to `crates/den-protocol` (public contracts) always need an ADR or an amendment to an existing one.

## Consequences
Agents start each architectural task by reading `docs/decisions.md`. This adds a little overhead to
contract changes, which is intended.
