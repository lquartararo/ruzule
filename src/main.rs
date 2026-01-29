mod cgen;
mod cli;
mod types;
mod utils;

use anyhow::Result;
use clap::Parser;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use cli::Args;
use types::AppBundle;
use utils::ipa;

fn main() {
    // Check if running as cgen
    let args: Vec<String> = std::env::args().collect();
    let is_cgen = args.first().map(|s| s.ends_with("cgen")).unwrap_or(false)
        || args.get(1).map(|s| s == "cgen").unwrap_or(false);

    if is_cgen {
        let cgen_args = if args.get(1).map(|s| s == "cgen").unwrap_or(false) {
            // Called as "ruzule cgen ..."
            let filtered: Vec<_> = std::iter::once(args[0].clone())
                .chain(args.into_iter().skip(2))
                .collect();
            cgen::CgenArgs::parse_from(filtered)
        } else {
            // Called as "cgen ..."
            cgen::CgenArgs::parse()
        };

        if let Err(e) = cgen::run(cgen_args) {
            eprintln!("[!] {}", e);
            std::process::exit(1);
        }
        return;
    }

    if let Err(e) = run() {
        eprintln!("[!] {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    if cfg!(windows) {
        anyhow::bail!("windows is not supported");
    }

    let mut args = Args::parse();
    let output = args.get_output();
    args.output = Some(output.clone());

    if let Some(err) = utils::validate_inputs(&args) {
        anyhow::bail!("{}", err);
    }

    let _input_is_ipa = args.input.ends_with(".ipa") || args.input.ends_with(".tipa");
    let output_is_ipa = output.ends_with(".ipa") || output.ends_with(".tipa");

    let tmpdir = TempDir::new()?;
    let tmpdir_path = tmpdir.path();

    let app_path = ipa::get_app(Path::new(&args.input), tmpdir_path)?;
    let mut app = AppBundle::new(&app_path)?;

    // Determine what tools we need based on requested operations
    let need_signing = args.fakesign || args.entitlements.is_some() || args.files.is_some();
    let need_injection = args.files.is_some();
    let need_tools = need_signing || need_injection;

    let tools = utils::tools::Tools::new()?;
    if need_tools {
        tools.check_required(need_signing, need_injection)?;
    }

    if app.executable.is_encrypted()? {
        if args.ignore_encrypted {
            println!("[?] main binary is encrypted, ignoring");
        } else {
            anyhow::bail!("main binary is encrypted; exiting");
        }
    }

    let mut tweaks: HashMap<String, PathBuf> = HashMap::new();

    if args.cyan.is_some() {
        utils::cyan::parse_cyans(&mut args, &mut tweaks, tmpdir_path)?;
    }

    if args.remove_extensions {
        app.remove_all_extensions();
    } else if args.remove_encrypted {
        app.remove_encrypted_extensions(&tools)?;
    }

    if let Some(ref files) = args.files {
        for f in files {
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            let path = fs::canonicalize(f)?;
            tweaks.insert(name, path);
        }
    }

    if !tweaks.is_empty() {
        app.inject(&mut tweaks, tmpdir_path, &tools)?;
    }

    if let Some(ref name) = args.name {
        app.plist.change_name(name);
    }
    if let Some(ref version) = args.version {
        app.plist.change_version(version);
    }
    if let Some(ref bundle_id) = args.bundle_id {
        app.plist.change_bundle_id(bundle_id);
    }
    if let Some(ref minimum) = args.minimum {
        app.plist.change_minimum_version(minimum);
    }
    if let Some(ref icon) = args.icon {
        app.change_icon(icon, tmpdir_path)?;
    }
    if let Some(ref merge_plist) = args.merge_plist {
        app.plist.merge_plist(merge_plist)?;
    }
    if let Some(ref entitlements) = args.entitlements {
        app.executable.merge_entitlements(entitlements, &tools)?;
    }

    if args.remove_supported_devices {
        app.plist.remove_uisd();
    }
    if args.no_watch {
        app.remove_watch_apps();
    }
    if args.enable_documents {
        app.plist.enable_documents();
    }
    if args.fakesign {
        app.fakesign_all(&tools)?;
    }
    if args.thin {
        app.thin_all()?;
    }

    app.plist.save()?;

    if let Some(parent) = PathBuf::from(&output).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    if output_is_ipa {
        println!("[*] generating ipa with compression level {}..", args.compress);
        ipa::make_ipa(tmpdir_path, Path::new(&output), args.compress)?;
        println!("[*] generated ipa at {}", output);
    } else {
        let output_path = PathBuf::from(&output);
        if output_path.is_dir() {
            fs::remove_dir_all(&output_path)?;
        }
        fs::rename(&app.path, &output_path)?;
        println!("[*] generated app at {}", output);
    }

    Ok(())
}
