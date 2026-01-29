#!/bin/bash
# Build and package ruzule releases for all platforms

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
VERSION=$(grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')

echo "[*] Building ruzule v$VERSION"

# Ensure tools and extras are bundled
if [ ! -d "$PROJECT_DIR/tools/Darwin" ] || [ ! -d "$PROJECT_DIR/extras" ]; then
    echo "[*] Running bundle script first..."
    "$SCRIPT_DIR/bundle.sh"
fi

mkdir -p "$PROJECT_DIR/releases"

# Build for current platform
echo "[*] Building for current platform..."
cargo build --release --manifest-path "$PROJECT_DIR/Cargo.toml"

# Determine current platform
SYSTEM=$(uname -s)
MACHINE=$(uname -m)

if [ "$MACHINE" = "arm64" ]; then
    MACHINE="aarch64"
fi

PLATFORM="${SYSTEM}_${MACHINE}"
RELEASE_DIR="$PROJECT_DIR/releases/ruzule-v${VERSION}-${PLATFORM}"

echo "[*] Packaging release for $PLATFORM..."

rm -rf "$RELEASE_DIR"
mkdir -p "$RELEASE_DIR"

# Copy binary
cp "$PROJECT_DIR/target/release/ruzule" "$RELEASE_DIR/"

# Create cgen symlink
ln -sf ruzule "$RELEASE_DIR/cgen"

# Copy platform-specific tools (preserve directory structure)
mkdir -p "$RELEASE_DIR/tools/$SYSTEM/$MACHINE"
if [ -d "$PROJECT_DIR/tools/$SYSTEM/$MACHINE" ]; then
    cp -r "$PROJECT_DIR/tools/$SYSTEM/$MACHINE"/* "$RELEASE_DIR/tools/$SYSTEM/$MACHINE/"
fi

# Copy extras (universal)
cp -r "$PROJECT_DIR/extras" "$RELEASE_DIR/"

# Create archive
cd "$PROJECT_DIR/releases"
tar -czvf "ruzule-v${VERSION}-${PLATFORM}.tar.gz" "ruzule-v${VERSION}-${PLATFORM}"

echo "[*] Release created: releases/ruzule-v${VERSION}-${PLATFORM}.tar.gz"

# Show contents
echo ""
echo "[*] Release contents:"
tar -tvf "ruzule-v${VERSION}-${PLATFORM}.tar.gz"
