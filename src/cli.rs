use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "ruzule")]
#[command(author = "asdfzxcvbn")]
#[command(version, disable_version_flag = true)]
#[command(about = "ruzule - the best iOS app injector, rewritten in Rust")]
#[command(long_about = "A Rust rewrite of cyan/pyzule for modifying iOS apps.\n\nSupports injecting dylibs, frameworks, bundles, and app extensions.\nCan also modify app metadata, icons, and more.")]
pub struct Args {
    /// The app to be modified (.app/.ipa/.tipa)
    #[arg(short, long, required = true)]
    pub input: String,

    /// Output path (if unspecified, overwrites input)
    #[arg(short, long)]
    pub output: Option<String>,

    /// The .cyan file(s) to use
    #[arg(short = 'z', long = "cyan")]
    pub cyan: Option<Vec<PathBuf>>,

    /// Tweaks/items to inject or add to the bundle
    #[arg(short = 'f')]
    pub files: Option<Vec<PathBuf>>,

    /// Modify the app's name
    #[arg(short = 'n')]
    pub name: Option<String>,

    /// Modify the app's version
    #[arg(short = 'v')]
    pub version: Option<String>,

    /// Print version
    #[arg(long = "version", action = clap::ArgAction::Version)]
    _version: (),

    /// Modify the app's bundle ID
    #[arg(short = 'b')]
    pub bundle_id: Option<String>,

    /// Modify the app's minimum OS version
    #[arg(short = 'm')]
    pub minimum: Option<String>,

    /// Modify the app's icon
    #[arg(short = 'k')]
    pub icon: Option<PathBuf>,

    /// A plist to merge with the app's Info.plist
    #[arg(short = 'l')]
    pub merge_plist: Option<PathBuf>,

    /// Add or modify entitlements to the main binary
    #[arg(short = 'x')]
    pub entitlements: Option<PathBuf>,

    /// Remove UISupportedDevices
    #[arg(short = 'u', long = "remove-supported-devices")]
    pub remove_supported_devices: bool,

    /// Remove all watch apps
    #[arg(short = 'w', long = "no-watch")]
    pub no_watch: bool,

    /// Enable documents support
    #[arg(short = 'd', long = "enable-documents")]
    pub enable_documents: bool,

    /// Fakesign all binaries for use with AppSync/TrollStore
    #[arg(short = 's', long = "fakesign")]
    pub fakesign: bool,

    /// Thin all binaries to arm64 (may largely reduce size)
    #[arg(short = 'q', long = "thin")]
    pub thin: bool,

    /// Remove all app extensions
    #[arg(short = 'e', long = "remove-extensions")]
    pub remove_extensions: bool,

    /// Only remove encrypted app extensions
    #[arg(short = 'g', long = "remove-encrypted")]
    pub remove_encrypted: bool,

    /// Compression level of the IPA (0-9, default: 6)
    #[arg(short = 'c', long = "compress", default_value = "6", value_parser = clap::value_parser!(u8).range(0..=9))]
    pub compress: u8,

    /// Skip main binary encryption check
    #[arg(long = "ignore-encrypted")]
    pub ignore_encrypted: bool,

    /// Overwrite existing files without confirming
    #[arg(long = "overwrite")]
    pub overwrite: bool,
}

impl Args {
    pub fn get_output(&self) -> String {
        if let Some(ref output) = self.output {
            let output = output.to_string();
            if !output.ends_with(".app") && !output.ends_with(".ipa") && !output.ends_with(".tipa") {
                println!("[?] valid file extension not found; will create ipa");
                return format!("{}.ipa", output);
            }
            output
        } else {
            self.input.clone()
        }
    }
}
