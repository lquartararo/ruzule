use clap::{Parser, Subcommand};
use ruzule::{
    parse_cyan, AppBundle, CyanConfig, Result, RuzuleError,
    copy_app, create_ipa, extract_ipa,
};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

#[derive(Parser, Debug)]
#[command(name = "ruzule")]
#[command(about = "iOS app injector and modifier - Rust rewrite of pyzule-rw/cyan")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    // Default inject command args (when no subcommand is specified)
    /// The app to be modified (.app/.ipa/.tipa)
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Output path (if unspecified, overwrites input)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// The .cyan file(s) to use
    #[arg(short = 'z', long = "cyan")]
    cyan: Option<Vec<PathBuf>>,

    /// Tweaks/files to inject
    #[arg(short = 'f')]
    files: Option<Vec<PathBuf>>,

    /// Modify the app's name
    #[arg(short = 'n')]
    name: Option<String>,

    /// Modify the app's version
    #[arg(short = 'v')]
    version: Option<String>,

    /// Modify the app's bundle id
    #[arg(short = 'b')]
    bundle_id: Option<String>,

    /// Modify the app's minimum OS version
    #[arg(short = 'm')]
    minimum: Option<String>,

    /// Modify the app's icon
    #[arg(short = 'k')]
    icon: Option<PathBuf>,

    /// A plist to merge with the app's Info.plist
    #[arg(short = 'l')]
    plist: Option<PathBuf>,

    /// Add or modify entitlements to the main binary
    #[arg(short = 'x')]
    entitlements: Option<PathBuf>,

    /// Remove UISupportedDevices
    #[arg(short = 'u', long)]
    remove_supported_devices: bool,

    /// Remove all watch apps
    #[arg(short = 'w', long)]
    no_watch: bool,

    /// Enable documents support
    #[arg(short = 'd', long)]
    enable_documents: bool,

    /// Fakesign all binaries for use with appsync/trollstore
    #[arg(short = 's', long)]
    fakesign: bool,

    /// Thin all binaries to arm64
    #[arg(short = 'q', long)]
    thin: bool,

    /// Remove all app extensions
    #[arg(short = 'e', long)]
    remove_extensions: bool,

    /// Only remove encrypted app extensions
    #[arg(short = 'g', long)]
    remove_encrypted: bool,

    /// The compression level of the ipa (0-9, defaults to 6)
    #[arg(short = 'c', long, default_value = "6", value_parser = clap::value_parser!(u32).range(0..=9))]
    compress: u32,

    /// Skip main binary encryption check
    #[arg(long)]
    ignore_encrypted: bool,

    /// Overwrite existing files without confirming
    #[arg(long)]
    overwrite: bool,

    /// Place dylibs in Frameworks/ with @rpath instead of app root with @executable_path
    #[arg(long)]
    use_frameworks_dir: bool,

    /// Patch plugins to fix share sheet, widgets, VPNs, etc.
    #[arg(short = 'p', long)]
    patch_plugins: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate a .cyan configuration file
    Cgen {
        /// Output path for the .cyan file
        #[arg(short, long, required = true)]
        output: PathBuf,

        /// Tweaks/files to inject
        #[arg(short = 'f')]
        files: Option<Vec<PathBuf>>,

        /// Modify the app's name
        #[arg(short = 'n')]
        name: Option<String>,

        /// Modify the app's version
        #[arg(short = 'v')]
        version: Option<String>,

        /// Modify the app's bundle id
        #[arg(short = 'b')]
        bundle_id: Option<String>,

        /// Modify the app's minimum OS version
        #[arg(short = 'm')]
        minimum: Option<String>,

        /// Modify the app's icon
        #[arg(short = 'k')]
        icon: Option<PathBuf>,

        /// A plist to merge with the app's Info.plist
        #[arg(short = 'l')]
        plist: Option<PathBuf>,

        /// Add or modify entitlements to the main binary
        #[arg(short = 'x')]
        entitlements: Option<PathBuf>,

        /// Remove UISupportedDevices
        #[arg(short = 'u', long)]
        remove_supported_devices: bool,

        /// Remove all watch apps
        #[arg(short = 'w', long)]
        no_watch: bool,

        /// Enable documents support
        #[arg(short = 'd', long)]
        enable_documents: bool,

        /// Fakesign all binaries for use with appsync/trollstore
        #[arg(short = 's', long)]
        fakesign: bool,

        /// Thin all binaries to arm64
        #[arg(short = 'q', long)]
        thin: bool,

        /// Remove all app extensions
        #[arg(short = 'e', long)]
        remove_extensions: bool,

        /// Only remove encrypted app extensions
        #[arg(short = 'g', long)]
        remove_encrypted: bool,

        /// Patch plugins to fix share sheet, widgets, VPNs, etc.
        #[arg(short = 'p', long)]
        patch_plugins: bool,

        /// Overwrite existing files without confirming
        #[arg(long)]
        overwrite: bool,
    },

    /// Duplicate an app with a new bundle ID (allows installing multiple copies)
    Dupe {
        /// Input IPA to duplicate
        #[arg(short, long, required = true)]
        input: PathBuf,

        /// Output path for the duplicated IPA
        #[arg(short, long, required = true)]
        output: PathBuf,

        /// A seed to derive the team ID from (any string, save it for related apps)
        #[arg(short, long)]
        seed: Option<String>,

        /// Bundle suffix to use (10 hex chars, see README)
        #[arg(short, long)]
        bundle: Option<String>,

        /// Overwrite existing files without confirming
        #[arg(long)]
        overwrite: bool,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("[!] {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Cgen {
            output,
            files,
            name,
            version,
            bundle_id,
            minimum,
            icon,
            plist,
            entitlements,
            remove_supported_devices,
            no_watch,
            enable_documents,
            fakesign,
            thin,
            remove_extensions,
            remove_encrypted,
            patch_plugins,
            overwrite,
        }) => {
            run_cgen(
                output,
                files,
                name,
                version,
                bundle_id,
                minimum,
                icon,
                plist,
                entitlements,
                remove_supported_devices,
                no_watch,
                enable_documents,
                fakesign,
                thin,
                remove_extensions,
                remove_encrypted,
                patch_plugins,
                overwrite,
            )
        }
        Some(Commands::Dupe {
            input,
            output,
            seed,
            bundle,
            overwrite,
        }) => {
            run_dupe(input, output, seed, bundle, overwrite)
        }
        None => {
            // Default inject behavior
            let input = cli.input.ok_or_else(|| {
                RuzuleError::InvalidInput("Input is required".to_string())
            })?;
            run_inject(
                input,
                cli.output,
                cli.cyan,
                cli.files,
                cli.name,
                cli.version,
                cli.bundle_id,
                cli.minimum,
                cli.icon,
                cli.plist,
                cli.entitlements,
                cli.remove_supported_devices,
                cli.no_watch,
                cli.enable_documents,
                cli.fakesign,
                cli.thin,
                cli.remove_extensions,
                cli.remove_encrypted,
                cli.compress,
                cli.ignore_encrypted,
                cli.overwrite,
                cli.use_frameworks_dir,
                cli.patch_plugins,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_cgen(
    mut output: PathBuf,
    files: Option<Vec<PathBuf>>,
    name: Option<String>,
    version: Option<String>,
    bundle_id: Option<String>,
    minimum: Option<String>,
    icon: Option<PathBuf>,
    plist: Option<PathBuf>,
    entitlements: Option<PathBuf>,
    remove_supported_devices: bool,
    no_watch: bool,
    enable_documents: bool,
    fakesign: bool,
    thin: bool,
    remove_extensions: bool,
    remove_encrypted: bool,
    patch_plugins: bool,
    overwrite: bool,
) -> Result<()> {
    // Validate inputs
    if let Some(ref m) = minimum {
        if !m.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return Err(RuzuleError::InvalidInput(format!(
                "Invalid minimum OS version: {}", m
            )));
        }
    }

    if let Some(ref k) = icon {
        if !k.is_file() {
            return Err(RuzuleError::FileNotFound(k.clone()));
        }
    }

    if let Some(ref l) = plist {
        if !l.is_file() {
            return Err(RuzuleError::FileNotFound(l.clone()));
        }
    }

    if let Some(ref x) = entitlements {
        if !x.is_file() {
            return Err(RuzuleError::FileNotFound(x.clone()));
        }
    }

    if let Some(ref files) = files {
        for f in files {
            if !f.exists() {
                return Err(RuzuleError::FileNotFound(f.clone()));
            }
        }
    }

    // Ensure .cyan extension
    if output.extension().map(|e| e != "cyan").unwrap_or(true) {
        println!("[?] appended .cyan extension to output");
        output = output.with_extension("cyan");
    }

    // Check if output exists
    if output.exists() && !overwrite {
        print!("[<] {} already exists. overwrite? [Y/n] ", output.display());
        std::io::stdout().flush()?;

        let mut response = String::new();
        std::io::stdin().read_line(&mut response)?;
        let response = response.trim().to_lowercase();

        if !matches!(response.as_str(), "y" | "yes" | "") {
            println!("[>] quitting.");
            return Ok(());
        }
    }

    // Build config
    let config = CyanConfig {
        f: files.is_some(),
        n: name,
        v: version,
        b: bundle_id,
        m: minimum,
        k: icon.is_some(),
        l: plist.is_some(),
        x: entitlements.is_some(),
        remove_supported_devices,
        no_watch,
        enable_documents,
        fakesign,
        thin,
        remove_extensions,
        remove_encrypted,
        patch_plugins,
    };

    println!("[*] generating...");

    let file = File::create(&output)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(1));

    // Write config.json
    let config_json = serde_json::to_string(&config)?;
    zip.start_file("config.json", options)?;
    zip.write_all(config_json.as_bytes())?;

    // Add files to inject
    if let Some(ref files) = files {
        for f in files {
            if f.is_file() {
                let name = f.file_name().unwrap().to_string_lossy();
                zip.start_file(format!("inject/{}", name), options)?;
                zip.write_all(&fs::read(f)?)?;
            } else if f.is_dir() {
                add_dir_to_zip(&mut zip, f, "inject", &options)?;
            }
        }
    }

    // Add icon
    if let Some(ref icon) = icon {
        zip.start_file("icon.idk", options)?;
        zip.write_all(&fs::read(icon)?)?;
    }

    // Add plist
    if let Some(ref plist) = plist {
        zip.start_file("merge.plist", options)?;
        zip.write_all(&fs::read(plist)?)?;
    }

    // Add entitlements
    if let Some(ref entitlements) = entitlements {
        zip.start_file("new.entitlements", options)?;
        zip.write_all(&fs::read(entitlements)?)?;
    }

    zip.finish()?;
    println!("[*] generated {}", output.display());

    Ok(())
}

