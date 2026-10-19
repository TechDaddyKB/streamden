# ADR-0004: Product name, identifiers and repository hosting

- **Status:** Accepted (owner, 2026-10-18; Q-B1, Q-B4)
- **Research:** [branding-and-distribution.md](../research/branding-and-distribution.md)

## Decision

| Thing | Value |
|---|---|
| Product name | **StreamDen** |
| Publisher | **Tech Daddy Digital** (legally registered; GitHub org `tech-daddy-digital`) |
| App identifier (Tauri `identifier`, Flatpak app-id later) | `com.techdaddydigital.streamden` |
| Windows publisher / winget id | `Tech Daddy Digital` / `TechDaddyDigital.StreamDen` |
| Binaries | `streamden` (desktop), `streamdend` (headless daemon), `streamden-mcp` (MCP stdio shim) |
| Plugin developer CLI | `den` (scaffold: `create-streamden-plugin`) |
| Internal crate prefix | `den-` (`den-core`, `den-protocol`, …), `publish = false` |
| Public SDK packages | `streamden-sdk` (crates.io, npm `@streamden/sdk`, PyPI `streamden`), names to be reserved in P0 |
| AUR | `streamden-bin`, later `streamden`, `streamden-git` |
| Repo | `TechDaddyKB/streamden` during development → **transferred** to `tech-daddy-digital/streamden` at release |

## Repo move plan (dev account → org)
GitHub keeps redirects after a transfer, but redirects shouldn't be relied on for things that ship inside
binaries. So, **from day one**:
- updater endpoint, `homepage`, `repository`, the AUR `url`, docs links and the issue-tracker URL in packaging
  metadata all point at `github.com/tech-daddy-digital/streamden` (or a project domain later), never at the
  personal account;
- **no binary releases** are published from the personal repo. The first public release happens after the
  transfer;
- CI secrets and branch protection are re-created on the org repo during the transfer (a checklist item in
  `docs/owner/registrations/distribution-and-signing.md`).

## Why not "Streamer.live"
The name was already used by an open-source project (`getstreamer.live`), the domain redirects to Switchboard
Live, and the confusion risk with Streamer.bot was medium–high.

## Open items
- A domain (`techdaddydigital.com` currently has broken HTTPS; Flathub verification needs a working site). Not
  blocking until P6.
- Reserve package names (crates.io, npm scope, PyPI) in P0.
