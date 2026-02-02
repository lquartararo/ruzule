use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuzuleError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Plist error: {0}")]
    Plist(#[from] plist::Error),

    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Goblin error: {0}")]
    Goblin(#[from] goblin::error::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    #[error("WalkDir error: {0}")]
    WalkDir(#[from] walkdir::Error),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Invalid IPA: {0}")]
    InvalidIpa(String),

    #[error("Invalid app bundle: {0}")]
    InvalidAppBundle(String),

    #[error("Encrypted binary: {0}")]
    EncryptedBinary(PathBuf),

    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),

    #[error("External tool failed: {0}")]
    ToolFailed(String),

    #[error("Mach-O manipulation error: {0}")]
    MachO(String),

    #[error("Signing error: {0}")]
    Sign(String),
}

pub type Result<T> = std::result::Result<T, RuzuleError>;
