#!/usr/bin/env bash
# Run only inside the disposable CI container, with downloaded artifacts at /work.
set -euo pipefail
shopt -s nullglob

case "${1:?expected debian, fedora or arch}" in
  debian)
    bundles=(/work/artifacts/deb/StreamDen*.deb)
    [[ ${#bundles[@]} == 1 ]]
    apt-get update
    apt-get install -y "${bundles[0]}" xvfb xauth dbus-x11
    package_name=$(dpkg-deb -f "${bundles[0]}" Package)
    dpkg-query -L "$package_name"
    ;;
  fedora)
    bundles=(/work/artifacts/rpm/StreamDen*.rpm)
    [[ ${#bundles[@]} == 1 ]]
    dnf install -y "${bundles[0]}" xorg-x11-server-Xvfb xorg-x11-xauth dbus-x11
    package_name=$(rpm -qp --queryformat '%{NAME}' "${bundles[0]}")
    rpm -ql "$package_name"
    ;;
  arch)
    bundles=(/work/artifacts/deb/StreamDen*.deb)
    [[ ${#bundles[@]} == 1 ]]
    pacman -Syu --noconfirm --needed base-devel namcap xorg-server-xvfb xorg-xauth dbus
    useradd -m probe
    install -d -o probe -g probe /tmp/aur-smoke
    cp /work/packaging/aur/streamden-bin/PKGBUILD /tmp/aur-smoke/PKGBUILD
    cp "${bundles[0]}" /tmp/aur-smoke/streamden.deb
    artifact_sha=$(sha256sum /tmp/aur-smoke/streamden.deb | cut -d ' ' -f 1)
    # Adapt only the source location and checksum; retain the draft's package logic.
    sed -i -e "s|^source=.*|source=('streamden.deb')|" \
      -e "s|^sha256sums=.*|sha256sums=('$artifact_sha')|" /tmp/aur-smoke/PKGBUILD
    chown -R probe:probe /tmp/aur-smoke
    cd /tmp/aur-smoke
    runuser -u probe -- makepkg --printsrcinfo > /tmp/aur-smoke.srcinfo
    mapfile -t runtime_deps < <(sed -n 's/^[[:space:]]*depends = //p' /tmp/aur-smoke.srcinfo)
    [[ ${#runtime_deps[@]} -gt 0 ]]
    pacman -S --noconfirm --needed "${runtime_deps[@]}"
    runuser -u probe -- namcap PKGBUILD
    runuser -u probe -- makepkg --noconfirm
    packages=(/tmp/aur-smoke/streamden-bin-*.pkg.tar.zst)
    [[ ${#packages[@]} == 1 ]]
    pacman -U --noconfirm "${packages[0]}"
    pacman -Ql streamden-bin
    ;;
  *) echo 'Unknown install-smoke distribution' >&2; exit 2 ;;
esac

# Assert installed paths, including the desktop entry and every bundled icon size.
test -x /usr/bin/streamden
test -s /usr/share/applications/StreamDen.desktop
for size in 32 128 512; do
  test -s "/usr/share/icons/hicolor/${size}x${size}/apps/streamden.png"
done
if ! ldd /usr/bin/streamden > /tmp/streamden-ldd.log 2>&1; then
  cat /tmp/streamden-ldd.log
  exit 1
fi
cat /tmp/streamden-ldd.log
if grep -Fq 'not found' /tmp/streamden-ldd.log; then
  echo 'Installed streamden has unresolved shared libraries' >&2
  exit 1
fi
if [[ -e /usr/bin/streamdend ]]; then
  /usr/bin/streamdend --version
else
  echo 'streamdend is not packaged in the P0 desktop bundle; version check skipped.'
fi
if ! id probe >/dev/null 2>&1; then
  useradd -m probe
fi
runuser -u probe -- bash /work/packaging/test-smoke-launch.sh
runuser -u probe -- bash /work/packaging/smoke-launch.sh