fn add_dir_to_zip<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    dir: &PathBuf,
    base: &str,
    options: &SimpleFileOptions,
) -> Result<()> {
    let dir_name = dir.file_name().unwrap().to_string_lossy();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let rel_path = format!("{}/{}/{}", base, dir_name, path.file_name().unwrap().to_string_lossy());

        if path.is_file() {
            zip.start_file(&rel_path, *options)?;
            zip.write_all(&fs::read(&path)?)?;
        } else if path.is_dir() {
            add_dir_to_zip(zip, &path, &format!("{}/{}", base, dir_name), options)?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_inject(
    input: PathBuf,
    output: Option<PathBuf>,
    cyan: Option<Vec<PathBuf>>,
    mut files: Option<Vec<PathBuf>>,
    mut name: Option<String>,
    mut version: Option<String>,
    mut bundle_id: Option<String>,
    mut minimum: Option<String>,
    mut icon: Option<PathBuf>,
    mut plist: Option<PathBuf>,
    mut entitlements: Option<PathBuf>,
    mut remove_supported_devices: bool,
    mut no_watch: bool,
    mut enable_documents: bool,
    mut fakesign: bool,
    mut thin: bool,
    mut remove_extensions: bool,
    mut remove_encrypted: bool,
    compress: u32,
    ignore_encrypted: bool,
    overwrite: bool,
    use_frameworks_dir: bool,
    mut patch_plugins: bool,
) -> Result<()> {
    // Validate input
    let input_ext = input
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());

    if !matches!(input_ext.as_deref(), Some("app") | Some("ipa") | Some("tipa")) {
        return Err(RuzuleError::InvalidInput(
            "Input must be an .ipa, .tipa, or .app".to_string(),
        ));
    }

    if !input.exists() {
        return Err(RuzuleError::FileNotFound(input));
    }

    // Determine output
    let output = output.unwrap_or_else(|| input.clone());
    let output_ext = output
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());

    let output = if !matches!(output_ext.as_deref(), Some("app") | Some("ipa") | Some("tipa")) {
        println!("[?] valid file extension not found; will create ipa");
        output.with_extension("ipa")
    } else {
        output
    };

    // Check if output exists
    if output.exists() && !overwrite {
        let msg = if output != input {
            format!("{} already exists, overwrite it? [Y/n] ", output.display())
        } else {
            "no output was specified. overwrite the input? [Y/n] ".to_string()
        };

        print!("[<] {}", msg);
        std::io::stdout().flush()?;

        let mut response = String::new();
        std::io::stdin().read_line(&mut response)?;
        let response = response.trim().to_lowercase();

        if !matches!(response.as_str(), "y" | "yes" | "") {
            println!("[>] quitting.");
            return Ok(());
        }
    }

    // Validate other inputs
    if let Some(ref files) = files {
        for f in files {
            if !f.exists() {
                return Err(RuzuleError::FileNotFound(f.clone()));
            }
        }
    }

    if let Some(ref m) = minimum {
        if !m.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return Err(RuzuleError::InvalidInput(format!(
                "Invalid OS version: {}",
                m
            )));
        }
    }

    if let Some(ref k) = icon {
        if !k.is_file() {
            return Err(RuzuleError::FileNotFound(k.clone()));
        }
    }

    if let Some(ref l) = plist {
        if !l.is_file() {
            return Err(RuzuleError::FileNotFound(l.clone()));
        }
    }

    if let Some(ref cyans) = cyan {
        for c in cyans {
            if !c.is_file() {
                return Err(RuzuleError::FileNotFound(c.clone()));
            }
        }
    }

    if let Some(ref x) = entitlements {
        if !x.is_file() {
            return Err(RuzuleError::FileNotFound(x.clone()));
        }
    }

    let input_is_ipa = matches!(input_ext.as_deref(), Some("ipa") | Some("tipa"));
    let output_is_ipa = output
        .extension()
        .map(|e| {
            let e = e.to_string_lossy().to_lowercase();
            e == "ipa" || e == "tipa"
        })
        .unwrap_or(false);

    // Create temp directory
    let tmpdir = TempDir::new()?;
    let tmpdir_path = tmpdir.path();

    // Extract or copy app
    println!("[*] extracting...");
    let app_path = if input_is_ipa {
        extract_ipa(&input, tmpdir_path)?
    } else {
        copy_app(&input, tmpdir_path)?
    };
    println!("[*] extracted");

    // Load app bundle
    let mut app = AppBundle::new(&app_path)?;

    // Check encryption
    if app.executable.is_encrypted()? {
        if ignore_encrypted {
            println!("[?] main binary is encrypted, ignoring");
        } else {
            return Err(RuzuleError::EncryptedBinary(app.executable.inner.path.clone()));
        }
    }

    // Parse .cyan files
    if let Some(ref cyans) = cyan {
        for (index, cyan_path) in cyans.iter().enumerate() {
            let parsed = parse_cyan(cyan_path, tmpdir_path, index)?;

            // Merge config into args
            if let Some(n) = parsed.config.n {
                name = Some(n);
            }
            if let Some(v) = parsed.config.v {
                version = Some(v);
            }
            if let Some(b) = parsed.config.b {
                bundle_id = Some(b);
            }
            if let Some(m) = parsed.config.m {
                minimum = Some(m);
            }
            if parsed.config.remove_supported_devices {
                remove_supported_devices = true;
            }
            if parsed.config.no_watch {
                no_watch = true;
            }
            if parsed.config.enable_documents {
                enable_documents = true;
            }
            if parsed.config.fakesign {
                fakesign = true;
            }
            if parsed.config.thin {
                thin = true;
            }
            if parsed.config.remove_extensions {
                remove_extensions = true;
            }
            if parsed.config.remove_encrypted {
                remove_encrypted = true;
            }
            if parsed.config.patch_plugins {
                patch_plugins = true;
            }

            // Merge files
            if !parsed.files.is_empty() {
                let file_list = files.get_or_insert_with(Vec::new);
                for (_, path) in parsed.files {
                    file_list.push(path);
                }
            }

            if let Some(i) = parsed.icon {
                icon = Some(i);
            }
            if let Some(p) = parsed.plist {
                plist = Some(p);
            }
            if let Some(e) = parsed.entitlements {
                entitlements = Some(e);
            }
        }
    }

    // Process extensions removal (before injection)
    if remove_extensions {
        app.remove_all_extensions();
    } else if remove_encrypted {
        app.remove_encrypted_extensions()?;
    }

    // Inject files
    if let Some(ref file_list) = files {
        let mut tweaks: HashMap<String, PathBuf> = HashMap::new();
        for f in file_list {
            let file_name = f.file_name().unwrap().to_string_lossy().to_string();
            tweaks.insert(file_name, f.clone());
        }
        app.inject(&mut tweaks, tmpdir_path, use_frameworks_dir)?;
    }

    // Apply modifications
    if let Some(ref n) = name {
        app.plist.change_name(n);
    }
    if let Some(ref v) = version {
        app.plist.change_version(v);
    }
    if let Some(ref b) = bundle_id {
        app.plist.change_bundle_id(b);
    }
    if let Some(ref m) = minimum {
        app.plist.change_minimum_version(m);
    }
    if let Some(ref i) = icon {
        app.change_icon(i, tmpdir_path)?;
    }
    if let Some(ref p) = plist {
        app.plist.merge_plist(p)?;
    }
    if let Some(ref e) = entitlements {
        app.executable.merge_entitlements(e)?;
    }

    if remove_supported_devices {
        app.plist.remove_uisd();
    }
    if no_watch {
        app.remove_watch_apps();
    }
    if enable_documents {
        app.plist.enable_documents();
    }
    if patch_plugins {
        app.patch_plugins()?;
    }
    if fakesign {
        app.fakesign_all()?;
    }
    if thin {
        app.thin_all()?;
    }

    // Create output directories if needed
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    // Generate output
    println!("[*] generating...");
    if output_is_ipa {
        create_ipa(tmpdir_path, &output, compress)?;
    } else {
        if output.exists() {
            fs::remove_dir_all(&output)?;
        }
        fs::rename(&app_path, &output)?;
    }
    println!("[*] done: {}", output.display());

    Ok(())
}

