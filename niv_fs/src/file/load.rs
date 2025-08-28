//! File loading with encoding detection, streaming, and binary guards.

use std::borrow::Cow;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use super::eol::{EolType, normalize_eol};
use super::identity::{FileIdentity, FileIdentityConfig};
use crate::bom::detect_bom;
use crate::encoding::{DetectionConfig, Encoding, detect_encoding_heuristic};

/// Configuration for file loading operations
#[derive(Debug, Clone)]
pub struct FileLoadConfig {
    /// Chunk size for streaming reads (default: 8MB)
    pub chunk_size: usize,
    /// Maximum line length before considering file "huge" (default: 1MB)
    pub max_line_length: usize,
    /// Whether to use memory mapping for large read-only files
    pub use_mmap: bool,
    /// Encoding detection configuration
    pub encoding_config: DetectionConfig,
    /// File identity configuration
    pub identity_config: FileIdentityConfig,
}

impl Default for FileLoadConfig {
    fn default() -> Self {
        FileLoadConfig {
            chunk_size: 8 * 1024 * 1024,  // 8MB
            max_line_length: 1024 * 1024, // 1MB
            use_mmap: true,
            encoding_config: DetectionConfig::default(),
            identity_config: FileIdentityConfig::default(),
        }
    }
}

/// Result of a file loading operation
#[derive(Debug)]
pub struct FileLoadResult {
    /// The loaded content (normalized to UTF-8, LF)
    pub content: String,
    /// Original encoding detected
    pub original_encoding: Encoding,
    /// Original end-of-line type
    pub original_eol: EolType,
    /// File identity for tracking renames/moves
    pub identity: FileIdentity,
    /// Whether file was opened as read-only due to binary/huge content
    pub read_only: bool,
    /// Warning messages (if any)
    pub warnings: Vec<String>,
}

/// Load a file with automatic encoding detection and normalization.
///
/// This function:
/// 1. Detects encoding via BOM + heuristics
/// 2. Checks for binary/huge file indicators
/// 3. Loads content with streaming/chunked reading
/// 4. Normalizes to UTF-8 + LF in memory
/// 5. Captures file identity for external change detection
pub fn load_file<P: AsRef<Path>>(path: P) -> Result<FileLoadResult, crate::EncodingError> {
    load_file_with_config(path, &FileLoadConfig::default())
}

