# Spec 01 — Build, CI, packaging and release

Status: **r1 proposed**, DEN-66, 2026-10-19. Author: Codex; Claude review and owner approval pending.
WS-01. This is a specification, not evidence that release publishing or updating is implemented.

## 1. Scope, stories and decision boundaries

Specify the path from a reviewed commit to verifiable v0.1 Linux release assets, preserving Windows
compilation and the DEN-20 runtime gate ([D-035](../decisions.md)). Capture the existing P0 workflows,
then define release metadata, signing, versioning, update boundaries and acceptance tests.

- As a streamer, I can install the correct package, verify its origin, and keep my configuration on upgrade.
- As an Arch user, I can use a reviewed AUR recipe without disabling checksum or signature verification.
- As a maintainer, I can trace every published byte to a commit, build inputs and checks.
- As a user on stream, I decide when an update restarts my app; a failed update preserves the working install.
- As a contributor, I get bounded CI feedback without access to publishing credentials.

Coverage: [FEATURE-PARITY §23](../FEATURE-PARITY.md#23-app-ux--ops), rows 23.01–23.06 (distribution; 23.05 partial: packaging/signing, not macOS permissions),
23.15 (pre-update backup), 23.17 (channels/verification/recovery), 23.18 (headless packaging).
Portable Windows mode and backup storage implementation belong to their own specs; this specifies their
release interfaces only. Flatpak, winget, itch.io automation, Steam, hosted apt/rpm repositories, ARM
artifacts, plugin registry signing and automatic daemon installation are non-goals for this card.

Accepted constraints: [ADR-0003](../adr/0003-license.md),
[ADR-0004](../adr/0004-naming-identifiers-hosting.md), D-024 (macOS best effort), D-035, D-040
(dependency-age policy), D-042 (native package name `streamden`). Remaining product choices are
[Q-B8–Q-B10 and Q-P4](../QUESTIONS.md); recommendations below are not owner decisions.
No public contract or ADR is changed here. Public updater types require a dedicated contract card and
ADR before implementation, following [Spec 02](02-protocol.md).

## 2. Verified baseline

Observed at `fe17750` on 2026-10-19. Source beats older examples in PLAN and research.

| Surface | Implemented behavior | Limit |
|---|---|---|
| [Docs CI](../../.github/workflows/docs.yml) | All tracked Markdown, including hidden directories/module rules; markdownlint-cli2 0.18.1 (markdownlint 0.38.0); SHA-256-verified lychee 0.24.2 | Offline relative files and heading fragments, not external URL health |
| Docs commit job | PR head branch validation and commitlint over every PR commit | Local hooks must be installed explicitly |
| [Workspace CI](../../.github/workflows/workspace.yml) | Ubuntu 24.04 and Windows 2025: frozen pnpm install, typecheck/build, pinned Rust, locked all-target workspace build | fmt/clippy/workspace tests on Linux only |
| Protocol CI (same workflow) | Both OSes: codegen drift, locked protocol tests including doctests, tracked-file diff check | Separate from UI/WebKit dependencies |
| [Packaging CI](../../.github/workflows/release-packaging.yml) | Manual, relevant-path PRs and `v*` tags; deb/rpm/AppImage and MSI/NSIS; artifacts retained 14 days | Unsigned `0.0.0`; no GitHub Release or updater payload |
| Install smoke | Digest-pinned Debian/Fedora/Arch containers; actual installation, files/icons/desktop name, ldd, three probe controls, 15-second desktop PID survival | Xvfb software rendering; no rendered-page assertion, Wayland or Windows runtime proof |
| [Hooks](../../lefthook.yml) | Branch validator and D-037 commit policy; installed tooling, no auto-install | Hooks shared between worktrees; do not reinstall casually |
| [Dispatch](../../scripts/dispatch.sh) | Dedicated worktree, frozen dependency install, stable per-dispatch claim ID, live claim confirmation, handoff | Work coordination only; no publishing authority |

The packaging matrix keeps Linux smoke evidence after a Windows-only bundle failure; a missing Linux
artifact fails download. Actions use commit SHAs, smoke images use digests, AppImage downloads have
checksums. Ubuntu 24.04 is the actual glibc build baseline; Ubuntu 22.04 and universal macOS in older
planning tables are not validated support claims. DEN-35's final Claude handoff records green hosted
run `39217241997` at `6b7aa8a`; that is historical evidence, not a fresh run of this spec.

[Packaging instructions](../packaging.md) and [the Linux build script](../../packaging/build-linux.sh)
are canonical: compile with display name `StreamDen`, then bundle with the Linux override for native
name `streamden`. Plain `cargo tauri build` does not produce the required Linux package identity.
MSI upgradeCode stays `544682fc-4369-5784-9d8b-20cea11a1e99`; NSIS installs for the current user.
WebView2 bootstrapper requires network when the runtime is missing. Current packages omit `streamdend`;
the [systemd user unit](../../packaging/systemd/streamdend.service) is an uninstalled P1 draft.
Both AUR recipes are unpublished, x86_64-only and intentionally have failing zero checksums.

## 3. Release identity and data model

Proposed scheme (Q-B8): SemVer `MAJOR.MINOR.PATCH`, tags `v<version>`; prereleases
`0.1.0-alpha.N`, `0.1.0-beta.N`, `0.1.0-rc.N`. Stable contains no prerelease suffix; rc belongs
to beta. `0.0.0` is development-only. No build metadata in public versions. A new public payload gets a
new version; never replace a tag or asset to repair a release. App version, `rpcVersion` and data-schema
version are independent; release notes state compatibility and migration requirements explicitly.

The release preparation PR synchronizes workspace/app Cargo versions, desktop Tauri version, applicable
web package versions and packaging recipes, regenerating lockfiles only in their owning task. SDKs may
version independently. Validate equality of app versions and tag before building. Native version mapping
must be tested before prerelease installers ship: Debian `~` ordering, rpm EVR and Arch vercmp must agree
with channel intent; Windows installer numeric versions must increase across prerelease → stable updates.
Do not silently strip prerelease suffixes into colliding MSI versions. A release is blocked for a format
until its monotonic mapping and upgrade tests pass; this spec does not assert Tauri provides that mapping.

Rust-ish release-tooling model (future schema-generated JSON, not a `den-protocol` declaration):

```rust
struct ReleaseManifest {
    schema_version: u32, // exactly 1
    version: String, // validated SemVer, no build metadata
    channel: Channel, // stable | beta | alpha
    commit: String, // full 40-character peeled commit SHA: refs/tags/vX^{commit}
    source_date_epoch: u64, // commit timestamp, seconds UTC
    artifacts: Vec<Artifact>,
}
struct Artifact {
    name: String, // basename only, unique; no path traversal
    kind: ArtifactKind, // package | source | sbom | signature
    target: Option<String>, // required only for packages: OS + architecture + format
    bytes: u64, // positive, checked against downloaded file
    sha256: String, // exactly 64 lowercase hex digits
    signature: Option<String>, // detached signature asset basename
    sbom: Option<String>, // required for package/source; absent for sbom/signature assets
}
enum Channel { Stable, Beta, Alpha }
enum ArtifactKind { Package, Source, Sbom, Signature }
```

Schema rejects unknown fields, duplicate names/package targets and missing required artifacts.
The manifest lists packages, source, SBOMs and their detached signatures; all references must resolve to
the matching kind. It excludes itself, SHA256SUMS and the checksum-set signature to avoid hash cycles.
Source and SBOM/signature entries have no target; one source SBOM describes the resolved source graph. Signing keys and
credentials never appear in the manifest. File names may retain native bundler conventions; the manifest
provides the stable target mapping. `SHA256SUMS` covers final packages, source archive, SBOMs and manifest;
it excludes itself and its detached signature. The release is identified by tag, commit and checksum-set
digest, not by a mutable download name. Tauri update JSON is a separate generated projection, validated
against the pinned plugin's format; do not treat this manifest as its wire format.

## 4. Tag to publication pipeline (P1 requirement)

1. A reviewed release-preparation PR records notes, versions, supported targets and migration/backup guidance.
   Tag creation is an owner action. Validate a strict version tag, approved commit reachable from protected
   main, matching versions, repository identity and all required checks for that exact commit. `v*` alone
   is not authorization. Re-run checks if exact-commit evidence is absent.
2. Build with read-only credentials from the tag's immutable SHA in clean runners. Use frozen pnpm install,
   pinned Rust/CLI, locked Cargo dependencies and `packaging/build-linux.sh` for Linux. Required v0.1
   assets are x86_64 deb/rpm/AppImage plus source and metadata. Windows compilation remains required;
   public Windows installer support waits for DEN-20 and signing readiness. Do not silently omit a required
   target when a matrix job fails; optional targets must be declared in the release plan.
3. Run distro installation and package metadata tests against the exact candidate artifacts. Keep hashes
   and workflow run IDs. Before public Linux support, exercise AppImage execution and the reference
   Wayland/NVIDIA host too; the current container probe is insufficient for that claim.
4. Produce SPDX JSON SBOMs from resolved Cargo/npm graphs and bundled native/tool/runtime components,
   including versions, licenses and hashes where available. Attach the exact corresponding source archive,
   build scripts, lockfiles and license notices; audit redistribution obligations under ADR-0003.
   Unknown or incompatible licenses block release pending review, not a silent scanner allowlist.
5. After a protected release-environment approval, sign the candidate packages using the applicable
   platform mechanism in §5, including inner Windows binaries before installer assembly. Verify signatures
   and repeat install/upgrade checks on final signed bytes.
   Generate final checksums, manifest, signature files and provenance after all byte-changing operations.
   Keep build jobs unprivileged; signing jobs must not execute candidate repository scripts with secrets.
6. A separately permissioned publisher (`contents: write` only where needed) creates a draft release in
   `tech-daddy-digital/streamden`, uploads the complete set and verifies downloaded hashes/signatures.
   Re-check tag→commit binding. Publish only after the full set passes; prerelease flags match channel.
   A retry may fill a draft's missing identical assets, but must reject conflicting bytes and never overwrite
   published assets. Serialize publication per version/channel and never cancel signing/publishing midway.
7. Publish updater metadata only after every referenced artifact is public and verified. Advance each
   channel pointer last; a failure leaves the previous pointer valid. AUR publication follows release
   verification, with separate credentials and approval; failure there must not corrupt the release.

No binaries may be publicly released from `TechDaddyKB/streamden` (ADR-0004). Repo transfer and restoration
of protections/secrets precede step 1. Existing [owner runbook](../owner/registrations/distribution-and-signing.md)
and [owner checklist](../owner/README.md) describe account/key setup; DEN-17 tracks baseline accounts.
This spec requires no account or credential creation now. Release implementation must reconcile that
runbook's old unanswered-name wording with accepted ADR-0004 before asking the owner to perform steps.

## 5. Signing and supply chain

Trust layers are distinct: HTTPS transport, a signed checksum set, native package signature and updater
payload signature. A checksum downloaded beside an artifact detects corruption but does not alone prove
publisher identity. Publish trusted key fingerprints independently of the release asset set.

| Format | Required verification plan | Gate |
|---|---|---|
| AppImage | Detached signature over release checksum set; Tauri signature additionally for self-update | Verify exact final bytes before replacement |
| deb | Signed release checksum set for direct downloads | Do not claim apt authenticates a standalone deb; a future apt repo needs signed repository metadata |
| rpm | Native RPM GPG signature plus signed release checksum set | Verify with trusted key and reject wrong/absent signatures |
| AUR | Reviewed PKGBUILD/.SRCINFO, real source digests, upstream signature verification | AUR account/SSH authentication is not a binary signature |
| Windows MSI/NSIS | Authenticode and timestamp; verify chain and intended signer | SignPath OSS application/approval, policy and DEN-20; no assumed acceptance |
| macOS | Best effort unsigned/ad-hoc until D-024 milestone; Developer ID/notarization later | Clearly label tier 2; no current macOS CI/runtime evidence |

[Tauri RPM documentation](https://v2.tauri.app/distribute/rpm/) documents GPG signing;
[Tauri Windows signing](https://v2.tauri.app/distribute/sign/windows/) describes native signing.

To preserve the unprivileged-build boundary, do not pass signing keys to `tauri build`. Use pinned
post-build `rpmsign --addsign` for RPMs, then verify with `rpm --checksig`; sign the checksum set with GPG.
Generate updater payloads without credentials and use the pinned CLI `tauri signer sign` separately on
each final payload. Bind the version into authenticated metadata/signatures and verify it on the client;
a replayed old payload labeled with a higher version must fail. Prove the pinned CLI/plugin support this
(or add a verified signed-manifest layer) before enabling updates. See the
[standalone signer reference](https://v2.tauri.app/reference/cli/#signer-sign).
Verify those detached signatures before generating the updater JSON. These replace
the build-time signing paths, not their signature formats.

Windows requires an explicit staged pipeline: build unsigned PE files → protected signing of app/sidecar
executables → installer assembly with pinned, reviewed WiX/NSIS tooling → sign installers. The NSIS
uninstaller must also be signed during assembly via a reviewed signing hook/service; assembly must not
execute arbitrary candidate build scripts with signing credentials. A SignPath integration may instead
perform equivalent nested signing only after a proof that it covers the installed executables and
uninstaller. The current one-shot Tauri command does not establish this pipeline. If tooling cannot
produce and verify every required inner/outer signature without exposing keys to candidate build code,
Windows publication remains blocked; never fall back to outer-installer-only signing.

[SignPath terms](https://signpath.org/terms) require an eligible open-source project and signing policy;
Foundation acceptance is discretionary. SignPath is the proposed Windows path, not a configured service
or promise of a Tech Daddy Digital certificate. Q-B10 retains owner approval of signer/cost alternatives.

Release and updater private keys are separate credentials with documented owners, encrypted backup and
recovery drills. Only protected jobs receive process-scoped secrets; no PR/fork job, cache, artifact,
command line or log may contain them. Separate signing, publishing and AUR permissions. No
`pull_request_target` checkout of untrusted code with secrets. SHA-pin actions and checksum/digest-pin
external tools/images; verify downloaded bytes before execution. Do not run automatic Sonar checks.

Preserve D-040 and the effective pnpm `minimumReleaseAge`; never bypass the age gate to obtain a newer
package. The checkout does not declare a numeric age in `pnpm-workspace.yaml`: implementation must record
and enforce the effective approved policy in CI rather than invent a value or assume a developer's global
configuration is portable. Dependency/lockfile changes are separate reviewed tasks. Add license and
vulnerability checks for both dependency graphs, with dated, justified exceptions; no automatic acceptance
of new licenses. Tool choice/pins are implementation work, not a new dependency installed by this spec.

## 6. Update channels, API and UI (proposed, Q-B9)

Recommend stable/beta/alpha with explicit opt-in to prereleases. Stable never offers a prerelease; beta
accepts beta/rc/stable; alpha accepts all. Compare parsed SemVer, not strings. Switching to a lower channel
never forces a downgrade. Install format is recorded by packaging/runtime evidence, not inferred solely
from file extension: deb/rpm/AUR/Flatpak defer to their package manager; unknown/development installs are
manual-only. Direct deb/rpm downloads need a newer verified package until a repository exists; `apt upgrade`
alone does not discover GitHub assets. No application sudo invocation or overwrite of package-owned files.

Tauri requires an updater signature and embedded public key; this is independent of native signing.
Its supported payload formats and static metadata must follow the pinned plugin documentation
([Tauri updater](https://v2.tauri.app/plugin/updater/)). The current plugin is not installed and
`createUpdaterArtifacts` is false. Do not enable it before keys, metadata validation and contract tests exist.

Candidate public contract for a later ADR/contract card (all types serde + schemars, generated UI/API):

| Schema | Fields / constraints |
|---|---|
| `UpdateSettings` | `channel`: stable/beta/alpha; `checkMode`: manual/notify; defaults proposed stable/manual; no custom endpoint or private key fields |
| `UpdateStatus` | state: idle/checking/available/downloading/verified/installing/failed; currentVersion; optional candidateVersion; installKind; bounded error code |
| `UpdateCandidate` | version, channel, target, byte count, sha256, trusted release URL; validated HTTPS origin and compatible target |
| `UpdateProgress` | state plus downloaded/total bytes; counters monotonic, total bounded by candidate size |

Proposed requests: `system.update.status`, `system.update.check`, `system.update.configure`,
`system.update.download`, `system.update.install`; names are reserved proposals, not existing endpoints.
Expose identical authorized semantics to UI and third parties through Spec 02/Spec 06; no private UI-only
backdoor. Status/check are read capabilities; configuration/download/install require separately declared
capabilities. Local confirmation is required for installation, enabling notifications, or changing to a
channel that admits more prereleases. Restricted/MCP principals cannot bypass confirmation.
No update-install automation sub-action or trigger is proposed; status/progress events are informational
and must not automatically restart the engine. Sub-action schemas: none for WS-01 r1.

Flow: Settings → Updates shows version, source/channel and package-manager/manual status. Check displays
release notes and compatibility warnings. Download is explicit. Verify size, SHA-256 and updater signature
before allowing Install. Reject HTTP, wrong OS/arch/channel, duplicate target, malformed/oversized metadata,
old/equal versions and invalid signatures; only approved HTTPS origins/redirects are accepted. Render notes
as sanitized text/Markdown, never executable HTML. Do not log URLs containing credentials.

Install requires a stopped/quiescent engine and verified pre-update backup (23.15); connected remote daemon
is never restarted by a local UI update. If busy, unknown, backup fails or disk is insufficient, defer.
Use a single update operation, bounded streaming download, temporary files and atomic publication of the
verified payload. On cancellation/failure preserve the installed executable and data. Relaunch only with
consent. No silent download/install/restart; notification frequency and rollout remain Q-B9.

Lost updater key: block self-update and offer an independently verified manual reinstall. Rotation needs
a bridge release trusted by the old key before new-key-only releases; key compromise requires a separate
incident/revocation plan, not trust in the compromised feed. Removing a bad channel pointer cannot revoke
already downloaded payloads. Withdraw the bad version from future offers and publish a higher fixed version;
never auto-downgrade a migrated database. Recovery instructions identify a compatible backup and previous
verified installer; restore only with user consent, preserving the newer data separately.

## 7. Platform, reproducibility and budgets

Linux v0.1 baseline is proposed in Q-P4: current x86_64 Ubuntu 24.04 build plus exact digest-pinned
Debian/Fedora/Arch install tests; publish actual distro versions, glibc minimum and required WebKitGTK.
AppImage is not proof of compatibility with all Linux distributions. Preserve Wayland/X11 portability;
software-rendering environment overrides stay confined to the container probe. DEN-20 owns KDE/Windows
runtime acceptance. Native Windows builds use MSVC; installer upgrade/uninstall tests must preserve user
data and MSI identity. macOS tier 2 is deferred, not a required current CI job; future native architecture
builds must validate deployment target and signing/notarization before claiming universal support.

P1 daemon packaging depends on an actual persistent engine and its separate card: version-coupled engine/UI,
explicit user-service enablement, no automatic system service or root daemon. Future worker/runtime assets
must enter the manifest/SBOM and package dependency tests; do not add speculative Python/.NET dependencies.

Reproducibility means traceable inputs first, not a claim of bit-identical signed installers. Record toolchain,
Node/pnpm/CLI versions, runner image revision, dependency locks, container/tool digests, native package
versions, environment allowlist, commit and build commands. Set `SOURCE_DATE_EPOCH` where supported and
normalize archive order/timestamps. Compare two clean unsigned builds and report differing files; native
signing timestamps are expected differences. Hosted runner labels and apt repositories are mutable, so
full hermetic reproduction needs snapshots; do not label current builds hermetic. Never include secrets,
absolute private paths or untracked workspace content in source/provenance archives.

| Check | Current timeout | Proposed P1 operating budget |
|---|---|---|
| Docs / commit checks | 10 min each | Warm median ≤ 2 min, fail at 10 min |
| Protocol per OS | 20 min | Warm median ≤ 5 min |
| Workspace per OS | 45 min | Warm median ≤ 15 min |
| Bundles per OS | 60 min | Warm median ≤ 30 min |
| Install smoke per distro | 30 min | Warm median ≤ 10 min |

Measure at least ten hosted runs, distinguishing cache state and queue time; these are engineering targets,
not observed performance. Cancel superseded PR checks; preserve release jobs. Main's concurrency queue is
not evidence that every commit ran: release gates require exact-SHA results. Cache keys include OS/target,
toolchain and locks; only trusted main/tag jobs save. Avoid compiling test tools on every matrix leg;
use reviewed pinned cached tooling. B-4's static actionlint/dry runs never count as hosted CI evidence.
Updater proposal budgets: 1 MiB metadata cap, 10-second check timeout, one check/download at a time,
progress ≤ 4 events/sec, disk-streamed downloads rather than full payload RAM buffering. Download size cap
is the validated manifest size plus a configured release limit; abort excess bytes and clean partial files.

## 8. Acceptance and implementation sequence

| Test | Required result before relevant feature ships |
|---|---|
| Tag/version fixtures | Invalid tags, mismatched manifests, numeric-installer collisions and wrong repo fail before signing |
| Clean build replay | Same inputs recorded; unsigned differences explained; no claim based only on exit 0 |
| Release dry run | No secrets or publication; expected target set, SBOM/source coverage, hashes validated |
| Signing fixtures | Tampering, wrong key, missing signature and changed post-sign bytes rejected |
| Windows nested signing | Installed app/sidecars and NSIS uninstaller, plus MSI/NSIS installers, have valid intended-signer Authenticode and timestamps |
| Publisher retry/failure | Partial draft never promoted; conflicting asset rejected; channel unchanged on failure |
| Install matrix | Native names/files/icons/deps, three smoke controls and real launch pass for final bytes |
| Upgrade/uninstall | Previous supported version → candidate preserves config; uninstall leaves user data; daemon not enabled |
| AUR clean build | Real release checksum, `.SRCINFO`, PKGBUILD and package-level namcap; unprivileged makepkg and install |
| Updater fixtures | Channel order, stale metadata, wrong target, redirect, oversize, network/disk failure and cancellation |
| Update recovery | Busy engine/failed backup blocks install; interrupted replacement and failed migration preserve recovery path |
| Platform validation | Linux Wayland evidence; Windows compile now, DEN-20 runtime later; macOS limitations explicit |

Implementation order: (1) approve owner questions/spec; (2) release metadata/version validator and CI gates;
(3) protected signing/publishing with owner setup; (4) AUR publishing; (5) updater ADR/contracts, backup and
shell/service coordination; (6) updater implementation and end-to-end recovery tests. Each gets a scoped
board card; no workflow, package, public type or secret is changed by DEN-66.

Spec checks use the exact tracked-Markdown selection in Docs CI, then
`xargs -0 -r pnpm exec markdownlint-cli2` and
`xargs -0 -r lychee --config .lychee.toml --` on that same NUL-delimited list (lychee 0.24.2).
Stage the new spec before selection so it is included. Claude review is required; owner resolves product
questions and approves the spec. No unrun hosted, signing, installer or updater test is claimed here.
