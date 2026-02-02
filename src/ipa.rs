use crate::error::{Result, RuzuleError};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

pub fn extract_ipa<P: AsRef<Path>, Q: AsRef<Path>>(ipa_path: P, dest: Q) -> Result<PathBuf> {
    let ipa_path = ipa_path.as_ref();
    let dest = dest.as_ref();

    let file = File::open(ipa_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // Check for valid IPA structure
    let has_payload = archive
        .file_names()
        .any(|name| name.starts_with("Payload/"));
    if !has_payload {
        return Err(RuzuleError::InvalidIpa(
            "No Payload folder found".to_string(),
        ));
    }

    let has_info_plist = archive
        .file_names()
        .any(|name| name.ends_with(".app/Info.plist"));
    if !has_info_plist {
        return Err(RuzuleError::InvalidIpa(
            "No Info.plist found, invalid app".to_string(),
        ));
    }

    // Extract all files
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = dest.join(file.name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;

            // Preserve Unix permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
                }
            }
        }
    }

    // Find the .app folder
    let payload = dest.join("Payload");
    let app_path = find_app_in_payload(&payload)?;

    Ok(app_path)
}

fn find_app_in_payload(payload: &Path) -> Result<PathBuf> {
    for entry in fs::read_dir(payload)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.extension().map(|e| e == "app").unwrap_or(false) {
            return Ok(path);
        }
    }
    Err(RuzuleError::InvalidIpa("No .app folder found".to_string()))
}

pub fn copy_app<P: AsRef<Path>, Q: AsRef<Path>>(app_path: P, dest: Q) -> Result<PathBuf> {
    let app_path = app_path.as_ref();
    let dest = dest.as_ref();

    // Check for Info.plist
    if !app_path.join("Info.plist").exists() {
        return Err(RuzuleError::InvalidAppBundle(
            "No Info.plist found".to_string(),
        ));
    }

    let payload = dest.join("Payload");
    fs::create_dir_all(&payload)?;

    let app_name = app_path
        .file_name()
        .ok_or_else(|| RuzuleError::InvalidInput("Invalid app path".to_string()))?;
    let new_app_path = payload.join(app_name);

    copy_dir_all(app_path, &new_app_path)?;

    Ok(new_app_path)
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else if ty.is_symlink() {
            let target = fs::read_link(&src_path)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(target, &dst_path)?;
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(target, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

pub fn create_ipa<P: AsRef<Path>, Q: AsRef<Path>>(tmpdir: P, output: Q, compression_level: u32) -> Result<()> {
    let tmpdir = tmpdir.as_ref();
    let output = output.as_ref();



    let file = File::create(output)?;
    let mut zip = zip::ZipWriter::new(file);

    let compression = match compression_level {
        0 => CompressionMethod::Stored,
        _ => CompressionMethod::Deflated,
    };

    let options = SimpleFileOptions::default()
        .compression_method(compression)
        .compression_level(Some(compression_level as i64));

    let payload = tmpdir.join("Payload");

    for entry in WalkDir::new(&payload) {
        let entry = entry?;
        let path = entry.path();
        let name = path.strip_prefix(tmpdir).expect("path is within tmpdir");

        // Skip hidden files (fixes installd errors)
        if name
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        {
            continue;
        }

        if path.is_file() {
            let name_str = name.to_string_lossy().replace('\\', "/");
            zip.start_file(&name_str, options)?;
            let mut f = File::open(path)?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
        } else if path.is_dir() && path != payload {
            let name_str = format!("{}/", name.to_string_lossy().replace('\\', "/"));
            zip.add_directory(&name_str, options)?;
        }
    }

    zip.finish()?;

    Ok(())
}