/// Load a file with custom configuration.
pub fn load_file_with_config<P: AsRef<Path>>(
    path: P,
    config: &FileLoadConfig,
) -> Result<FileLoadResult, crate::EncodingError> {
    let path = path.as_ref();

    // First, capture file identity
    let identity =
        FileIdentity::from_path(path, &config.identity_config).map_err(crate::EncodingError::Io)?;

    // Check if file is too large to load entirely
    if identity.size > 100 * 1024 * 1024 {
        // 100MB threshold
        return Ok(FileLoadResult {
            content: String::new(),
            original_encoding: Encoding::Unknown,
            original_eol: EolType::Lf,
            identity,
            read_only: true,
            warnings: vec!["File too large (>100MB), opened as read-only".to_string()],
        });
    }

    // Read initial sample for encoding detection
    let mut file = File::open(path).map_err(crate::EncodingError::Io)?;
    let mut sample = vec![0u8; config.encoding_config.sample_size];
    let sample_size = file.read(&mut sample).map_err(crate::EncodingError::Io)?;

    if sample_size == 0 {
        return Ok(FileLoadResult {
            content: String::new(),
            original_encoding: Encoding::Utf8,
            original_eol: EolType::Lf,
            identity,
            read_only: false,
            warnings: vec![],
        });
    }

    let sample = &sample[..sample_size];

    // Check for binary content
    if is_binary_content(sample) {
        return Ok(FileLoadResult {
            content: String::new(),
            original_encoding: Encoding::Unknown,
            original_eol: EolType::Lf,
            identity,
            read_only: true,
            warnings: vec!["Binary file detected, opened as read-only".to_string()],
        });
    }

    // Check for extremely long lines
    if has_extremely_long_lines(sample, config.max_line_length) {
        return Ok(FileLoadResult {
            content: String::new(),
            original_encoding: Encoding::Unknown,
            original_eol: EolType::Lf,
            identity,
            read_only: true,
            warnings: vec![format!(
                "Extremely long lines detected (>{} bytes), opened as read-only",
                config.max_line_length
            )],
        });
    }

    // Detect encoding
    let bom_result = detect_bom(sample);
    let encoding = if bom_result.encoding != Encoding::Unknown {
        bom_result.encoding
    } else {
        detect_encoding_heuristic(
            &sample[bom_result.bom_length..],
            config.encoding_config.clone(),
        )?
    };

    // Load full content
    let raw_content = load_content_streaming(path, config)?;
    let raw_content = &raw_content[bom_result.bom_length..]; // Skip BOM

    // Decode content based on encoding
    let decoded_content = match encoding {
        Encoding::Utf8 => {
            String::from_utf8(raw_content.to_vec()).map_err(|_| crate::EncodingError::BinaryFile)?
        }
        Encoding::Utf16Le => decode_utf16le(raw_content)?,
        Encoding::Utf16Be => decode_utf16be(raw_content)?,
        Encoding::Utf32Le => decode_utf32le(raw_content)?,
        Encoding::Utf32Be => decode_utf32be(raw_content)?,
        Encoding::Latin1 | Encoding::Windows1252 | Encoding::Latin9 => {
            decode_latin(raw_content, encoding)
        }
        Encoding::Unknown => return Err(crate::EncodingError::BinaryFile),
    };

    // Normalize EOL
    let (normalized_content, original_eol) = normalize_eol(decoded_content.as_bytes());

    // Convert to String, handling the Cow
    let content = match normalized_content {
        Cow::Borrowed(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        Cow::Owned(vec) => String::from_utf8_lossy(&vec).into_owned(),
    };

    Ok(FileLoadResult {
        content,
        original_encoding: encoding,
        original_eol,
        identity,
        read_only: false,
        warnings: vec![],
    })
}

/// Load file content using streaming/chunked reading to avoid large allocations.
fn load_content_streaming<P: AsRef<Path>>(
    path: P,
    config: &FileLoadConfig,
) -> Result<Vec<u8>, crate::EncodingError> {
    let mut file = File::open(path).map_err(crate::EncodingError::Io)?;
    let mut content = Vec::new();
    let mut buffer = vec![0u8; config.chunk_size];

    loop {
        let bytes_read = file.read(&mut buffer).map_err(crate::EncodingError::Io)?;
        if bytes_read == 0 {
            break;
        }
        content.extend_from_slice(&buffer[..bytes_read]);
    }

    Ok(content)
}

/// Check if content appears to be binary based on null bytes and control characters.
fn is_binary_content(sample: &[u8]) -> bool {
    if sample.len() < 512 {
        return false; // Too small to determine
    }

    let mut null_count = 0;
    let mut control_count = 0;

    for &byte in sample {
        if byte == 0 {
            null_count += 1;
        } else if byte < 32 && byte != 9 && byte != 10 && byte != 13 {
            control_count += 1;
        }
    }

    let null_ratio = null_count as f64 / sample.len() as f64;
    let control_ratio = control_count as f64 / sample.len() as f64;

    // Binary if >10% null bytes or >30% control characters
    null_ratio > 0.1 || control_ratio > 0.3
}

/// Check if the file has extremely long lines that might indicate binary data.
fn has_extremely_long_lines(sample: &[u8], max_line_length: usize) -> bool {
    let mut current_line_length = 0;
    let mut max_found_length = 0;

    for &byte in sample {
        if byte == b'\n' || byte == b'\r' {
            max_found_length = max_found_length.max(current_line_length);
            current_line_length = 0;
        } else {
            current_line_length += 1;
            if current_line_length > max_line_length {
                return true;
            }
        }
    }

    max_found_length > max_line_length
}

/// Decode UTF-16LE bytes to String.
fn decode_utf16le(bytes: &[u8]) -> Result<String, crate::EncodingError> {
    if bytes.len() % 2 != 0 {
        return Err(crate::EncodingError::BinaryFile);
    }

    let u16_slice =
        unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u16, bytes.len() / 2) };

    String::from_utf16(u16_slice).map_err(|_| crate::EncodingError::BinaryFile)
}

