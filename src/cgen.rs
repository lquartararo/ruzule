use anyhow::{bail, Result};
use clap::Parser;
use serde_json::json;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Parser, Debug)]
#[command(name = "cgen")]
#[command(about = "Generate .cyan configuration files")]
pub struct CgenArgs {
    /// Output path for the .cyan file
    #[arg(short, long, required = true)]
    pub output: String,

    /// Tweaks/items to inject or add to the bundle
    #[arg(short = 'f')]
    pub files: Option<Vec<PathBuf>>,

    /// Modify the app's name
    #[arg(short = 'n')]
    pub name: Option<String>,

    /// Modify the app's version
    #[arg(short = 'v')]
    pub version: Option<String>,

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

    /// Fakesign all binaries
    #[arg(short = 's', long = "fakesign")]
    pub fakesign: bool,

    /// Thin all binaries to arm64
    #[arg(short = 'q', long = "thin")]
    pub thin: bool,

    /// Remove all app extensions
    #[arg(short = 'e', long = "remove-extensions")]
    pub remove_extensions: bool,

    /// Only remove encrypted app extensions
    #[arg(short = 'g', long = "remove-encrypted")]
    pub remove_encrypted: bool,

    /// Overwrite existing file without confirming
    #[arg(long = "overwrite")]
    pub overwrite: bool,
}

pub fn run(args: CgenArgs) -> Result<()> {
    // Validate inputs
    if let Some(ref m) = args.minimum {
        if m.chars().any(|c| !c.is_ascii_digit() && c != '.') {
            bail!("invalid minimum OS version: {}", m);
        }
    }
    if let Some(ref k) = args.icon {
        if !k.is_file() {
            bail!("{} does not exist", k.display());
        }
    }
    if let Some(ref l) = args.merge_plist {
        if !l.is_file() {
            bail!("{} does not exist", l.display());
        }
    }
    if let Some(ref x) = args.entitlements {
        if !x.is_file() {
            bail!("{} does not exist", x.display());
        }
    }
    if let Some(ref files) = args.files {
        let missing: Vec<_> = files.iter().filter(|f| !f.exists()).collect();
        if !missing.is_empty() {
            let names: Vec<_> = missing.iter().map(|p| p.display().to_string()).collect();
            bail!("the following file(s) do not exist: {}", names.join(", "));
        }
    }

    let mut output = args.output.clone();
    if !output.ends_with(".cyan") {
        println!("[*] appended cyan file extension to output");
        output.push_str(".cyan");
    }

    if PathBuf::from(&output).exists() && !args.overwrite {
        eprint!("[<] {} already exists. overwrite? [Y/n] ", output);
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim().to_lowercase();
        if trimmed != "y" && trimmed != "yes" && !trimmed.is_empty() {
            println!("[>] quitting.");
            return Ok(());
        }
    }

    // Build config JSON
    let mut config = serde_json::Map::new();
    
    if args.files.is_some() {
        config.insert("f".to_string(), json!(true));
    }
    if let Some(ref n) = args.name {
        config.insert("n".to_string(), json!(n));
    }
    if let Some(ref v) = args.version {
        config.insert("v".to_string(), json!(v));
    }
    if let Some(ref b) = args.bundle_id {
        config.insert("b".to_string(), json!(b));
    }
    if let Some(ref m) = args.minimum {
        config.insert("m".to_string(), json!(m));
    }
    if args.icon.is_some() {
        config.insert("k".to_string(), json!(true));
    }
    if args.merge_plist.is_some() {
        config.insert("l".to_string(), json!(true));
    }
    if args.entitlements.is_some() {
        config.insert("x".to_string(), json!(true));
    }
    if args.remove_supported_devices {
        config.insert("remove-supported-devices".to_string(), json!(true));
    }
    if args.no_watch {
        config.insert("no-watch".to_string(), json!(true));
    }
    if args.enable_documents {
        config.insert("enable-documents".to_string(), json!(true));
    }
    if args.fakesign {
        config.insert("fakesign".to_string(), json!(true));
    }
    if args.thin {
        config.insert("thin".to_string(), json!(true));
    }
    if args.remove_extensions {
        config.insert("remove-extensions".to_string(), json!(true));
    }
    if args.remove_encrypted {
        config.insert("remove-encrypted".to_string(), json!(true));
    }

    println!("[*] generating..");

    let file = File::create(&output)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(1));

    // Write config.json
    zip.start_file("config.json", options)?;
    zip.write_all(serde_json::to_string(&config)?.as_bytes())?;

    // Write inject files
    if let Some(ref files) = args.files {
        for f in files {
            if f.is_file() {
                let name = f.file_name().unwrap().to_string_lossy();
                zip.start_file(format!("inject/{}", name), options)?;
                zip.write_all(&fs::read(f)?)?;
            } else if f.is_dir() {
                let base = f.parent().unwrap_or(f);
                for entry in WalkDir::new(f).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() {
                        let rel = path.strip_prefix(base)?;
                        zip.start_file(format!("inject/{}", rel.display()), options)?;
                        zip.write_all(&fs::read(path)?)?;
                    }
                }
            }
        }
    }

    // Write icon
    if let Some(ref icon) = args.icon {
        zip.start_file("icon.idk", options)?;
        zip.write_all(&fs::read(icon)?)?;
    }

    // Write merge plist
    if let Some(ref plist) = args.merge_plist {
        zip.start_file("merge.plist", options)?;
        zip.write_all(&fs::read(plist)?)?;
    }

    // Write entitlements
    if let Some(ref ent) = args.entitlements {
        zip.start_file("new.entitlements", options)?;
        zip.write_all(&fs::read(ent)?)?;
    }

    zip.finish()?;
    println!("[*] generated {}", output);

    Ok(())
}
