#!/usr/bin/env bash
set -euxo pipefail

# shellcheck disable=SC2016
APPZ_CURRENT_VERSION=$APPZ_VERSION \
	APPZ_CHECKSUM_LINUX_X86_64=$(grep "appz-.*linux-x64.tar.gz" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_LINUX_X86_64_MUSL=$(grep "appz-.*linux-x64-musl.tar.gz" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_LINUX_ARM64=$(grep "appz-.*linux-arm64.tar.gz" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_LINUX_ARM64_MUSL=$(grep "appz-.*linux-arm64-musl.tar.gz" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_LINUX_ARMV7=$(grep "appz-.*linux-armv7.tar.gz" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_LINUX_ARMV7_MUSL=$(grep "appz-.*linux-armv7-musl.tar.gz" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_MACOS_X86_64=$(grep "appz-.*macos-x64.tar.gz" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_MACOS_ARM64=$(grep "appz-.*macos-arm64.tar.gz" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_LINUX_X86_64_ZSTD=$(grep "appz-.*linux-x64.tar.zst" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_LINUX_X86_64_MUSL_ZSTD=$(grep "appz-.*linux-x64-musl.tar.zst" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_LINUX_ARM64_ZSTD=$(grep "appz-.*linux-arm64.tar.zst" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_LINUX_ARM64_MUSL_ZSTD=$(grep "appz-.*linux-arm64-musl.tar.zst" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_LINUX_ARMV7_ZSTD=$(grep "appz-.*linux-armv7.tar.zst" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_LINUX_ARMV7_MUSL_ZSTD=$(grep "appz-.*linux-armv7-musl.tar.zst" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_MACOS_X86_64_ZSTD=$(grep "appz-.*macos-x64.tar.zst" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	APPZ_CHECKSUM_MACOS_ARM64_ZSTD=$(grep "appz-.*macos-arm64.tar.zst" "$RELEASE_DIR/$APPZ_VERSION/SHASUMS256.txt" | awk '{print $1}') \
	envsubst '$APPZ_CURRENT_VERSION,$APPZ_CHECKSUM_LINUX_X86_64,$APPZ_CHECKSUM_LINUX_X86_64_MUSL,$APPZ_CHECKSUM_LINUX_ARM64,$APPZ_CHECKSUM_LINUX_ARM64_MUSL,$APPZ_CHECKSUM_LINUX_ARMV7,$APPZ_CHECKSUM_LINUX_ARMV7_MUSL,$APPZ_CHECKSUM_MACOS_X86_64,$APPZ_CHECKSUM_MACOS_ARM64,$APPZ_CHECKSUM_LINUX_X86_64_ZSTD,$APPZ_CHECKSUM_LINUX_X86_64_MUSL_ZSTD,$APPZ_CHECKSUM_LINUX_ARM64_ZSTD,$APPZ_CHECKSUM_LINUX_ARM64_MUSL_ZSTD,$APPZ_CHECKSUM_LINUX_ARMV7_ZSTD,$APPZ_CHECKSUM_LINUX_ARMV7_MUSL_ZSTD,$APPZ_CHECKSUM_MACOS_X86_64_ZSTD,$APPZ_CHECKSUM_MACOS_ARM64_ZSTD' \
	<"$BASE_DIR/packaging/standalone/install.envsubst"

