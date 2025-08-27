//! Atomic file saving with transcoding and permission preservation.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::bom::BomDetectionResult;
use crate::encoding::Encoding;
use super::eol::{EolType, restore_eol};
use super::identity::FileIdentity;

/// Configuration for file saving operations
#[derive(Debug, Clone)]
pub struct FileSaveConfig {
    /// Whether to preserve file permissions (Unix only)
    pub preserve_permissions: bool,
    /// Whether to use atomic writes with temp files
    pub atomic_writes: bool,
    /// Custom temp file suffix
    pub temp_suffix: String,
    /// Buffer size for streaming writes
    pub buffer_size: usize,
}

impl Default for FileSaveConfig {
    fn default() -> Self {
        FileSaveConfig {
            preserve_permissions: true,
            atomic_writes: true,
            temp_suffix: ".tmp".to_string(),
            buffer_size: 64 * 1024, // 64KB
        }
    }
}

/// Result of a file saving operation
#[derive(Debug)]
pub struct FileSaveResult {
    /// Final path where file was saved
    pub path: PathBuf,
    /// Number of bytes written
    pub bytes_written: u64,
    /// Whether atomic write was used
    pub atomic_write: bool,
    /// Any warnings encountered during save
    pub warnings: Vec<String>,
}

/// Context information for saving (from original load)
#[derive(Debug, Clone)]
pub struct SaveContext {
    /// Original encoding detected during load
    pub original_encoding: Encoding,
    /// Original end-of-line type
    pub original_eol: EolType,
    /// Original BOM (if any)
    pub original_bom: BomDetectionResult,
    /// Original file identity
    pub original_identity: FileIdentity,
}

impl SaveContext {
    /// Create a default save context (for new files)
    pub fn new() -> Self {
        SaveContext {
            original_encoding: Encoding::Utf8,
            original_eol: EolType::Lf,
            original_bom: BomDetectionResult {
                encoding: Encoding::Unknown,
                bom_length: 0,
            },
            original_identity: FileIdentity {
                device_id: 0,
                inode: 0,
                size: 0,
                mtime: std::time::SystemTime::now(),
                content_hash: None,
            },
        }
    }

    /// Create save context from a file load result
    pub fn from_load_result(result: &super::load::FileLoadResult) -> Self {
        SaveContext {
            original_encoding: result.original_encoding,
            original_eol: result.original_eol,
            original_bom: BomDetectionResult {
                encoding: result.original_encoding,
                bom_length: match result.original_encoding {
                    Encoding::Utf8 => 3,
                    Encoding::Utf16Le | Encoding::Utf16Be => 2,
                    Encoding::Utf32Le | Encoding::Utf32Be => 4,
                    _ => 0,
                },
            },
            original_identity: result.identity.clone(),
        }
    }
}

/// Save content to file with transcoding and atomic writes.
///
/// This function:
/// 1. Transcodes UTF-8 content back to original encoding
/// 2. Restores original end-of-line type
/// 3. Adds BOM if original file had one
/// 4. Performs atomic write using temp file
/// 5. Preserves file permissions
pub fn save_file<P: AsRef<Path>>(
    path: P,
    content: &str,
    context: &SaveContext,
) -> Result<FileSaveResult, crate::EncodingError> {
    save_file_with_config(path, content, context, &FileSaveConfig::default())
}

/// Save file with custom configuration.
pub fn save_file_with_config<P: AsRef<Path>>(
    path: P,
    content: &str,
    context: &SaveContext,
    config: &FileSaveConfig,
) -> Result<FileSaveResult, crate::EncodingError> {
    let path = path.as_ref();

    // Prepare content for saving
    let prepared_content = prepare_content_for_save(content, context)?;

    // Perform atomic write
    if config.atomic_writes {
        save_atomic(path, &prepared_content, context, config)
    } else {
        save_direct(path, &prepared_content, context, config)
    }
}

/// Prepare content for saving by transcoding and restoring format.
fn prepare_content_for_save(
    content: &str,
    context: &SaveContext,
) -> Result<Vec<u8>, crate::EncodingError> {
    // First, restore original EOL type
    let content_with_eol = restore_eol(content.as_bytes(), context.original_eol);

    // Transcode to original encoding
    let transcoded = transcode_to_encoding(&content_with_eol, context.original_encoding)?;

    // Add BOM if original had one
    let final_content = if context.original_bom.bom_length > 0 {
        let mut with_bom = get_bom_bytes(context.original_encoding);
        with_bom.extend_from_slice(&transcoded);
        with_bom
    } else {
        transcoded
    };

    Ok(final_content)
}

