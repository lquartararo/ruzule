use anyhow::{bail, Context, Result};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub fn extract_ipa(path: &Path, tmpdir: &Path) -> Result<PathBuf> {
    let payload = tmpdir.join("Payload");

    println!("[*] extracting ipa..");

    let file = File::open(path).context("Failed to open IPA file")?;
    let mut archive = ZipArchive::new(file).context("Invalid IPA (not a zip file)")?;

    let has_payload = archive.file_names().any(|n| n.starts_with("Payload/"));
    if !has_payload {
        bail!("couldn't find Payload folder, invalid ipa");
    }

    let has_plist = archive.file_names().any(|n| n.ends_with(".app/Info.plist"));
    if !has_plist {
        bail!("no Info.plist, invalid app");
    }

    archive.extract(tmpdir).context("Failed to extract IPA")?;

    let app_path = find_app_in_payload(&payload)?;
    println!("[*] extracted ipa");

    Ok(app_path)
}

pub fn copy_app(path: &Path, tmpdir: &Path) -> Result<PathBuf> {
    let payload = tmpdir.join("Payload");

    if !path.join("Info.plist").is_file() {
        bail!("no Info.plist, invalid app");
    }

    println!("[*] copying app..");
    fs::create_dir_all(&payload)?;

    let app_name = path.file_name().context("Invalid app path")?;
    let dest = payload.join(app_name);

    copy_dir_recursive(path, &dest)?;
    println!("[*] copied app");

    Ok(dest)
}

fn find_app_in_payload(payload: &Path) -> Result<PathBuf> {
    for entry in fs::read_dir(payload)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.extension().is_some_and(|e| e == "app") {
            return Ok(path);
        }
    }
    bail!("couldn't find app folder in Payload, invalid ipa")
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
}

pub fn make_ipa(tmpdir: &Path, output: &Path, level: u8) -> Result<()> {
    let payload = tmpdir.join("Payload");

    if output.exists() {
        fs::remove_file(output)?;
    }

    let file = File::create(output)?;
    let mut zip = ZipWriter::new(file);

    let compression = if level == 0 {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Deflated
    };

    let options = SimpleFileOptions::default()
        .compression_method(compression)
        .compression_level(Some(level as i64));

    let mut weird = 0;

    for entry in WalkDir::new(&payload) {
        let entry = entry?;
        let path = entry.path();
        
        let name = path.strip_prefix(tmpdir)?;
        let name_str = name.to_string_lossy();
        
        if name_str.contains("/.") || name_str.starts_with('.') {
            continue;
        }

        if path.is_file() {
            match zip.start_file(&*name_str, options) {
                Ok(_) => {
                    let mut f = File::open(path)?;
                    let mut buffer = Vec::new();
                    f.read_to_end(&mut buffer)?;
                    zip.write_all(&buffer)?;
                }
                Err(_) => {
                    weird += 1;
                }
            }
        } else if path.is_dir() && !name_str.is_empty() {
            let _ = zip.add_directory(format!("{}/", name_str), options);
        }
    }

    zip.finish()?;

    if weird != 0 {
        println!("[?] was unable to zip {} file(s) due to timestamps", weird);
    }

    Ok(())
}

pub fn get_app(input: &Path, tmpdir: &Path) -> Result<PathBuf> {
    let input_str = input.to_string_lossy();
    
    let is_ipa = input_str.ends_with(".ipa") || input_str.ends_with(".tipa");
    
    if is_ipa {
        extract_ipa(input, tmpdir)
    } else {
        copy_app(input, tmpdir)
    }
}
