use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

use crate::cli::Args;

#[derive(Debug, Deserialize, Default)]
pub struct CyanConfig {
    #[serde(default)]
    pub f: bool,
    #[serde(default)]
    pub n: Option<String>,
    #[serde(default)]
    pub v: Option<String>,
    #[serde(default)]
    pub b: Option<String>,
    #[serde(default)]
    pub m: Option<String>,
    #[serde(default)]
    pub k: bool,
    #[serde(default)]
    pub l: bool,
    #[serde(default)]
    pub x: bool,
    #[serde(rename = "remove-supported-devices")]
    #[serde(default)]
    pub remove_supported_devices: Option<bool>,
    #[serde(rename = "no-watch")]
    #[serde(default)]
    pub no_watch: Option<bool>,
    #[serde(rename = "enable-documents")]
    #[serde(default)]
    pub enable_documents: Option<bool>,
    #[serde(default)]
    pub fakesign: Option<bool>,
    #[serde(default)]
    pub thin: Option<bool>,
}

pub fn parse_cyans(
    args: &mut Args,
    tweaks: &mut HashMap<String, PathBuf>,
    tmpdir: &Path,
) -> Result<()> {
    let cyan_files = match &args.cyan {
        Some(files) => files.clone(),
        None => return Ok(()),
    };

    for (ind, cyan_path) in cyan_files.iter().enumerate() {
        println!("[*] parsing {} ..", cyan_path.file_name().unwrap().to_string_lossy());

        let file = fs::File::open(cyan_path)?;
        let mut archive = ZipArchive::new(file)?;

        let dot_path = tmpdir.join(format!("cyan-{}", ind));
        fs::create_dir_all(&dot_path)?;

        let config: CyanConfig = {
            let mut config_file = archive.by_name("config.json")?;
            serde_json::from_reader(&mut config_file)?
        };

        if config.f {
            let inject_dir = dot_path.join("inject");
            fs::create_dir_all(&inject_dir)?;

            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let name = file.name().to_string();
                
                if name.starts_with("inject/") && name.len() > 7 {
                    let out_path = dot_path.join(&name);
                    if let Some(parent) = out_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    
                    if !name.ends_with('/') {
                        let mut out_file = fs::File::create(&out_path)?;
                        std::io::copy(&mut file, &mut out_file)?;
                    }
                }
            }

            for entry in fs::read_dir(&inject_dir)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                tweaks.insert(name, entry.path());
            }
        }

        if config.k {
            let icon_path = dot_path.join("icon.idk");
            extract_file(&mut archive, "icon.idk", &icon_path)?;
            args.icon = Some(icon_path);
        }

        if config.l {
            let plist_path = dot_path.join("merge.plist");
            extract_file(&mut archive, "merge.plist", &plist_path)?;
            args.merge_plist = Some(plist_path);
        }

        if config.x {
            let ent_path = dot_path.join("new.entitlements");
            extract_file(&mut archive, "new.entitlements", &ent_path)?;
            args.entitlements = Some(ent_path);
        }

        if let Some(n) = config.n {
            args.name = Some(n);
        }
        if let Some(v) = config.v {
            args.version = Some(v);
        }
        if let Some(b) = config.b {
            args.bundle_id = Some(b);
        }
        if let Some(m) = config.m {
            args.minimum = Some(m);
        }
        if let Some(v) = config.remove_supported_devices {
            args.remove_supported_devices = v;
        }
        if let Some(v) = config.no_watch {
            args.no_watch = v;
        }
        if let Some(v) = config.enable_documents {
            args.enable_documents = v;
        }
        if let Some(v) = config.fakesign {
            args.fakesign = v;
        }
        if let Some(v) = config.thin {
            args.thin = v;
        }
    }

    Ok(())
}

fn extract_file(archive: &mut ZipArchive<fs::File>, name: &str, output: &Path) -> Result<()> {
    let mut file = archive.by_name(name)?;
    let mut out_file = fs::File::create(output)?;
    std::io::copy(&mut file, &mut out_file)?;
    Ok(())
}