/// Transcode UTF-8 content to the specified encoding.
fn transcode_to_encoding(content: &[u8], encoding: Encoding) -> Result<Vec<u8>, crate::EncodingError> {
    match encoding {
        Encoding::Utf8 => Ok(content.to_vec()),
        Encoding::Utf16Le => encode_utf16le(content),
        Encoding::Utf16Be => encode_utf16be(content),
        Encoding::Utf32Le => encode_utf32le(content),
        Encoding::Utf32Be => encode_utf32be(content),
        Encoding::Latin1 | Encoding::Windows1252 | Encoding::Latin9 => {
            encode_latin(content, encoding)
        }
        Encoding::Unknown => Err(crate::EncodingError::BinaryFile),
    }
}

/// Get BOM bytes for an encoding.
fn get_bom_bytes(encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::Utf8 => vec![0xEF, 0xBB, 0xBF],
        Encoding::Utf16Le => vec![0xFF, 0xFE],
        Encoding::Utf16Be => vec![0xFE, 0xFF],
        Encoding::Utf32Le => vec![0xFF, 0xFE, 0x00, 0x00],
        Encoding::Utf32Be => vec![0x00, 0x00, 0xFE, 0xFF],
        _ => vec![],
    }
}

/// Perform atomic save using temp file.
fn save_atomic(
    path: &Path,
    content: &[u8],
    _context: &SaveContext,
    config: &FileSaveConfig,
) -> Result<FileSaveResult, crate::EncodingError> {
    // Create temp file path
    let temp_path = get_temp_path(path, &config.temp_suffix);

    // Write to temp file first
    let bytes_written = write_to_file(&temp_path, content, config)?;

    // Preserve permissions from original file if it exists
    if config.preserve_permissions && path.exists() {
        preserve_permissions(path, &temp_path)?;
    }

    // Atomically move temp file to final location
    fs::rename(&temp_path, path).map_err(|e| {
        // If rename fails, try to clean up temp file
        let _ = fs::remove_file(&temp_path);
        crate::EncodingError::Io(e)
    })?;

    Ok(FileSaveResult {
        path: path.to_path_buf(),
        bytes_written,
        atomic_write: true,
        warnings: vec![],
    })
}

/// Perform direct save (non-atomic).
fn save_direct(
    path: &Path,
    content: &[u8],
    _context: &SaveContext,
    config: &FileSaveConfig,
) -> Result<FileSaveResult, crate::EncodingError> {
    let bytes_written = write_to_file(path, content, config)?;

    Ok(FileSaveResult {
        path: path.to_path_buf(),
        bytes_written,
        atomic_write: false,
        warnings: vec!["Non-atomic write used".to_string()],
    })
}

/// Write content to a file with buffering.
fn write_to_file(
    path: &Path,
    content: &[u8],
    config: &FileSaveConfig,
) -> Result<u64, crate::EncodingError> {
    let file = File::create(path).map_err(crate::EncodingError::Io)?;

    // Use buffered writing for better performance
    let mut writer = io::BufWriter::with_capacity(config.buffer_size, file);

    // Write in chunks to handle large files efficiently
    let mut bytes_written = 0u64;
    let chunk_size = config.buffer_size;

    for chunk in content.chunks(chunk_size) {
        writer.write_all(chunk).map_err(crate::EncodingError::Io)?;
        bytes_written += chunk.len() as u64;
    }

    // Ensure all data is flushed to disk
    writer.flush().map_err(crate::EncodingError::Io)?;
    writer.get_mut().sync_all().map_err(crate::EncodingError::Io)?;

    Ok(bytes_written)
}

/// Generate temp file path.
fn get_temp_path(original_path: &Path, suffix: &str) -> PathBuf {
    let mut temp_path = original_path.to_path_buf();
    let original_name = temp_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");

    let temp_name = format!("{}{}", original_name, suffix);
    temp_path.set_file_name(temp_name);
    temp_path
}

