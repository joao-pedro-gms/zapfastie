#!/usr/bin/env bash
# Usage: bash packaging/test-install.sh ubuntu:24.04 native-packages-output
# The compiler runs on the host so it cannot supply missing runtime libraries.
set -euo pipefail
image=${1:?Supply an Ubuntu, Debian or Fedora container image}
packages=$(realpath "${2:?Supply a native-packages output directory}")
case "$(uname -m)" in
  x86_64) target=linux-amd64 ;;
  aarch64) target=linux-arm64 ;;
  *) echo 'Unsupported test architecture' >&2; exit 1 ;;
esac
case "$image" in
  ubuntu:*|debian:*) format=deb ;;
  fedora:*) format=rpm ;;
  *) echo 'Unsupported test distribution' >&2; exit 1 ;;
esac
package_dir="$packages/packages/$target/$format"
test -d "$package_dir"
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
checks=$(mktemp -d)
trap 'rm -rf -- "$checks"' EXIT
cc -std=c99 -Wall -Wextra -Werror "$script_dir/check-runtime-libs.c" -ldl -o "$checks/check-runtime-libs"
docker run --rm \
  --volume "$package_dir:/packages:ro" \
  --volume "$checks:/checks:ro" \
  --env "FORMAT=$format" "$image" sh -ec '
    set -- /packages/*."$FORMAT"
    test "$#" -eq 1
    test -f "$1"
    mkdir -p /root/.config/zapfastie
    printf "%s\n" "preserve-existing-settings" > /root/.config/zapfastie/fixture
    if [ "$FORMAT" = deb ]; then
      apt-get update
      DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "$1"
      dpkg-query -W zapfastie
    else
      dnf install -y --setopt=install_weak_deps=False "$1"
      rpm -q zapfastie
    fi
    zapfastie --version
    /checks/check-runtime-libs
    test -s /usr/share/applications/zapfastie.desktop
    test -s /usr/share/icons/hicolor/scalable/apps/zapfastie.svg
    grep -qx "Icon=zapfastie" /usr/share/applications/zapfastie.desktop
    grep -qx "StartupWMClass=zapfastie" /usr/share/applications/zapfastie.desktop
    test -s /usr/share/zapfastie/omarchy/zapfastie.json.tpl
    test -x /usr/share/zapfastie/omarchy/zapfastie-theme
    if [ "$FORMAT" = deb ]; then apt-get remove -y zapfastie; else dnf remove -y zapfastie; fi
    test ! -e /usr/bin/zapfastie
    test ! -e /usr/share/applications/zapfastie.desktop
    test ! -e /usr/share/icons/hicolor/scalable/apps/zapfastie.svg
    test ! -e /usr/share/zapfastie/omarchy/zapfastie.json.tpl
    test ! -e /usr/share/zapfastie/omarchy/zapfastie-theme
    test "$(cat /root/.config/zapfastie/fixture)" = preserve-existing-settings
  '
