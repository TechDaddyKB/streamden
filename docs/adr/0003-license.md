# ADR-0003: License — GPL-3.0-or-later app, permissive SDKs

- **Status:** Accepted (owner, 2026-10-18; Q-B2)
- **Research:** [tech-stack.md §12](../research/tech-stack.md), [branding-and-distribution.md](../research/branding-and-distribution.md), [mcp-and-ai-providers.md](../research/mcp-and-ai-providers.md)

## Decision
- **The application** (engine, desktop shell, UI, first-party plugins shipped in the app): **GPL-3.0-or-later**.
  The full text is in `/LICENSE`.
- **SDKs, protocol schemas, generated client libraries, plugin templates and examples** (`sdk/**`,
  `crates/den-protocol` generated outputs, `tools/create-streamden-plugin` templates): **MIT OR Apache-2.0**, so
  plugin authors can license their plugins however they like. Each of these folders gets its own `LICENSE-*` files.
- **Contributions:** external contributors sign off under the Developer Certificate of Origin (`Signed-off-by:`, checked on PRs). No CLA. Owner and fleet-agent commits follow D-034 (single subject line).

## Why
- Piper/Kokoro local voices pull in GPL-3.0 espeak-ng. GPL avoids a separate optional download.
- An OSI license qualifies for free code signing (SignPath Foundation) and OpenAI's "Sign in with ChatGPT"
  program for open-source apps.
- Plugins talk to the app over IPC/JSON-RPC, so permissive SDKs keep third-party and commercial plugins possible.

## Consequences
- Without a CLA we can't relicense later. A Steamworks SDK integration would need a GPL linking exception
  (avoided: the plan has no Steamworks SDK, see Q-B5).
- "or-later" was chosen following the FSF recommendation. The owner can switch to "GPL-3.0-only" before the
  first outside contribution if preferred.
- Clean-room rule (AGENTS.md): no code from GPL projects (Firebot, Social Stream Ninja) or proprietary tools.