/// Preserve file permissions from source to target.
#[cfg(unix)]
fn preserve_permissions(source: &Path, target: &Path) -> Result<(), crate::EncodingError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(source).map_err(crate::EncodingError::Io)?;
    let permissions = metadata.permissions();

    fs::set_permissions(target, permissions).map_err(crate::EncodingError::Io)
}

#[cfg(not(unix))]
fn preserve_permissions(_source: &Path, _target: &Path) -> Result<(), crate::EncodingError> {
    // Windows permission preservation would require additional dependencies
    Ok(())
}

/// Encode UTF-8 content to UTF-16LE.
fn encode_utf16le(content: &[u8]) -> Result<Vec<u8>, crate::EncodingError> {
    let utf8_str = std::str::from_utf8(content)
        .map_err(|_| crate::EncodingError::BinaryFile)?;

    let mut result = Vec::new();
    for code_unit in utf8_str.encode_utf16() {
        result.extend_from_slice(&code_unit.to_le_bytes());
    }
    Ok(result)
}

/// Encode UTF-8 content to UTF-16BE.
fn encode_utf16be(content: &[u8]) -> Result<Vec<u8>, crate::EncodingError> {
    let utf8_str = std::str::from_utf8(content)
        .map_err(|_| crate::EncodingError::BinaryFile)?;

    let mut result = Vec::new();
    for code_unit in utf8_str.encode_utf16() {
        result.extend_from_slice(&code_unit.to_be_bytes());
    }
    Ok(result)
}

/// Encode UTF-8 content to UTF-32LE.
fn encode_utf32le(content: &[u8]) -> Result<Vec<u8>, crate::EncodingError> {
    let utf8_str = std::str::from_utf8(content)
        .map_err(|_| crate::EncodingError::BinaryFile)?;

    let mut result = Vec::new();
    for ch in utf8_str.chars() {
        result.extend_from_slice(&(ch as u32).to_le_bytes());
    }
    Ok(result)
}

/// Encode UTF-8 content to UTF-32BE.
fn encode_utf32be(content: &[u8]) -> Result<Vec<u8>, crate::EncodingError> {
    let utf8_str = std::str::from_utf8(content)
        .map_err(|_| crate::EncodingError::BinaryFile)?;

    let mut result = Vec::new();
    for ch in utf8_str.chars() {
        result.extend_from_slice(&(ch as u32).to_be_bytes());
    }
    Ok(result)
}

/// Encode UTF-8 content to Latin encoding.
fn encode_latin(content: &[u8], encoding: Encoding) -> Result<Vec<u8>, crate::EncodingError> {
    let utf8_str = std::str::from_utf8(content)
        .map_err(|_| crate::EncodingError::BinaryFile)?;

    let mut result = Vec::new();
    for ch in utf8_str.chars() {
        let byte = match encoding {
            Encoding::Latin1 => char_to_latin1(ch)?,
            Encoding::Windows1252 => char_to_windows1252(ch)?,
            Encoding::Latin9 => char_to_latin9(ch)?,
            _ => unreachable!(),
        };
        result.push(byte);
    }
    Ok(result)
}

/// Convert Unicode character to Latin-1 byte.
fn char_to_latin1(ch: char) -> Result<u8, crate::EncodingError> {
    if ch as u32 <= 0xFF {
        Ok(ch as u8)
    } else {
        Err(crate::EncodingError::BinaryFile) // Character cannot be represented
    }
}

/// Convert Unicode character to Windows-1252 byte.
fn char_to_windows1252(ch: char) -> Result<u8, crate::EncodingError> {
    match ch {
        '€' => Ok(0x80),
        '‚' => Ok(0x82),
        'ƒ' => Ok(0x83),
        '„' => Ok(0x84),
        '…' => Ok(0x85),
        '†' => Ok(0x86),
        '‡' => Ok(0x87),
        'ˆ' => Ok(0x88),
        '‰' => Ok(0x89),
        'Š' => Ok(0x8A),
        '‹' => Ok(0x8B),
        'Œ' => Ok(0x8C),
        'Ž' => Ok(0x8E),
        '‘' => Ok(0x91),
        '’' => Ok(0x92),
        '"' => Ok(0x93), // Double quotation mark
        '•' => Ok(0x95),
        '–' => Ok(0x96),
        '—' => Ok(0x97),
        '˜' => Ok(0x98),
        '™' => Ok(0x99),
        'š' => Ok(0x9A),
        '›' => Ok(0x9B),
        'œ' => Ok(0x9C),
        'ž' => Ok(0x9E),
        'Ÿ' => Ok(0x9F),
        ch if ch as u32 <= 0xFF => Ok(ch as u8),
        _ => Err(crate::EncodingError::BinaryFile),
    }
}

