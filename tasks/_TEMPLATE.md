# Templates for board cards (codex-orchestrator project `streamden`)

## A. Card detail (written by whoever creates the card; title starts with `DEN-<n> · `)

Labels: one `agent:*` (when dispatched), one `reviewer:*` (a different vendor), one `phase:P*`, one `kind:*`,
one `WS-*`.

```markdown
### Goal
<Observable outcome in 1–2 sentences.>

### Read first
<Repo paths: spec sections, research, ADRs, a similar code pattern to follow.>

### Scope (paths)
<Globs this item may modify. This is the write lease.>

### Requirements / checks
- …

### Done when
- `<command 1>`   e.g. cargo nextest run -p den-core queue
- `<command 2>`   e.g. cargo clippy -p den-core -- -D warnings
- Works on Linux (Wayland) and Windows (CI matrix green)
- Docs/spec updated if behaviour is user-visible

### Out of scope
- …
```

Dependencies go in the card's `depends_on` field (card numbers), not in the text.

## B. Handoff note (the `note` on `project_card_release` at the end of EVERY session)

```markdown
**Handoff — YYYY-MM-DD — <vendor>/<model>**
- State: done | partial | blocked
- Branch / last commit: agent/<vendor>/DEN-<n>-<slug> @ <short-sha>
- Files touched:
- Commands run + results: (paste the Done-when output)
- New dependencies (+license):
- Next steps:
- Gotchas:
- Follow-ups created (Backlog cards): DEN-…
- Open questions (also added to docs/QUESTIONS.md):
```
