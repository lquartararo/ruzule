use crate::error::{Result, RuzuleError};
use apple_codesign::{MachFile, SettingsScope, SigningSettings, UnifiedSigner};
use std::fs;
use std::path::Path;
use tempfile::NamedTempFile;

/// Ad-hoc sign a Mach-O binary (no entitlements, no certificate)
pub fn fakesign<P: AsRef<Path>>(path: P) -> Result<bool> {
    let path = path.as_ref();
    let settings = SigningSettings::default();
    sign_macho_in_place(path, &settings)
}

/// Sign a Mach-O binary with entitlements (ad-hoc, no certificate)
pub fn sign_with_entitlements<P: AsRef<Path>, Q: AsRef<Path>>(
    path: P,
    entitlements: Q,
) -> Result<bool> {
    let path = path.as_ref();
    let ent_path = entitlements.as_ref();

    let ent_xml = fs::read_to_string(ent_path)?;

    let mut settings = SigningSettings::default();
    settings
        .set_entitlements_xml(SettingsScope::Main, &ent_xml)
        .map_err(|e| RuzuleError::Sign(format!("Failed to set entitlements: {}", e)))?;

    sign_macho_in_place(path, &settings)
}

/// Extract entitlements from a signed Mach-O binary
pub fn extract_entitlements<P: AsRef<Path>>(path: P) -> Result<Vec<u8>> {
    let path = path.as_ref();
    let data = fs::read(path)?;
    let data = Box::leak(data.into_boxed_slice());

    let mach = MachFile::parse(data)
        .map_err(|e| RuzuleError::Sign(format!("Failed to parse Mach-O: {}", e)))?;

    // Get entitlements from first arch
    if let Some(macho) = mach.iter_macho().next() {
        if let Ok(Some(sig)) = macho.code_signature() {
            if let Ok(Some(ent)) = sig.entitlements() {
                return Ok(ent.as_str().as_bytes().to_vec());
            }
        }
    }

    Ok(Vec::new())
}

/// Remove code signature from a Mach-O binary
pub fn remove_signature<P: AsRef<Path>>(path: P) -> Result<()> {
    crate::macho::remove_code_signature(path)?;
    Ok(())
}

fn sign_macho_in_place(path: &Path, settings: &SigningSettings) -> Result<bool> {
    let signer = UnifiedSigner::new(settings.clone());

    // Create a temp file for output
    let temp_file = NamedTempFile::new()?;
    let temp_path = temp_file.path();

    // Sign to temp file
    signer
        .sign_macho(path, temp_path)
        .map_err(|e| RuzuleError::Sign(format!("Failed to sign: {}", e)))?;

    // Copy back to original
    fs::copy(temp_path, path)?;

    Ok(true)
}