/// Convert Unicode character to Latin-9 byte.
fn char_to_latin9(ch: char) -> Result<u8, crate::EncodingError> {
    match ch {
        '€' => Ok(0xA4), // Euro sign
        'Š' => Ok(0xA6), // Latin capital S with caron
        'š' => Ok(0xA8), // Latin small s with caron
        'Ž' => Ok(0xB4), // Latin capital Z with caron
        'ž' => Ok(0xB8), // Latin small z with caron
        'Œ' => Ok(0xBC), // Latin capital OE ligature
        'œ' => Ok(0xBD), // Latin small oe ligature
        'Ÿ' => Ok(0xBE), // Latin capital Y with diaeresis
        ch if ch as u32 <= 0xFF => Ok(ch as u8),
        _ => Err(crate::EncodingError::BinaryFile),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_save_utf8_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let content = "Hello, UTF-8!\nSecond line";
        let context = SaveContext::new();

        let result = save_file(&temp_file, content, &context).unwrap();
        assert_eq!(result.bytes_written, content.len() as u64);
        assert!(result.atomic_write);

        let saved_content = std::fs::read_to_string(&temp_file).unwrap();
        assert_eq!(saved_content, content);
    }

    #[test]
    fn test_save_with_bom() {
        let temp_file = NamedTempFile::new().unwrap();
        let content = "Hello with BOM!";
        let context = SaveContext {
            original_encoding: Encoding::Utf8,
            original_eol: EolType::Lf,
            original_bom: BomDetectionResult {
                encoding: Encoding::Utf8,
                bom_length: 3,
            },
            original_identity: FileIdentity {
                device_id: 0,
                inode: 0,
                size: 0,
                mtime: std::time::SystemTime::now(),
                content_hash: None,
            },
        };

        let result = save_file(&temp_file, content, &context).unwrap();

        let saved_bytes = std::fs::read(&temp_file).unwrap();
        assert_eq!(&saved_bytes[0..3], &[0xEF, 0xBB, 0xBF]); // BOM
        assert_eq!(&saved_bytes[3..], content.as_bytes());
    }

    #[test]
    fn test_save_crlf_restoration() {
        let temp_file = NamedTempFile::new().unwrap();
        let content = "Line1\nLine2\nLine3";
        let context = SaveContext {
            original_encoding: Encoding::Utf8,
            original_eol: EolType::Crlf,
            original_bom: BomDetectionResult {
                encoding: Encoding::Unknown,
                bom_length: 0,
            },
            original_identity: FileIdentity {
                device_id: 0,
                inode: 0,
                size: 0,
                mtime: std::time::SystemTime::now(),
                content_hash: None,
            },
        };

        let result = save_file(&temp_file, content, &context).unwrap();

        let saved_content = std::fs::read_to_string(&temp_file).unwrap();
        assert_eq!(saved_content, "Line1\r\nLine2\r\nLine3");
    }

    #[test]
    fn test_transcode_to_utf16le() {
        let content = "Hello! 🌍";
        let transcoded = transcode_to_encoding(content.as_bytes(), Encoding::Utf16Le).unwrap();

        // Should be even number of bytes (UTF-16)
        assert_eq!(transcoded.len() % 2, 0);
        // Should NOT include BOM (BOM is added separately in save process)
        // First two bytes should be 'H' (0x48) as little-endian UTF-16
        assert_eq!(&transcoded[0..2], &[0x48, 0x00]);
    }

    #[test]
    fn test_encode_latin1() {
        let content = "Hello, ©®";
        let encoded = encode_latin(content.as_bytes(), Encoding::Latin1).unwrap();

        assert_eq!(encoded.len(), content.chars().count());
        assert_eq!(encoded[7], 0xA9); // © in Latin-1
        assert_eq!(encoded[8], 0xAE); // ® in Latin-1
    }

    #[test]
    fn test_get_temp_path() {
        let original = Path::new("/path/to/file.txt");
        let temp = get_temp_path(original, ".tmp");

        assert_eq!(temp, Path::new("/path/to/file.txt.tmp"));
    }
}
