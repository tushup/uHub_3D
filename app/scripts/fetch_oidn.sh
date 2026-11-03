#!/bin/sh
# Fetch the pinned Intel Open Image Denoise 2.3.3 prebuilt (D-32).
# Stages the CPU-only libraries Tauri bundles, and prints the
# OIDN_DIR value `cargo build` needs. On macOS the staged libraries
# are universal (x86_64 + arm64) binaries built with lipo.
set -eu

VERSION="2.3.3"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DOWNLOAD_ROOT="$REPO_ROOT/src-tauri/.oidn/$VERSION"
STAGE_DIR="$REPO_ROOT/src-tauri/oidn"
BASE_URL="https://github.com/OpenImageDenoise/oidn/releases/download/v$VERSION"

mkdir -p "$DOWNLOAD_ROOT" "$STAGE_DIR"

download() {
    # $1 = asset file name
    if [ ! -f "$DOWNLOAD_ROOT/$1" ]; then
        echo "Downloading OIDN $VERSION: $1"
        curl -fL "$BASE_URL/$1" -o "$DOWNLOAD_ROOT/$1"
    fi
    tar -xzf "$DOWNLOAD_ROOT/$1" -C "$DOWNLOAD_ROOT"
}

case "$(uname -s)" in
    Darwin)
        download "oidn-$VERSION.x86_64.macos.tar.gz"
        download "oidn-$VERSION.arm64.macos.tar.gz"
        X64_DIR="$DOWNLOAD_ROOT/oidn-$VERSION.x86_64.macos"
        ARM_DIR="$DOWNLOAD_ROOT/oidn-$VERSION.arm64.macos"
        UNIVERSAL_DIR="$DOWNLOAD_ROOT/oidn-$VERSION.universal.macos"
        mkdir -p "$UNIVERSAL_DIR/lib"
        cp -R "$X64_DIR/include" "$UNIVERSAL_DIR/include"
        # CPU device only (D-32): core, CPU device and TBB.
        for lib in "$X64_DIR"/lib/libOpenImageDenoise*.dylib "$X64_DIR"/lib/libtbb*.dylib; do
            name="$(basename "$lib")"
            case "$name" in
                *cuda*|*hip*|*sycl*|*metal*) continue ;;
            esac
            if [ -f "$ARM_DIR/lib/$name" ]; then
                lipo -create "$X64_DIR/lib/$name" "$ARM_DIR/lib/$name" -output "$UNIVERSAL_DIR/lib/$name"
            else
                cp "$X64_DIR/lib/$name" "$UNIVERSAL_DIR/lib/$name"
            fi
            cp "$UNIVERSAL_DIR/lib/$name" "$STAGE_DIR/$name"
        done
        OIDN_DIR="$UNIVERSAL_DIR"
        ;;
    Linux)
        download "oidn-$VERSION.x86_64.linux.tar.gz"
        EXTRACTED="$DOWNLOAD_ROOT/oidn-$VERSION.x86_64.linux"
        for lib in "$EXTRACTED"/lib/libOpenImageDenoise*.so* "$EXTRACTED"/lib/libtbb*.so*; do
            name="$(basename "$lib")"
            case "$name" in
                *cuda*|*hip*|*sycl*|*metal*) continue ;;
            esac
            cp "$lib" "$STAGE_DIR/$name"
        done
        OIDN_DIR="$EXTRACTED"
        ;;
    *)
        echo "Unsupported OS: $(uname -s). On Windows use scripts/fetch_oidn.ps1." >&2
        exit 1
        ;;
esac

echo "OIDN_DIR=$OIDN_DIR"
echo "Set it before building: export OIDN_DIR=\"$OIDN_DIR\""
if [ -n "${GITHUB_ENV:-}" ]; then
    echo "OIDN_DIR=$OIDN_DIR" >> "$GITHUB_ENV"
fi
