use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CyanConfig {
    #[serde(default)]
    pub f: bool,  // Has files to inject
    #[serde(default)]
    pub n: Option<String>,  // App name
    #[serde(default)]
    pub v: Option<String>,  // Version
    #[serde(default)]
    pub b: Option<String>,  // Bundle ID
    #[serde(default)]
    pub m: Option<String>,  // Minimum OS version
    #[serde(default)]
    pub k: bool,  // Has icon
    #[serde(default)]
    pub l: bool,  // Has plist to merge
    #[serde(default)]
    pub x: bool,  // Has entitlements
    #[serde(default)]
    pub remove_supported_devices: bool,
    #[serde(default)]
    pub no_watch: bool,
    #[serde(default)]
    pub enable_documents: bool,
    #[serde(default)]
    pub fakesign: bool,
    #[serde(default)]
    pub thin: bool,
    #[serde(default)]
    pub remove_extensions: bool,
    #[serde(default)]
    pub remove_encrypted: bool,
    #[serde(default)]
    pub patch_plugins: bool,
}

pub struct ParsedCyan {
    pub config: CyanConfig,
    pub files: HashMap<String, PathBuf>,
    pub icon: Option<PathBuf>,
    pub plist: Option<PathBuf>,
    pub entitlements: Option<PathBuf>,
}

pub fn parse_cyan<P: AsRef<Path>, Q: AsRef<Path>>(cyan_path: P, tmpdir: Q, index: usize) -> Result<ParsedCyan> {
    let cyan_path = cyan_path.as_ref();
    let tmpdir = tmpdir.as_ref();

    println!("[*] loading {}", cyan_path.file_name().unwrap().to_string_lossy());

    let file = File::open(cyan_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let extract_dir = tmpdir.join(format!("cyan-{}", index));
    fs::create_dir_all(&extract_dir)?;

    // Read config.json
    let config: CyanConfig = {
        let mut config_file = archive.by_name("config.json")?;
        let mut contents = String::new();
        config_file.read_to_string(&mut contents)?;
        serde_json::from_str(&contents)?
    };

    let mut files = HashMap::new();
    let mut icon = None;
    let mut plist = None;
    let mut entitlements = None;

    // Extract relevant files
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        if name.starts_with("inject/") && config.f {
            let outpath = extract_dir.join(&name);
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p)?;
            }
            if !name.ends_with('/') {
                let mut outfile = File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        } else if name == "icon.idk" && config.k {
            let outpath = extract_dir.join(&name);
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
            icon = Some(outpath);
        } else if name == "merge.plist" && config.l {
            let outpath = extract_dir.join(&name);
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
            plist = Some(outpath);
        } else if name == "new.entitlements" && config.x {
            let outpath = extract_dir.join(&name);
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
            entitlements = Some(outpath);
        }
    }

    // Collect files from inject directory
    if config.f {
        let inject_dir = extract_dir.join("inject");
        if inject_dir.exists() {
            for entry in fs::read_dir(&inject_dir)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                files.insert(name, entry.path());
            }
        }
    }

    Ok(ParsedCyan {
        config,
        files,
        icon,
        plist,
        entitlements,
    })
}
