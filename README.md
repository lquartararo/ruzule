# ruzule

A cross-platform iOS IPA injector and modifier written in Rust. Spiritual successor to [pyzule](https://github.com/asdfzxcvbn/pyzule-rw).

## Features

- **Tweak Injection**: Inject .dylib files and frameworks from .deb packages
- **Bundled Frameworks**: Auto-injects CydiaSubstrate ([ElleKit](https://github.com/evelyneee/ellekit)), Orion, Cephei when needed
- **App Duplication**: Create duplicate apps with unique bundle IDs
- **Plist Modification**: Change app name, version, bundle ID
- **Icon Replacement**: Custom app icons
- **Cross-Platform**: Works on macOS, Linux, and Windows (no external tools required)

## Installation

### From Source

```bash
cargo install --git https://github.com/lquartararo/ruzule
```

### Pre-built Binaries

Download from [Releases](https://github.com/lquartararo/ruzule/releases).

## Usage

### Inject a tweak

```bash
ruzule -i app.ipa -o modified.ipa -f tweak.deb
```

### Inject with .cyan config

```bash
ruzule -i app.ipa -o modified.ipa -f config.cyan
```

### Generate a .cyan file

```bash
ruzule cgen -o config.cyan -f tweak.deb -n "New Name" -v "1.0.0"
```

### Duplicate an app

```bash
ruzule dupe -i app.ipa -o duplicate.ipa
```

### Recommended flags

For most use cases, consider using `-uwsgq`:

| Flag | Description |
|------|-------------|
| `-u` | Remove `UISupportedDevices` - prevents install failures on specific devices with AltStore/SideStore/Sideloadly |
| `-w` | Remove watch apps - saves space, most users can't use them without a paid dev account anyway |
| `-s` | Fakesign the app - required for AppSync/TrollStore users to prevent crashes on launch |
| `-g` | Remove encrypted extensions - saves space and app IDs, encrypted extensions are unusable anyway |
| `-q` | Thin binaries to arm64 - can reduce app size significantly (sometimes up to 2x) |
| `-p` | Patch plugins - fixes share sheet, widgets, VPNs, and other extension functionality |

Example with all recommended flags:
```bash
ruzule -i app.ipa -o modified.ipa -f tweak.deb -uwsgqp
```

### All options

```
ruzule [OPTIONS] -i <INPUT> [OUTPUT]

Options:
  -i, --input <INPUT>       Input IPA file
  -o, --output <OUTPUT>     Output IPA file
  -f, --files <FILES>       Files to inject (.dylib, .deb, .framework, .cyan)
  -n, --name <NAME>         New app display name
  -v, --version <VERSION>   New app version
  -b, --bundle-id <ID>      New bundle identifier
  -k, --icon <ICON>         New app icon (PNG)
  -u                        Remove UISupportedDevices
  -w                        Remove watch apps
  -s                        Fakesign all binaries
  -q                        Thin binaries to arm64
  -e                        Remove all app extensions
  -g                        Remove only encrypted extensions
  -d                        Enable documents support
  -p                        Patch plugins (fixes share sheet, widgets, VPNs)
  -c, --compress <0-9>      Compression level (default: 6)
      --use-frameworks-dir  Place dylibs in Frameworks/ with @rpath
      --overwrite           Overwrite output without prompting
  -h, --help                Print help
```

## Building

```bash
git clone https://github.com/lquartararo/ruzule
cd ruzule
cargo build --release
```

Binary will be at `target/release/ruzule`.

## Credits

- [pyzule](https://github.com/asdfzxcvbn/pyzule-rw) - Original Python implementation
- [ipapatch](https://github.com/asdfzxcvbn/ipapatch) / [zxPluginsInject](https://github.com/asdfzxcvbn/zxPluginsInject) - Plugin patching
- [ElleKit](https://github.com/evelyneee/ellekit) - Bundled as CydiaSubstrate for tweak compatibility
- [Impactor](https://github.com/khcrysalis/Impactor) - Mach-O manipulation techniques
- [apple-codesign](https://github.com/indygreg/apple-platform-rs) - Code signing

## License

[Unlicense](LICENSE)
