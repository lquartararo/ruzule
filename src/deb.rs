use crate::error::{Result, RuzuleError};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub fn extract_deb(
    deb_path: &Path,
    tweaks: &mut HashMap<String, PathBuf>,
    tmpdir: &Path,
) -> Result<()> {
    let deb_name = deb_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let extract_dir = tmpdir.join(format!("deb_{}", uuid::Uuid::new_v4().simple()));
    fs::create_dir_all(&extract_dir)?;

    // Read the .deb file (it's an ar archive)
    let file = BufReader::new(File::open(deb_path)?);
    let mut archive = ar::Archive::new(file);

    let mut data_tar_path = None;

    loop {
        match archive.next_entry() {
            Some(Ok(mut entry)) => {
                let name = std::str::from_utf8(entry.header().identifier())
                    .unwrap_or("")
                    .trim_end_matches('/')
                    .trim()
                    .to_string();

                if name.starts_with("data.tar") {
                    let tar_path = extract_dir.join(&name);
                    let mut tar_file = File::create(&tar_path)?;
                    std::io::copy(&mut entry, &mut tar_file)?;
                    data_tar_path = Some(tar_path);
                    break; // Found what we need
                }
            }
            Some(Err(_)) => continue, // Skip problematic entries
            None => break,            // No more entries
        }
    }

    let data_tar_path = data_tar_path.ok_or_else(|| {
        RuzuleError::InvalidInput(format!("No data.tar found in {}", deb_name))
    })?;

    // Extract the data tar
    extract_data_tar(&data_tar_path, &extract_dir)?;

    // Find injectables
    let patterns = ["**/*.dylib", "**/*.appex", "**/*.bundle", "**/*.framework"];

    for pattern in patterns {
        let full_pattern = format!("{}/{}", extract_dir.display(), pattern);
        if let Ok(paths) = glob::glob(&full_pattern) {
            for entry in paths.flatten() {
                // Skip symlinks for security
                if entry.is_symlink() {
                    continue;
                }

                // Skip nested bundles/frameworks
                let path_str = entry.to_string_lossy();
                if (path_str.matches(".bundle").count() > 1)
                    || (path_str.matches(".framework").count() > 1)
                {
                    continue;
                }

                if let Some(name) = entry.file_name() {
                    let name = name.to_string_lossy().to_string();
                    tweaks.insert(name, entry);
                }
            }
        }
    }

    println!("[*] extracted {}", deb_name);

    // Remove the deb from tweaks
    tweaks.remove(&deb_name);

    Ok(())
}

fn extract_data_tar<P: AsRef<Path>>(tar_path: P, dest: P) -> Result<()> {
    let tar_path = tar_path.as_ref();
    let dest = dest.as_ref();

    let file = File::open(tar_path)?;
    let tar_name = tar_path.file_name().unwrap().to_string_lossy();

    // Determine compression
    if tar_name.ends_with(".tar.gz") || tar_name.ends_with(".tar.gzip") {
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(dest)?;
    } else if tar_name.ends_with(".tar.xz") {
        let decoder = xz2::read::XzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(dest)?;
    } else if tar_name.ends_with(".tar.lzma") {
        // LZMA uses a different stream format than XZ
        let decoder = xz2::read::XzDecoder::new_stream(
            file,
            xz2::stream::Stream::new_lzma_decoder(u64::MAX).map_err(|e| {
                RuzuleError::InvalidInput(format!("LZMA decoder error: {}", e))
            })?,
        );
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(dest)?;
    } else if tar_name.ends_with(".tar.zst") || tar_name.ends_with(".tar.zstd") {
        // zstd support would require adding the zstd crate
        return Err(RuzuleError::InvalidInput(
            "zstd compression not yet supported".to_string(),
        ));
    } else if tar_name.ends_with(".tar.bz2") {
        // bz2 support would require adding the bzip2 crate
        return Err(RuzuleError::InvalidInput(
            "bz2 compression not yet supported".to_string(),
        ));
    } else {
        // Assume uncompressed tar
        let mut archive = tar::Archive::new(file);
        archive.unpack(dest)?;
    }

    Ok(())
}
