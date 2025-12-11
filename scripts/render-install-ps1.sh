#!/usr/bin/env bash
set -euxo pipefail

# shellcheck disable=SC2016
APPZ_CURRENT_VERSION=$APPZ_VERSION \
	APPZ_CHECKSUM_WINDOWS_X64=$(grep "appz-.*windows-x64.zip" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_WINDOWS_ARM64=$(grep "appz-.*windows-arm64.zip" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	envsubst '$APPZ_CURRENT_VERSION,$APPZ_CHECKSUM_WINDOWS_X64,$APPZ_CHECKSUM_WINDOWS_ARM64' \
	<"$BASE_DIR/packaging/standalone/install.ps1.envsubst"

