#!/usr/bin/env bash
set -euo pipefail

error() {
	echo "$@" >&2
	exit 1
}

RUST_TRIPLE=${1:-$(rustc -vV | grep ^host: | cut -d ' ' -f2)}
#region os/arch
get_os() {
	case "$RUST_TRIPLE" in
	*-apple-darwin*)
		echo "macos"
		;;
	*-windows-*)
		echo "windows"
		;;
	*-linux-*)
		echo "linux"
		;;
	*)
		error "unsupported OS: $RUST_TRIPLE"
		;;
	esac
}

get_arch() {
	case "$RUST_TRIPLE" in
	aarch64-*)
		echo "arm64"
		;;
	arm*)
		echo "armv7"
		;;
	x86_64-*)
		echo "x64"
		;;
	*)
		error "unsupported arch: $RUST_TRIPLE"
		;;
	esac
}
get_suffix() {
	case "$RUST_TRIPLE" in
	*-musl | *-musleabi | *-musleabihf)
		echo "-musl"
		;;
	*)
		echo ""
		;;
	esac
}
#endregion

set -x
os=$(get_os)
arch=$(get_arch)
suffix=$(get_suffix)
version=$(./scripts/get-version.sh | sed 's/^v//')
basename=appz-$version-$os-$arch$suffix

case "$os-$arch" in
linux-arm*)
	# don't use sccache
	unset RUSTC_WRAPPER
	;;
esac

if [[ $os == "linux" ]]; then
	cross build --release --target "$RUST_TRIPLE" --bin appz
else
	cargo build --release --target "$RUST_TRIPLE" --bin appz
fi

mkdir -p dist/appz/bin
target_dir="${CARGO_TARGET_DIR:-target}"
if [[ $os == "windows" ]]; then
    cp "$target_dir/$RUST_TRIPLE/release/appz.exe" dist/appz/bin/appz.exe 2>/dev/null || true
else
    cp "$target_dir/$RUST_TRIPLE/release/appz" dist/appz/bin/appz 2>/dev/null || true
    chmod +x dist/appz/bin/appz
    # Compress binary with UPX (skip for ARM architectures if it fails)
    if command -v upx >/dev/null 2>&1; then
        if [[ "$arch" == "arm64" || "$arch" == "armv7" ]]; then
            # Try UPX compression for ARM, but don't fail if it doesn't work
            upx --best dist/appz/bin/appz 2>/dev/null || echo "UPX compression skipped for ARM architecture"
        else
            # For non-ARM architectures, compress with UPX (non-blocking)
            upx --best dist/appz/bin/appz || true
        fi
    else
        echo "UPX not found, skipping compression"
    fi
fi

if [[ -f README.md ]]; then
	cp README.md dist/appz/README.md
fi
if [[ -f LICENSE ]]; then
	cp LICENSE dist/appz/LICENSE
fi

cd dist

if [[ $os == "windows" ]]; then
	zip -r "$basename.zip" appz
	ls -oh "$basename.zip"
else
	tar -cf - appz | gzip -9 >"$basename.tar.gz"
	tar -acf "$basename.tar.xz" appz
	ls -oh "$basename.tar."*
fi

