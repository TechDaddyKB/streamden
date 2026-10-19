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
cd apps/desktop
cargo tauri build -- --locked
```

Linux produces `target/release/bundle/{deb,rpm,appimage}` at the workspace root.
Windows produces MSI and NSIS installers. The MSI upgrade code is frozen to Tauri's
current product-derived value; preserve it across updates. NSIS defaults to the current
user. WebView2's bootstrapper downloads the runtime if needed; installers are not offline.
No signing certificate or publishing credentials are configured.

The [Packaging workflow](../.github/workflows/release-packaging.yml) runs manually or
on `v*` tags, on Ubuntu 24.04 and Windows 2025. It uploads bundles for 14 days, including
when a subsequent Linux smoke test fails. It never creates a GitHub release. All actions
and container images are pinned by digest; the Linux AppImage tools have SHA-256 checks
in [`fetch-appimage-tools.sh`](../packaging/fetch-appimage-tools.sh). A changed upstream
continuous asset fails verification and requires review before updating its digest.
`pnpm install` keeps the repository minimumReleaseAge policy and frozen lockfile.

Ubuntu 24.04 is the CI build baseline. Debian 13 (trixie) and Fedora 44 are the initial
install-test targets. Older distributions are not validated; local Arch builds may need
newer glibc than these targets, so use CI artifacts for distribution tests. This narrows
the older Ubuntu 22.04 baseline suggested in [the research](research/tech-stack.md#101-what-the-tauri-bundler-produces).

## Install and launch probes

The workflow installs each native bundle using its package manager, creates an unprivileged
user, then runs [`smoke-launch.sh`](../packaging/smoke-launch.sh) under an isolated Xvfb
and D-Bus session. X11/software-rendering settings apply only to this probe. Success means
the process survived 15 seconds without selected startup-error diagnostics; it does not
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
and Debian/Fedora step logs. Neither hosted run nor Windows installer execution was
available during this local session.

## AUR drafts

[`streamden-bin`](../packaging/aur/streamden-bin/PKGBUILD) extracts Tauri's gzip Debian
payload; [`streamden`](../packaging/aur/streamden/PKGBUILD) builds the release archive
using the workspace's pinned Rust toolchain, frozen Cargo dependencies and locked pnpm
install with scripts disabled. Both target x86_64 only, matching this workflow. Modern
Arch package hooks handle icon/desktop caches; no custom install hooks are needed.

These are **unpublished drafts**. Release URLs use the permanent organisation from
[ADR-0004](adr/0004-naming-identifiers-hosting.md). No `v0.0.0` release is assumed to
exist. All-zero checksums deliberately fail source verification. Before AUR publication,
transfer the repository, create reviewed release assets, replace versions/checksums,
provide `cargo-tauri` 2.12.3 or newer, generate `.SRCINFO` with `makepkg --printsrcinfo`,
then build/install and run package-level namcap in a clean Arch environment. PKGBUILD
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

Rollback: revert the DEN-35 commit; no host package, service or release was installed.
