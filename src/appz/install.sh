#!/bin/sh
set -eu

VERSION="${VERSION:-latest}"
AUTOINSTALL="${AUTOINSTALL:-false}"

echo "appz feature: installing appz (version: ${VERSION})"

if ! command -v curl >/dev/null 2>&1; then
  echo "Error: curl not found in this base image — appz's installer requires it."
  exit 1
fi

export APPZ_INSTALL_DIR=/usr/local/bin
export APPZ_QUIET_INSTALL=1
export APPZ_VERSION="$VERSION"

curl -fsSL https://raw.githubusercontent.com/getappz/cli/main/install.sh | sh -s -- --download

mkdir -p /usr/local/share
echo "$AUTOINSTALL" > /usr/local/share/appz-autoinstall

echo "appz feature: installed appz to ${APPZ_INSTALL_DIR}/appz (autoInstall=${AUTOINSTALL})"
