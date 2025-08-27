//! # niv_fs - Filesystem utilities for niv editor
//!
//! Comprehensive file operations with encoding detection, atomic writes, and external change detection.
//!
//! Modules:
//! - `bom` for BOM detection (UTF-8/16/32)
//! - `encoding` for heuristic detection (UTF-8, UTF-16, Latin-1/9, Windows-1252)
//! - `file` for file loading/saving operations

mod bom;
mod encoding;
mod file;

pub use bom::{BomDetectionResult, detect_bom};
pub use encoding::{
    DetectionConfidence, DetectionConfig, Encoding, EncodingDetectionResult,
    detect_encoding_heuristic, detect_encoding_heuristic_with_confidence,
};
pub use file::{
    FileLoadResult, FileLoadConfig, load_file, load_file_with_config,
    FileSaveResult, FileSaveConfig, save_file, save_file_with_config,
    SaveContext, FileIdentity, FileIdentityConfig,
};

use std::fmt;
use std::io;

/// Errors that can occur during encoding detection
#[derive(Debug)]
pub enum EncodingError {
    /// I/O error while reading file
    Io(io::Error),
    /// File appears to be binary (contains many null/control bytes)
    BinaryFile,
    /// File is too large to process
    FileTooLarge,
}

impl fmt::Display for EncodingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodingError::Io(err) => write!(f, "I/O error: {}", err),
            EncodingError::BinaryFile => write!(f, "File appears to be binary"),
            EncodingError::FileTooLarge => write!(f, "File is too large to process"),
        }
    }
}

impl std::error::Error for EncodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EncodingError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for EncodingError {
    fn from(err: io::Error) -> Self {
        EncodingError::Io(err)
    }
}

/// Result type for encoding detection operations
pub type EncodingResult<T> = Result<T, EncodingError>;

/// Detect the encoding of a file from its byte content.
///
/// Strategy:
/// 1) BOM detection (high confidence)
/// 2) Heuristic detection (UTF-16/UTF-8/Latin families)
pub fn detect_encoding(bytes: &[u8], config: Option<DetectionConfig>) -> EncodingResult<Encoding> {
    let cfg = config.unwrap_or_default();

    let bom = detect_bom(bytes);
    if bom.encoding != Encoding::Unknown {
        return Ok(bom.encoding);
    }

    encoding::detect_encoding_heuristic(bytes, cfg)
}

/// Detect encoding with confidence information.
pub fn detect_encoding_with_confidence(
    bytes: &[u8],
    config: Option<DetectionConfig>,
) -> EncodingResult<EncodingDetectionResult> {
    let cfg = config.unwrap_or_default();

    let bom = detect_bom(bytes);
    if bom.encoding != Encoding::Unknown {
        return Ok(EncodingDetectionResult {
            encoding: bom.encoding,
            confidence: DetectionConfidence::High,
        });
    }

    if let Some(r) = encoding::detect_encoding_heuristic_with_confidence(bytes, cfg) {
        return Ok(r);
    }

    Ok(EncodingDetectionResult {
        encoding: Encoding::Utf8,
        confidence: DetectionConfidence::Unknown,
    })
}

/// Detect encoding from a file path (reads up to `sample_size`).
pub fn detect_encoding_from_file<P: AsRef<std::path::Path>>(
    path: P,
    config: Option<DetectionConfig>,
) -> EncodingResult<Encoding> {
    use std::fs;
    let cfg = config.unwrap_or_default();

    let mut buffer = vec![0u8; cfg.sample_size];
    let mut file = fs::File::open(path)?;
    let bytes_read = io::Read::read(&mut file, &mut buffer)?;
    let content = &buffer[..bytes_read];

    detect_encoding(content, Some(cfg))
}

/// Detect encoding from a file path with confidence information.
pub fn detect_encoding_from_file_with_confidence<P: AsRef<std::path::Path>>(
    path: P,
    config: Option<DetectionConfig>,
) -> EncodingResult<EncodingDetectionResult> {
    use std::fs;
    let cfg = config.unwrap_or_default();

    let mut buffer = vec![0u8; cfg.sample_size];
    let mut file = fs::File::open(path)?;
    let bytes_read = io::Read::read(&mut file, &mut buffer)?;
    let content = &buffer[..bytes_read];

    detect_encoding_with_confidence(content, Some(cfg))
}
