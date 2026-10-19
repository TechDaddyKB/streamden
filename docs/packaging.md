# Packaging skeleton (DEN-35)

The P0 packages contain the desktop shell at version `0.0.0`. They are unsigned test
artifacts, with no updater metadata. The daemon and its systemd unit are not installed:
`streamdend` still prints its version and exits. The unit in
[`packaging/systemd`](../packaging/systemd/streamdend.service) is a P1 draft, with
no automatic restart or enablement.

## Build and collect

From the repository root, with the [repository prerequisites](../AGENTS.md) installed:

```bash
pnpm install --frozen-lockfile --ignore-scripts
cargo install tauri-cli --version 2.12.3 --locked
bash packaging/fetch-appimage-tools.sh
bash packaging/build-linux.sh
```

Linux produces `target/release/bundle/{deb,rpm,appimage}` at the workspace root.
Native deb/rpm package names are explicitly `streamden`; artifact names start with
`streamden` too. Tauri 2.12.3 has no deb/rpm package-name field and derives these
names from `productName`. The build script first runs
`cargo tauri build --no-bundle -- --locked` with the original
`productName: "StreamDen"`, then `cargo tauri bundle`
with [`tauri-linux-bundle.json`](../packaging/tauri-linux-bundle.json). This bundle-only
override sets `productName` to `streamden`; the compiled application's display name
stays `StreamDen`. The shared desktop template also keeps `Name=StreamDen`.
Use this script for Linux distribution builds: plain `cargo tauri build` still uses
Tauri's default `stream-den` package name. Windows uses the original config directly.
Windows produces MSI and NSIS installers. The MSI upgrade code is frozen to Tauri's
current product-derived value; preserve it across updates. NSIS defaults to the current
user. WebView2's bootstrapper downloads the runtime if needed; installers are not offline.
No signing certificate or publishing credentials are configured.

The [Packaging workflow](../.github/workflows/release-packaging.yml) runs manually, on
`v*` tags, and on pull requests touching packaging, apps, crates, web packages,
workspace manifests/lockfiles, Rust toolchain/config, or the packaging workflow itself.
It builds on Ubuntu 24.04 and Windows 2025, then
uploads bundles for 14 days before the separate install-smoke jobs run. It never creates
a GitHub release. Install-smoke jobs also run after a failed bundle matrix, so a
Windows-only failure does not suppress Linux evidence; missing Linux artifacts still
fail the download step. All actions
and container images are pinned by digest; the Linux AppImage tools have SHA-256 checks
in [`fetch-appimage-tools.sh`](../packaging/fetch-appimage-tools.sh). A changed upstream
continuous asset fails verification and requires review before updating its digest.
`pnpm install` keeps the repository minimumReleaseAge policy and frozen lockfile.

