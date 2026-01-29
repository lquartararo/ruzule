use anyhow::{bail, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

pub fn extract_deb(
    deb: &Path,
    tweaks: &mut HashMap<String, PathBuf>,
    tmpdir: &Path,
) -> Result<()> {
    let deb_name = deb.file_name().unwrap().to_string_lossy().to_string();
    let t2 = tmpdir.join(format!("deb_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&t2)?;

    let system = std::env::consts::OS;
    
    let ar_result = if system == "linux" {
        Command::new("ar")
            .args(["-x", deb.to_str().unwrap(), &format!("--output={}", t2.display())])
            .output()
    } else {
        Command::new("tar")
            .args(["-xf", deb.to_str().unwrap(), "-C", t2.to_str().unwrap()])
            .output()
    };

    if ar_result.is_err() || !ar_result.unwrap().status.success() {
        bail!("couldn't extract {}", deb_name);
    }

    let data_tar = find_data_tar(&t2)?;
    
    Command::new("tar")
        .args(["-xf", data_tar.to_str().unwrap(), "-C", t2.to_str().unwrap()])
        .output()?;

    for entry in WalkDir::new(&t2).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        
        if path.is_symlink() {
            continue;
        }
        
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy();
            if ext_str == "dylib"
                || ext_str == "appex"
                || ext_str == "bundle"
                || ext_str == "framework"
            {
                let path_str = path.to_string_lossy();
                if path_str.matches(".bundle").count() > 1
                    || path_str.matches(".framework").count() > 1
                {
                    continue;
                }

                if let Some(name) = path.file_name() {
                    tweaks.insert(name.to_string_lossy().to_string(), path.to_path_buf());
                }
            }
        }
    }

    println!("[*] extracted {}", deb_name);
    tweaks.remove(&deb_name);

    Ok(())
}

fn find_data_tar(dir: &Path) -> Result<PathBuf> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("data.") {
            return Ok(entry.path());
        }
    }
    bail!("couldn't find data.tar in deb")
}