fn run_dupe(
    input: PathBuf,
    mut output: PathBuf,
    seed: Option<String>,
    bundle: Option<String>,
    overwrite: bool,
) -> Result<()> {
    // Validate input
    if !input.exists() {
        return Err(RuzuleError::FileNotFound(input));
    }

    let input_ext = input
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());

    if !matches!(input_ext.as_deref(), Some("ipa") | Some("tipa")) {
        return Err(RuzuleError::InvalidInput(
            "Input must be an .ipa or .tipa".to_string(),
        ));
    }

    // Ensure output has .ipa extension
    if !output.to_string_lossy().ends_with(".ipa") {
        println!("[?] ipa file extension not detected, appending manually");
        output = output.with_extension("ipa");
    }

    // Check if output exists
    if output.exists() && !overwrite {
        print!("[<] {} already exists. overwrite? [Y/n] ", output.display());
        std::io::stdout().flush()?;

        let mut response = String::new();
        std::io::stdin().read_line(&mut response)?;
        let response = response.trim().to_lowercase();

        if !matches!(response.as_str(), "y" | "yes" | "") {
            println!("[>] quitting.");
            return Ok(());
        }
    }

    // Validate bundle suffix if provided
    if let Some(ref b) = bundle {
        if b.len() != 10 {
            return Err(RuzuleError::InvalidInput(
                "-b argument has invalid length (must be 10 hex chars)".to_string(),
            ));
        }
        if !b.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(RuzuleError::InvalidInput(
                "-b argument is invalid (must be hex chars only)".to_string(),
            ));
        }
    }

    // Generate or use provided seed
    let seed = seed.unwrap_or_else(|| Uuid::new_v4().to_string());

    // Derive team ID from seed (last 10 chars of SHA256 hash, uppercase)
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let hash = hasher.finalize();
    let hash_hex = hex::encode_upper(hash);
    let team_id = &hash_hex[hash_hex.len() - 10..];

    // Bundle ID components
    let bundle_ti = format!("fyi.zxcvbn.appdupe.{}", team_id);
    let bundle_suffix = bundle.unwrap_or_else(|| Uuid::new_v4().to_string()[..10].replace('-', ""));
    let bundle_id = format!("fyi.zxcvbn.appdupe.{}", bundle_suffix);

    println!("[*] seed: \"{}\"", seed);
    println!("[*] bundle id: {}", bundle_id);
    println!("[*] team id: {}", team_id);

    // Create temp directory
    let tmpdir = TempDir::new()?;
    let tmpdir_path = tmpdir.path();

    // Extract IPA
    println!("[*] extracting...");
    let app_path = extract_ipa(&input, tmpdir_path)?;

    // Load app bundle
    let mut app = AppBundle::new(&app_path)?;

    // Modify plist
    app.plist.set("CFBundleIdentifier", plist::Value::String(bundle_id.clone()));
    app.plist.remove("UISupportedDevices");
    app.plist.remove("CFBundleURLTypes");

    // Get and modify entitlements
    let ent_path = tmpdir_path.join("entitlements.plist");
    let has_entitlements = app.executable.write_entitlements(&ent_path)?;
    
    let mut entitlements: plist::Dictionary = if has_entitlements {
        let ent_data = fs::read(&ent_path)?;
        plist::from_bytes(&ent_data).unwrap_or_default()
    } else {
        plist::Dictionary::new()
    };

    // Set required entitlements
    entitlements.insert(
        "application-identifier".to_string(),
        plist::Value::String(format!("{}.{}", team_id, bundle_id)),
    );
    entitlements.insert(
        "com.apple.developer.team-identifier".to_string(),
        plist::Value::String(team_id.to_string()),
    );
    entitlements.insert(
        "keychain-access-groups".to_string(),
        plist::Value::Array(vec![plist::Value::String(bundle_ti.clone())]),
    );
    entitlements.insert(
        "com.apple.security.application-groups".to_string(),
        plist::Value::Array(vec![plist::Value::String(format!("group.{}", bundle_ti))]),
    );

    // Remove associated domains (prevents URL conflicts)
    entitlements.remove("com.apple.developer.associated-domains");

    // Write modified entitlements
    let mut ent_file = File::create(&ent_path)?;
    plist::to_writer_xml(&mut ent_file, &entitlements)?;

    // Remove app extensions (PlugIns and Extensions)
    app.remove_all_extensions();

    // Sign with new entitlements
    app.executable.sign_with_entitlements(&ent_path)?;

    // Save plist changes
    app.plist.save()?;

    // Create output IPA
    println!("[*] generating...");
    create_ipa(tmpdir_path, &output, 6)?;

    println!("[*] done: {}", output.display());

    Ok(())
}