/// Decode UTF-16BE bytes to String.
fn decode_utf16be(bytes: &[u8]) -> Result<String, crate::EncodingError> {
    if bytes.len() % 2 != 0 {
        return Err(crate::EncodingError::BinaryFile);
    }

    let mut u16_vec = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let be_bytes = [chunk[1], chunk[0]]; // Swap endianness
        u16_vec.push(u16::from_be_bytes(be_bytes));
    }

    String::from_utf16(&u16_vec).map_err(|_| crate::EncodingError::BinaryFile)
}

/// Decode UTF-32LE bytes to String.
fn decode_utf32le(bytes: &[u8]) -> Result<String, crate::EncodingError> {
    if bytes.len() % 4 != 0 {
        return Err(crate::EncodingError::BinaryFile);
    }

    let u32_slice =
        unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u32, bytes.len() / 4) };

    let mut result = String::new();
    for &code in u32_slice {
        if let Some(ch) = char::from_u32(code) {
            result.push(ch);
        } else {
            return Err(crate::EncodingError::BinaryFile);
        }
    }
    Ok(result)
}

/// Decode UTF-32BE bytes to String.
fn decode_utf32be(bytes: &[u8]) -> Result<String, crate::EncodingError> {
    if bytes.len() % 4 != 0 {
        return Err(crate::EncodingError::BinaryFile);
    }

    let mut result = String::new();
    for chunk in bytes.chunks_exact(4) {
        let be_bytes = [chunk[3], chunk[2], chunk[1], chunk[0]]; // Swap endianness
        let code = u32::from_be_bytes(be_bytes);
        if let Some(ch) = char::from_u32(code) {
            result.push(ch);
        } else {
            return Err(crate::EncodingError::BinaryFile);
        }
    }
    Ok(result)
}

/// Decode Latin encodings (Latin-1, Windows-1252, Latin-9) to UTF-8.
fn decode_latin(bytes: &[u8], encoding: Encoding) -> String {
    let mut result = String::new();

    for &byte in bytes {
        let ch = match encoding {
            Encoding::Latin1 => latin1_to_char(byte),
            Encoding::Windows1252 => windows1252_to_char(byte),
            Encoding::Latin9 => latin9_to_char(byte),
            _ => unreachable!(),
        };
        result.push(ch);
    }

    result
}

/// Convert Latin-1 byte to Unicode character.
fn latin1_to_char(byte: u8) -> char {
    if byte < 0x80 {
        byte as char
    } else {
        // Latin-1 supplementary characters
        match byte {
            0x80..=0x9F => char::from_u32(0x0080 + byte as u32 - 0x80).unwrap(),
            _ => char::from_u32(0x00A0 + byte as u32 - 0xA0).unwrap(),
        }
    }
}

/// Convert Windows-1252 byte to Unicode character.
fn windows1252_to_char(byte: u8) -> char {
    if byte < 0x80 {
        byte as char
    } else {
        // Windows-1252 specific mappings for 0x80-0x9F
        match byte {
            0x80 => '€',
            0x81 => '\u{0081}', // Unassigned
            0x82 => '‚',
            0x83 => 'ƒ',
            0x84 => '„',
            0x85 => '…',
            0x86 => '†',
            0x87 => '‡',
            0x88 => 'ˆ',
            0x89 => '‰',
            0x8A => 'Š',
            0x8B => '‹',
            0x8C => 'Œ',
            0x8D => '\u{008D}', // Unassigned
            0x8E => 'Ž',
            0x8F => '\u{008F}', // Unassigned
            0x90 => '\u{0090}', // Unassigned
            0x91 => '‘',
            0x92 => '’',
            0x93 => '"',
            0x94 => '"',
            0x95 => '•',
            0x96 => '–',
            0x97 => '—',
            0x98 => '˜',
            0x99 => '™',
            0x9A => 'š',
            0x9B => '›',
            0x9C => 'œ',
            0x9D => '\u{009D}', // Unassigned
            0x9E => 'ž',
            0x9F => 'Ÿ',
            _ => latin1_to_char(byte), // Fall back to Latin-1 for 0xA0+
        }
    }
}

