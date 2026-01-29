# ruzule

A Rust rewrite of [pyzule-rw/cyan](https://github.com/asdfzxcvbn/pyzule-rw) - the best iOS app injector (and more)!

## Features

- Inject deb, dylib, framework, bundle, and appex files
- Automatically fix dependencies on CydiaSubstrate/ElleKit, Cephei, and Orion
- Copy unknown file/folder types to app root
- Change app name, version, bundle ID, and minimum OS version
- Remove UISupportedDevices
- Remove watch app
- Change app icon
- Fakesign output IPA/TIPA/app
- Merge plists into Info.plist
- Add custom entitlements to main executable
- Thin all binaries to arm64 (native Rust implementation)
- Remove app extensions (all or just encrypted ones)
- Support for .cyan config files
- Generate .cyan config files with `cgen` command

## Installation

### Pre-built Releases

Download the latest release for your platform from the [Releases](https://github.com/asdfzxcvbn/ruzule/releases) page. Each release includes all required tools and frameworks.

### From Source

```bash
# Clone the repository
git clone https://github.com/asdfzxcvbn/ruzule
cd ruzule

# Download bundled tools and frameworks
./scripts/bundle.sh

# Build
cargo build --release

# Binary will be at ./target/release/ruzule
```

## Usage

```bash
# Basic usage - inject a dylib
ruzule -i app.ipa -o modified.ipa -f tweak.dylib

# Inject multiple files
ruzule -i app.ipa -f tweak.dylib -f AnotherTweak.framework -f plugin.appex

# Change app name and bundle ID
ruzule -i app.ipa -n "New Name" -b "com.new.bundle"

# Fakesign for TrollStore/AppSync
ruzule -i app.ipa -o signed.ipa -s

# Thin binaries to reduce size
ruzule -i app.ipa -o thin.ipa -q

# Use a .cyan config file
ruzule -i app.ipa -z config.cyan

# Multiple options combined
ruzule -i app.ipa -o out.ipa -f tweak.dylib -n "My App" -s -w -q
```

### Options

| Flag | Long | Description |
|------|------|-------------|
| `-i` | `--input` | Input app (.app/.ipa/.tipa) **required** |
| `-o` | `--output` | Output path (overwrites input if unspecified) |
| `-f` | | Files to inject (dylib, deb, framework, appex, bundle) |
| `-z` | `--cyan` | .cyan config file(s) to apply |
| `-n` | | New app display name |
| `-v` | | New app version |
| `-b` | | New bundle identifier |
| `-m` | | Minimum iOS version |
| `-k` | | New app icon (PNG) |
| `-l` | | Plist to merge with Info.plist |
| `-x` | | Entitlements file to apply |
| `-u` | `--remove-supported-devices` | Remove UISupportedDevices |
| `-w` | `--no-watch` | Remove watch app |
| `-d` | `--enable-documents` | Enable documents support |
| `-s` | `--fakesign` | Fakesign all binaries |
| `-q` | `--thin` | Thin binaries to arm64 |
| `-e` | `--remove-extensions` | Remove all app extensions |
| `-g` | `--remove-encrypted` | Remove only encrypted extensions |
| `-c` | `--compress` | Compression level 0-9 (default: 6) |
| | `--ignore-encrypted` | Skip encryption check |
| | `--overwrite` | Overwrite without confirming |
| | `--version` | Print version |

## Generating .cyan Files

Use the `cgen` command to create reusable .cyan configuration files:

```bash
# Via subcommand
ruzule cgen -o config.cyan -f tweak.dylib -s -q

# Via symlink (if installed)
cgen -o config.cyan -f tweak.dylib -n "Tweaked App" -s
```

The generated .cyan file can then be applied to any app:

```bash
ruzule -i app.ipa -z config.cyan
```

## Bundled Dependencies

Releases include all required tools and frameworks:

### Tools (platform-specific)
- `ldid` - Code signing
- `insert_dylib` - Dylib injection
- `install_name_tool` - Dependency path modification

### Frameworks (in `extras/`)
- `CydiaSubstrate.framework` (ElleKit)
- `Orion.framework`
- `Cephei.framework`
- `CepheiUI.framework`
- `CepheiPrefs.framework`

## Native Implementations

Unlike the original Python version, ruzule implements several operations natively in Rust:

- **Binary thinning** - Extracts arm64 slice from fat binaries using the `goblin` crate
- **Dependency inspection** - Parses Mach-O LC_LOAD_DYLIB commands directly
- **Encryption detection** - Checks LC_ENCRYPTION_INFO without external tools

This reduces external dependencies and improves performance.

## Building Releases

To build a release package with all dependencies:

```bash
# First, download tools and frameworks
./scripts/bundle.sh

# Build and package for current platform
./scripts/release.sh
```

The release tarball will be created in the `releases/` directory.

## Acknowledgements

- [asdfzxcvbn](https://github.com/asdfzxcvbn/pyzule-rw) for the original pyzule-rw/cyan