Ubuntu 24.04 is the CI build baseline. Digest snapshots of `debian:stable-slim`, `fedora:latest`
and `archlinux:latest` are the install-test targets (resolved on 2026-10-19). Older
distributions are not validated; local Arch builds may need
newer glibc than these targets, so use CI artifacts for distribution tests. This narrows
the older Ubuntu 22.04 baseline suggested in [the research](research/tech-stack.md#101-what-the-tauri-bundler-produces).

## Install and launch probes

Three independent matrix jobs download the Linux artifact from the same workflow run.
[`install-smoke.sh`](../packaging/install-smoke.sh) installs the deb with apt, the rpm
with dnf, and a package built from the `streamden-bin` draft with `pacman -U`. For Arch,
it copies the draft to a temporary build directory and substitutes only its source
location and SHA-256 with the downloaded deb. This exercises the actual AUR payload
extraction and dependency list without a published release or disabled checksums;
`makepkg` runs as an unprivileged user. The checked-in draft stays fail-closed.

Each deb/rpm job asserts the native package name is `streamden`. Each job lists
package-owned files and asserts `/usr/bin/streamden`,
`/usr/share/applications/streamden.desktop` with `Name=StreamDen`, and the 32/128/512-pixel icons in
`/usr/share/icons/hicolor`. It logs `ldd /usr/bin/streamden` and fails on unresolved
libraries or an ldd error. If `/usr/bin/streamdend` is present it runs
`streamdend --version`; the current desktop-only packages explicitly skip that check.
The jobs create an unprivileged user, then run
[`smoke-launch.sh`](../packaging/smoke-launch.sh) under an isolated Xvfb
and D-Bus session. X11/software-rendering settings apply only to this probe. Success means
the actual StreamDen PID survived 15 seconds. The probe fails on any earlier desktop
exit (including exit 0) or infrastructure failure; a 30-second outer timeout also fails.
Session logs are printed for diagnosis, but portal FUSE, D-Bus and Gtk helper warnings
do not determine success. Each install job first runs
[`test-smoke-launch.sh`](../packaging/test-smoke-launch.sh): a surviving fixture with
portal/display noise must pass, while a SIGSEGV fixture and an early clean exit must fail.
Run those controls locally with `bash packaging/test-smoke-launch.sh`. The launch check does not
assert rendered pixels, Wayland/NVIDIA behaviour, or Windows runtime compatibility.

Exact install-and-launch commands attempted on DailyDriver (2026-10-19), from the repo root:

```bash
docker run --rm -v "$PWD:/work:ro" debian:trixie-slim@sha256:918311b7b6c4c6f68b232ba516584925f6c78ad82b6fd534b98979df6438e483 bash -euxc '
  apt-get update
  apt-get install -y /work/target/release/bundle/deb/*.deb xvfb xauth dbus-x11
  useradd -m probe
  runuser -u probe -- bash /work/packaging/smoke-launch.sh
'
docker run --rm -v "$PWD:/work:ro" fedora:44@sha256:be9d65e2344d805cc11114319c685ecaa96b6d9b4350a0a6460cdb931babbd19 bash -euxc '
  dnf install -y /work/target/release/bundle/rpm/*.rpm xorg-x11-server-Xvfb xorg-x11-xauth dbus-x11
  useradd -m probe
  runuser -u probe -- bash /work/packaging/smoke-launch.sh
'
```

Both exited 1 with `permission denied while trying to connect to the docker API at unix:///var/run/docker.sock`.
The diagnostic reproduced it:

```console
$ docker info
Client: Version 29.9.1
Server:
permission denied while trying to connect to the docker API at unix:///var/run/docker.sock
# exit 1; no container was created, no package was installed
```

Podman was not installed. The Docker denial was not bypassed. Coordinator: push this
branch and run Packaging with `workflow_dispatch`; retain the Linux/Windows artifacts
and all three install-smoke job logs. Packaging-related pull requests also run these
jobs before merge. Local Docker and Windows installer execution remain unavailable.
The exact current CI reproduction, after downloading
the Linux artifact into `artifacts/` with its `deb/`, `rpm/` and `appimage/` directories, is:

```bash
# Use the digest-pinned SMOKE_IMAGE from the workflow for the chosen distribution.
docker run --rm -v "$PWD:/work:ro" "$SMOKE_IMAGE" \
  bash /work/packaging/install-smoke.sh debian  # or fedora / arch
```

## AUR drafts

[`streamden-bin`](../packaging/aur/streamden-bin/PKGBUILD) extracts Tauri's gzip Debian
payload; [`streamden`](../packaging/aur/streamden/PKGBUILD) builds the release archive
using `rustup` (an explicit build dependency) to honour the workspace's pinned Rust
toolchain, frozen Cargo dependencies and locked pnpm install with scripts disabled.
It uses the same separate compile/bundle configuration as the Linux build script.
Both target x86_64 only, matching this workflow. Modern
Arch package hooks handle icon/desktop caches; no custom install hooks are needed.

These are **unpublished drafts**. Release URLs use the permanent organisation from
[ADR-0004](adr/0004-naming-identifiers-hosting.md). No `v0.0.0` release is assumed to
exist. All-zero checksums deliberately fail source verification. Before AUR publication,
transfer the repository, create reviewed release assets, replace versions/checksums,
provide `cargo-tauri` 2.12.3 or newer, generate `.SRCINFO` with `makepkg --printsrcinfo`,
then run package-level namcap and validate the published source in a clean Arch
environment. CI builds and installs the local-artifact variant described above. PKGBUILD
lint does not establish that a release exists or that its dependencies install.

Patterns: [Tauri AUR guide](https://v2.tauri.app/distribute/aur/) and
[AppImage guidance](https://v2.tauri.app/distribute/appimage/). No Python/.NET/audio
runtime dependencies are added for features the P0 shell does not implement.

## Local evidence (DailyDriver, 2026-10-19)

- `pnpm install --frozen-lockfile --ignore-scripts`: exit 0, lockfile current.
- `cargo tauri build -- --locked` (desktop): exit 0, finished 3 bundles:
  `StreamDen_0.0.0_amd64.deb` (2.87 MiB), `StreamDen-0.0.0-1.x86_64.rpm`
  (2.87 MiB), `StreamDen_0.0.0_amd64.AppImage` (102.09 MiB).
- namcap 3.6.0 on both PKGBUILDs: exit 0, no diagnostics. Tool and required Python
  modules were extracted into `/tmp/den35-tools` from signature-verified Arch packages;
  no global packages were installed.
- actionlint 1.7.12 on the packaging workflow: exit 0, no diagnostics.
- Extracted Debian payload launched on Arch through the same probe: exit 0,
  `Desktop stayed running for 15 seconds under Xvfb (rendering not asserted).`
  Isolated-session portal/accessibility warnings were present. This was not a Debian install.
- `cargo build --workspace --locked`, `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `pnpm -r typecheck`, `pnpm -r build`: all exit 0.
  The scaffold currently contains zero Rust tests.
- `pnpm dlx markdownlint-cli2@0.18.1 docs/packaging.md`: zero errors.
- AppImage tool bootstrap: downloaded all three assets into an empty temporary cache
  and verified their SHA-256 pins. Tauri clears linuxdeploy's AppImage magic bytes
  in its cache; the download pin uses the original upstream bytes.
- `makepkg --printsrcinfo` from each draft directory: exit 0 (output kept in `/tmp`).
- `bash -n` on packaging scripts/PKGBUILDs: exit 0. Unit syntax checked with a temporary
  copy substituting `/usr/bin/true` for the unavailable `/usr/bin/streamdend`; exit 0.
  The original unit cannot pass executable validation until the daemon is installed.

Round 2 rechecked the Tauri build (all three bundles), both PKGBUILD lints, actionlint,
shell syntax, Markdown lint and all six workspace checks above: all exit 0. A temporary
copy of the `streamden-bin` draft, adapted exactly as in CI, passed checksum validation
and `makepkg --nodeps --noconfirm` on Arch and contained the expected binary, desktop
entry and all three icons. This local probe skipped dependency checks and did not
install the package; CI checks dependencies and installs it with pacman. Native package
names were read from artifact metadata (before round 4, the deb was `stream-den`).
Local Docker access remains unavailable.

Round 3 replaces the session-log grep with direct desktop PID survival. The coordinator
reports hosted run `39212543221` on PR #17 installed successfully on all three distros:
Arch passed; Debian/Fedora failed the former diagnostic check on container portal noise.
Local regression controls now pass for noisy survival and reject SIGSEGV (status 139)
and early clean exit (status 0). They run in every install-smoke container before the
installed desktop probe. Claude's review verified hosted run
[39214623808](https://github.com/TechDaddyKB/streamden/actions/runs/39214623808) at
`2b760be`: both bundle jobs and all three install jobs passed, including controls and
15-second desktop survival. Round 4's renamed packages still need a hosted rerun.

Round 4 rebuilt all three Linux bundles through `bash packaging/build-linux.sh`:
`streamden_0.0.0_amd64.deb`, `streamden-0.0.0-1.x86_64.rpm` and
`streamden_0.0.0_amd64.AppImage`. Direct deb control and rpm header inspection assert
`Package: streamden` / `Name: streamden`; desktop entries in the deb and AppImage
assert `Name=StreamDen`. The deb long description is populated. Both PKGBUILD lints,
actionlint, Markdown lint and all six workspace checks above passed again.
The three launch controls passed, and the renamed deb payload's desktop survived
15 seconds on Arch (exit 0); this does not establish Debian installation success.
The Debian command above was retried with the explicit
`/work/target/release/bundle/deb/streamden_0.0.0_amd64.deb` path and `bash -ec`:
exit 1, the same Docker socket permission denial; no container was created.

Rollback: revert the DEN-35 commits; no host package, service or release was installed.