/// Convert Latin-9 byte to Unicode character.
fn latin9_to_char(byte: u8) -> char {
    match byte {
        0xA4 => '€', // Euro sign (different from Latin-1)
        0xA6 => 'Š', // Latin capital letter S with caron
        0xA8 => 'š', // Latin small letter s with caron
        0xB4 => 'Ž', // Latin capital letter Z with caron
        0xB8 => 'ž', // Latin small letter z with caron
        0xBC => 'Œ', // Latin capital ligature OE
        0xBD => 'œ', // Latin small ligature oe
        0xBE => 'Ÿ', // Latin capital letter Y with diaeresis
        _ => latin1_to_char(byte),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn create_temp_file(content: &str) -> std::path::PathBuf {
        let temp_dir = env::temp_dir();
        let file_name = format!(
            "test_file_{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let temp_path = temp_dir.join(file_name);
        std::fs::write(&temp_path, content).unwrap();
        temp_path
    }

    fn cleanup_temp_file(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_load_utf8_file() {
        let temp_file = create_temp_file("Hello, UTF-8!\nSecond line");

        let result = load_file(&temp_file).unwrap();
        assert_eq!(result.content, "Hello, UTF-8!\nSecond line");
        assert_eq!(result.original_encoding, Encoding::Utf8);
        assert_eq!(result.original_eol, EolType::Lf);
        assert!(!result.read_only);

        cleanup_temp_file(&temp_file);
    }

    #[test]
    fn test_load_utf8_with_bom() {
        let mut content = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        content.extend_from_slice(b"Hello with BOM!");
        let temp_file = create_temp_file(&String::from_utf8_lossy(&content));

        let result = load_file(&temp_file).unwrap();
        assert_eq!(result.content, "Hello with BOM!");
        assert_eq!(result.original_encoding, Encoding::Utf8);

        cleanup_temp_file(&temp_file);
    }

    #[test]
    fn test_load_binary_file() {
        let binary_content = vec![0u8; 1024]; // Lots of null bytes
        let temp_file = create_temp_file(&String::from_utf8_lossy(&binary_content));

        let result = load_file(&temp_file).unwrap();
        assert!(result.read_only);
        assert!(result.warnings.iter().any(|w| w.contains("Binary file")));

        cleanup_temp_file(&temp_file);
    }

    #[test]
    fn test_load_crlf_file() {
        let temp_file = create_temp_file("Line1\r\nLine2\r\nLine3");

        let result = load_file(&temp_file).unwrap();
        assert_eq!(result.content, "Line1\nLine2\nLine3");
        assert_eq!(result.original_eol, EolType::Crlf);

        cleanup_temp_file(&temp_file);
    }

    #[test]
    fn test_is_binary_content() {
        let ascii_content = b"Hello, world! This is text.";
        assert!(!is_binary_content(ascii_content));

        let binary_content = vec![0u8; 600]; // >10% null bytes
        assert!(is_binary_content(&binary_content));

        let control_content = (0..600).map(|i| (i % 32) as u8).collect::<Vec<_>>();
        assert!(is_binary_content(&control_content));
    }

    #[test]
    fn test_has_extremely_long_lines() {
        let normal_lines = b"Short line\nAnother short line\n";
        assert!(!has_extremely_long_lines(normal_lines, 1000));

        let long_line = vec![b'A'; 2000]; // Very long line
        assert!(has_extremely_long_lines(&long_line, 1000));
    }

    #[test]
    fn test_decode_latin1() {
        let latin1_bytes = &[0x48, 0x65, 0x6C, 0x6C, 0x6F, 0xA9, 0xAE]; // "Hello©®"
        let decoded = decode_latin(latin1_bytes, Encoding::Latin1);
        assert_eq!(decoded, "Hello©®");
    }
}
