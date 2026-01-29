#!/bin/bash
# Downloads and bundles tools and extras from the original cyan repo

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
CYAN_BASE="https://raw.githubusercontent.com/asdfzxcvbn/pyzule-rw/main/cyan"

echo "[*] Downloading tools and extras from cyan repo..."

# Create directories
mkdir -p "$PROJECT_DIR/tools/Darwin/aarch64"
mkdir -p "$PROJECT_DIR/tools/Darwin/x86_64"
mkdir -p "$PROJECT_DIR/tools/Linux/aarch64"
mkdir -p "$PROJECT_DIR/tools/Linux/x86_64"
mkdir -p "$PROJECT_DIR/extras"

# Tools we need (we have native Rust replacements for otool and lipo)
TOOLS="ldid insert_dylib install_name_tool"

# Download Darwin arm64 tools
echo "[*] Downloading Darwin/arm64 tools..."
for tool in $TOOLS; do
    curl -sL "$CYAN_BASE/tools/Darwin/arm64/$tool" -o "$PROJECT_DIR/tools/Darwin/aarch64/$tool"
    chmod +x "$PROJECT_DIR/tools/Darwin/aarch64/$tool"
done

# Download Darwin x86_64 tools
echo "[*] Downloading Darwin/x86_64 tools..."
for tool in $TOOLS; do
    curl -sL "$CYAN_BASE/tools/Darwin/x86_64/$tool" -o "$PROJECT_DIR/tools/Darwin/x86_64/$tool"
    chmod +x "$PROJECT_DIR/tools/Darwin/x86_64/$tool"
done

# Download Linux aarch64 tools
echo "[*] Downloading Linux/aarch64 tools..."
for tool in $TOOLS; do
    curl -sL "$CYAN_BASE/tools/Linux/aarch64/$tool" -o "$PROJECT_DIR/tools/Linux/aarch64/$tool" 2>/dev/null || echo "  [?] $tool not available for Linux/aarch64"
    chmod +x "$PROJECT_DIR/tools/Linux/aarch64/$tool" 2>/dev/null || true
done

# Download Linux x86_64 tools
echo "[*] Downloading Linux/x86_64 tools..."
for tool in $TOOLS; do
    curl -sL "$CYAN_BASE/tools/Linux/x86_64/$tool" -o "$PROJECT_DIR/tools/Linux/x86_64/$tool" 2>/dev/null || echo "  [?] $tool not available for Linux/x86_64"
    chmod +x "$PROJECT_DIR/tools/Linux/x86_64/$tool" 2>/dev/null || true
done

# Download extras (frameworks)
FRAMEWORKS="CydiaSubstrate Orion Cephei CepheiUI CepheiPrefs"

echo "[*] Downloading extras (frameworks)..."
for fw in $FRAMEWORKS; do
    mkdir -p "$PROJECT_DIR/extras/${fw}.framework"
    
    # Download Info.plist
    curl -sL "$CYAN_BASE/extras/${fw}.framework/Info.plist" -o "$PROJECT_DIR/extras/${fw}.framework/Info.plist"
    
    # Download the binary
    curl -sL "$CYAN_BASE/extras/${fw}.framework/$fw" -o "$PROJECT_DIR/extras/${fw}.framework/$fw"
    chmod +x "$PROJECT_DIR/extras/${fw}.framework/$fw"
    
    # Download LICENSE if exists
    curl -sL "$CYAN_BASE/extras/${fw}.framework/LICENSE" -o "$PROJECT_DIR/extras/${fw}.framework/LICENSE" 2>/dev/null || true
    
    echo "  [+] ${fw}.framework"
done

echo "[*] Done! Bundled files are in:"
echo "    - $PROJECT_DIR/tools/"
echo "    - $PROJECT_DIR/extras/"
