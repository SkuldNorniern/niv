//! # niv_fs - Filesystem utilities for niv editor
//!
//! This crate provides filesystem operations with proper encoding detection,
//! atomic writes, and external change detection.
//!
//! ## Encoding Detection
//!
//! The crate supports comprehensive encoding detection including:
//! - **Unicode encodings**: UTF-8, UTF-16 (LE/BE), UTF-32 (LE/BE) with BOM detection
//! - **Latin encodings**: Latin-1 (ISO-8859-1), Windows-1252, Latin-9 (ISO-8859-15)
//! - **Binary detection**: Identifies binary files and prevents corruption
//! - **Heuristic analysis**: Smart detection of encodings without BOM markers

use std::io;
use std::fmt;

/// Represents the detected text encoding of a file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8 encoding
    Utf8,
    /// UTF-16 Little Endian
    Utf16Le,
    /// UTF-16 Big Endian
    Utf16Be,
    /// UTF-32 Little Endian
    Utf32Le,
    /// UTF-32 Big Endian
    Utf32Be,
    /// Latin-1 (ISO-8859-1) encoding
    Latin1,
    /// Windows-1252 encoding (common on Windows)
    Windows1252,
    /// Latin-9 (ISO-8859-15) encoding
    Latin9,
    /// Unknown encoding (may be binary or unsupported encoding)
    Unknown,
}

impl fmt::Display for Encoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Encoding::Utf8 => write!(f, "Utf8"),
            Encoding::Utf16Le => write!(f, "Utf16Le"),
            Encoding::Utf16Be => write!(f, "Utf16Be"),
            Encoding::Utf32Le => write!(f, "Utf32Le"),
            Encoding::Utf32Be => write!(f, "Utf32Be"),
            Encoding::Latin1 => write!(f, "Latin1"),
            Encoding::Windows1252 => write!(f, "Windows1252"),
            Encoding::Latin9 => write!(f, "Latin9"),
            Encoding::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Errors that can occur during encoding detection
#[derive(Debug)]
pub enum EncodingError {
    /// I/O error while reading file
    Io(io::Error),
    /// File appears to be binary (contains null bytes)
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

/// Result of BOM detection containing the detected encoding and BOM length
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BomDetectionResult {
    /// The detected encoding
    pub encoding: Encoding,
    /// Number of bytes to skip (BOM length)
    pub bom_length: usize,
}

/// Detect Byte Order Mark (BOM) in the given byte slice
///
/// Returns the detected encoding and BOM length if a BOM is found,
/// or Encoding::Unknown with bom_length 0 if no BOM is detected.
///
/// # Arguments
/// * `bytes` - Byte slice to examine for BOM
///
/// # Returns
/// BomDetectionResult with encoding and BOM length
pub fn detect_bom(bytes: &[u8]) -> BomDetectionResult {
    // Need at least 2 bytes for UTF-16 BOMs
    if bytes.len() < 2 {
        return BomDetectionResult {
            encoding: Encoding::Unknown,
            bom_length: 0,
        };
    }

    // UTF-32 BOMs require 4 bytes
    if bytes.len() >= 4 {
        // UTF-32 Little Endian: FF FE 00 00
        if bytes[0] == 0xFF && bytes[1] == 0xFE && bytes[2] == 0x00 && bytes[3] == 0x00 {
            return BomDetectionResult {
                encoding: Encoding::Utf32Le,
                bom_length: 4,
            };
        }

        // UTF-32 Big Endian: 00 00 FE FF
        if bytes[0] == 0x00 && bytes[1] == 0x00 && bytes[2] == 0xFE && bytes[3] == 0xFF {
            return BomDetectionResult {
                encoding: Encoding::Utf32Be,
                bom_length: 4,
            };
        }
    }

    // UTF-16 Little Endian: FF FE
    if bytes[0] == 0xFF && bytes[1] == 0xFE {
        return BomDetectionResult {
            encoding: Encoding::Utf16Le,
            bom_length: 2,
        };
    }

    // UTF-16 Big Endian: FE FF
    if bytes[0] == 0xFE && bytes[1] == 0xFF {
        return BomDetectionResult {
            encoding: Encoding::Utf16Be,
            bom_length: 2,
        };
    }

    // UTF-8 BOM: EF BB BF (requires 3 bytes)
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return BomDetectionResult {
            encoding: Encoding::Utf8,
            bom_length: 3,
        };
    }

    // No BOM detected
    BomDetectionResult {
        encoding: Encoding::Unknown,
        bom_length: 0,
    }
}

/// Configuration for encoding detection heuristics
#[derive(Debug, Clone, Copy)]
pub struct DetectionConfig {
    /// Maximum ratio of null bytes before considering file binary (0.0 to 1.0)
    pub max_null_ratio: f64,
    /// Maximum ratio of control characters before considering file binary (0.0 to 1.0)
    pub max_control_ratio: f64,
    /// Minimum number of bytes to sample for detection
    pub sample_size: usize,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        DetectionConfig {
            max_null_ratio: 0.1,      // 10% null bytes max
            max_control_ratio: 0.3,   // 30% control chars max
            sample_size: 1024,        // Sample first 1KB
        }
    }
}

/// Detect encoding using heuristics when BOM detection fails
///
/// This function performs heuristic analysis to detect:
/// - Binary files (high ratio of null/control characters)
/// - UTF-8 encoding patterns
/// - Basic UTF-16 detection
/// - Falls back to UTF-8 assumption for unknown text files
///
/// # Arguments
/// * `bytes` - Byte slice to analyze
/// * `config` - Detection configuration
///
/// # Returns
/// EncodingResult with detected encoding
pub fn detect_encoding_heuristic(bytes: &[u8], config: DetectionConfig) -> EncodingResult<Encoding> {
    // Sample the beginning of the file for analysis
    let sample = if bytes.len() > config.sample_size {
        &bytes[..config.sample_size]
    } else {
        bytes
    };

    if sample.is_empty() {
        return Ok(Encoding::Utf8); // Empty files default to UTF-8
    }

    // Count null bytes and control characters
    let mut null_count = 0;
    let mut control_count = 0;

    for &byte in sample {
        if byte == 0 {
            null_count += 1;
        } else if byte < 32 && byte != 9 && byte != 10 && byte != 13 {
            // Control characters except tab, LF, CR
            control_count += 1;
        }
    }

    let null_ratio = null_count as f64 / sample.len() as f64;
    let control_ratio = control_count as f64 / sample.len() as f64;

    // Check for binary file indicators
    if null_ratio > config.max_null_ratio || control_ratio > config.max_control_ratio {
        return Err(EncodingError::BinaryFile);
    }

    // Check for UTF-16 patterns (even bytes are mostly 0)
    if sample.len() >= 32 {
        let mut even_null_count = 0;
        let mut odd_nonzero_count = 0;

        for (i, &byte) in sample.iter().enumerate() {
            if i % 2 == 0 && byte == 0 {
                even_null_count += 1;
            } else if i % 2 == 1 && byte != 0 {
                odd_nonzero_count += 1;
            }
        }

        let even_null_ratio = even_null_count as f64 / (sample.len() / 2) as f64;
        let odd_nonzero_ratio = odd_nonzero_count as f64 / (sample.len() / 2) as f64;

        // UTF-16 LE: even bytes are mostly 0, odd bytes have data
        if even_null_ratio > 0.8 && odd_nonzero_ratio > 0.3 {
            return Ok(Encoding::Utf16Le);
        }

        // UTF-16 BE: odd bytes are mostly 0, even bytes have data
        if odd_nonzero_ratio < 0.2 && even_null_ratio < 0.2 {
            // Need to check if even bytes contain valid UTF-16 data
            // This is a simple heuristic - check for valid ASCII range
            let mut valid_ascii = 0;
            for (i, &byte) in sample.iter().enumerate() {
                if i % 2 == 0 && byte >= 32 && byte < 127 {
                    valid_ascii += 1;
                }
            }
            if valid_ascii as f64 / (sample.len() / 2) as f64 > 0.6 {
                return Ok(Encoding::Utf16Be);
            }
        }
    }

    // Check for UTF-8 validity
    if is_valid_utf8(sample) {
        return Ok(Encoding::Utf8);
    }

    // Check for Latin encodings (invalid UTF-8 but valid extended ASCII)
    if let Some(latin_encoding) = detect_latin_encoding(sample) {
        return Ok(latin_encoding);
    }

    // If we can't determine the encoding but it doesn't look binary,
    // assume UTF-8 as fallback
    Ok(Encoding::Utf8)
}

/// Check if a byte slice contains valid UTF-8 sequences
fn is_valid_utf8(bytes: &[u8]) -> bool {
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];

        if byte < 128 {
            // ASCII byte
            i += 1;
        } else if byte & 0xE0 == 0xC0 {
            // 2-byte sequence
            if i + 1 >= bytes.len() || bytes[i + 1] & 0xC0 != 0x80 {
                return false;
            }
            i += 2;
        } else if byte & 0xF0 == 0xE0 {
            // 3-byte sequence
            if i + 2 >= bytes.len() ||
               bytes[i + 1] & 0xC0 != 0x80 ||
               bytes[i + 2] & 0xC0 != 0x80 {
                return false;
            }
            i += 3;
        } else if byte & 0xF8 == 0xF0 {
            // 4-byte sequence
            if i + 3 >= bytes.len() ||
               bytes[i + 1] & 0xC0 != 0x80 ||
               bytes[i + 2] & 0xC0 != 0x80 ||
               bytes[i + 3] & 0xC0 != 0x80 {
                return false;
            }
            i += 4;
        } else {
            // Invalid UTF-8 lead byte
            return false;
        }
    }
    true
}

/// Detect Latin encoding patterns in the given byte slice
///
/// Analyzes byte patterns to detect Latin-1, Windows-1252, or Latin-9 encodings.
/// Returns None if no Latin encoding patterns are detected.
///
/// # Arguments
/// * `bytes` - Byte slice to analyze
///
/// # Returns
/// Some(Encoding) if a Latin encoding is detected, None otherwise
fn detect_latin_encoding(bytes: &[u8]) -> Option<Encoding> {
    if bytes.len() < 10 {
        return None;
    }

    let mut extended_ascii_count = 0;
    let mut windows1252_chars = 0;
    let mut latin9_chars = 0;
    let mut latin1_chars = 0;

    // Common character byte values for different encodings
    for &byte in bytes {
        if byte >= 0x80 {
            extended_ascii_count += 1;

            // Windows-1252 specific characters (common in Western European text)
            // 0x80-0x9F range contains Windows-1252 specific characters
            if byte >= 0x80 && byte <= 0x9F {
                windows1252_chars += 1;
            }

            // Latin-9 specific characters (includes euro symbol, etc.)
            match byte {
                0xA4 | 0xA6 | 0xA8 | 0xB4 | 0xB8 | 0xBC | 0xBD | 0xBE => {
                    // These are more common in Latin-9/Latin-1
                    latin9_chars += 1;
                }
                0xA0..=0xBF => {
                    // General extended characters
                    latin1_chars += 1;
                }
                _ => {}
            }
        }
    }

    let extended_ratio = extended_ascii_count as f64 / bytes.len() as f64;

    // Need at least 5% extended ASCII characters to consider Latin encoding
    if extended_ratio < 0.05 {
        return None;
    }

    // Determine the most likely encoding based on character patterns
    if windows1252_chars > latin9_chars && windows1252_chars > latin1_chars {
        // High ratio of Windows-1252 specific characters
        Some(Encoding::Windows1252)
    } else if latin9_chars > windows1252_chars && latin9_chars > latin1_chars {
        // High ratio of Latin-9 specific characters
        Some(Encoding::Latin9)
    } else {
        // Default to Latin-1 for general extended ASCII
        Some(Encoding::Latin1)
    }
}

/// Detect the encoding of a file from its byte content
///
/// This function first attempts BOM detection, and if no BOM is found,
/// falls back to heuristic detection to determine the encoding.
///
/// # Arguments
/// * `bytes` - Byte content of the file to analyze
/// * `config` - Optional detection configuration (uses defaults if None)
///
/// # Returns
/// EncodingResult with the detected encoding
///
/// # Examples
/// ```rust
/// use niv_fs::{detect_encoding, DetectionConfig, Encoding};
///
/// // UTF-8 content
/// let content = b"Hello, world!";
/// let encoding = detect_encoding(content, None).unwrap();
/// assert_eq!(encoding, Encoding::Utf8);
///
/// // Latin-1 content with accented characters
/// let latin1_content = b"Hello, caf\xE9!"; // café in Latin-1
/// let encoding = detect_encoding(latin1_content, None).unwrap();
/// assert_eq!(encoding, Encoding::Latin1);
/// ```
pub fn detect_encoding(bytes: &[u8], config: Option<DetectionConfig>) -> EncodingResult<Encoding> {
    let config = config.unwrap_or_default();

    // First, try BOM detection
    let bom_result = detect_bom(bytes);

    // If BOM detected a specific encoding, return it
    if bom_result.encoding != Encoding::Unknown {
        return Ok(bom_result.encoding);
    }

    // No BOM found, use heuristic detection
    detect_encoding_heuristic(bytes, config)
}

/// Detect encoding from a file path
///
/// Reads the beginning of the file and detects its encoding using the same
/// logic as detect_encoding.
///
/// # Arguments
/// * `path` - Path to the file to analyze
/// * `config` - Optional detection configuration (uses defaults if None)
///
/// # Returns
/// EncodingResult with the detected encoding
pub fn detect_encoding_from_file<P: AsRef<std::path::Path>>(
    path: P,
    config: Option<DetectionConfig>
) -> EncodingResult<Encoding> {
    use std::fs;

    let config = config.unwrap_or_default();

    // Read just enough bytes for detection
    let mut buffer = vec![0u8; config.sample_size];
    let mut file = fs::File::open(path)?;

    let bytes_read = io::Read::read(&mut file, &mut buffer)?;
    let content = &buffer[..bytes_read];

    detect_encoding(content, Some(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_bom_utf8() {
        let bom = [0xEF, 0xBB, 0xBF];
        let result = detect_bom(&bom);
        assert_eq!(result.encoding, Encoding::Utf8);
        assert_eq!(result.bom_length, 3);
    }

    #[test]
    fn test_detect_bom_utf16le() {
        let bom = [0xFF, 0xFE];
        let result = detect_bom(&bom);
        assert_eq!(result.encoding, Encoding::Utf16Le);
        assert_eq!(result.bom_length, 2);
    }

    #[test]
    fn test_detect_bom_utf16be() {
        let bom = [0xFE, 0xFF];
        let result = detect_bom(&bom);
        assert_eq!(result.encoding, Encoding::Utf16Be);
        assert_eq!(result.bom_length, 2);
    }

    #[test]
    fn test_detect_bom_utf32le() {
        let bom = [0xFF, 0xFE, 0x00, 0x00];
        let result = detect_bom(&bom);
        assert_eq!(result.encoding, Encoding::Utf32Le);
        assert_eq!(result.bom_length, 4);
    }

    #[test]
    fn test_detect_bom_utf32be() {
        let bom = [0x00, 0x00, 0xFE, 0xFF];
        let result = detect_bom(&bom);
        assert_eq!(result.encoding, Encoding::Utf32Be);
        assert_eq!(result.bom_length, 4);
    }

    #[test]
    fn test_detect_bom_no_bom() {
        let content = b"Hello, world!";
        let result = detect_bom(content);
        assert_eq!(result.encoding, Encoding::Unknown);
        assert_eq!(result.bom_length, 0);
    }

    #[test]
    fn test_detect_bom_empty() {
        let content = [];
        let result = detect_bom(&content);
        assert_eq!(result.encoding, Encoding::Unknown);
        assert_eq!(result.bom_length, 0);
    }

    #[test]
    fn test_detect_bom_short() {
        let content = [0xFF]; // Too short for any BOM
        let result = detect_bom(&content);
        assert_eq!(result.encoding, Encoding::Unknown);
        assert_eq!(result.bom_length, 0);
    }

    #[test]
    fn test_is_valid_utf8() {
        // Valid ASCII
        assert!(is_valid_utf8(b"Hello, world!"));

        // Valid UTF-8 with multi-byte sequences
        assert!(is_valid_utf8("Hello, 世界! 🌍".as_bytes()));

        // Invalid UTF-8 (incomplete sequence)
        assert!(!is_valid_utf8(&[0xC2])); // Incomplete 2-byte sequence

        // Invalid UTF-8 (wrong continuation byte)
        assert!(!is_valid_utf8(&[0xC2, 0x00])); // 0x00 is not a valid continuation byte

        // Invalid UTF-8 (wrong lead byte)
        assert!(!is_valid_utf8(&[0xFF, 0x80, 0x80])); // Invalid lead byte
    }

    #[test]
    fn test_detect_encoding_heuristic_utf8() {
        let content = b"Hello, world! This is UTF-8 text.";
        let config = DetectionConfig::default();
        let result = detect_encoding_heuristic(content, config);
        assert_eq!(result.unwrap(), Encoding::Utf8);
    }

    #[test]
    fn test_detect_encoding_heuristic_utf16le() {
        // Create UTF-16 LE content with mostly null even bytes
        let mut content = Vec::new();
        let text = "Hello"; // ASCII text in UTF-16 LE
        for byte in text.as_bytes() {
            content.push(0);     // High byte (null for ASCII)
            content.push(*byte); // Low byte
        }
        // Add more content to make the pattern clearer
        for _ in 0..50 {
            content.push(0);
            content.push(65); // 'A'
        }
        let config = DetectionConfig {
            max_null_ratio: 0.6, // Allow higher null ratio for UTF-16
            max_control_ratio: 0.3,
            sample_size: 1024,
        };
        let result = detect_encoding_heuristic(&content, config);
        assert_eq!(result.unwrap(), Encoding::Utf16Le);
    }

    #[test]
    fn test_detect_encoding_heuristic_binary() {
        // Create binary-like content with many nulls
        let content = [0u8; 200]; // 200 null bytes
        let config = DetectionConfig::default();
        let result = detect_encoding_heuristic(&content, config);
        assert!(matches!(result, Err(EncodingError::BinaryFile)));
    }

    #[test]
    fn test_detect_encoding_heuristic_empty() {
        let content = [];
        let config = DetectionConfig::default();
        let result = detect_encoding_heuristic(&content, config);
        assert_eq!(result.unwrap(), Encoding::Utf8);
    }

    #[test]
    fn test_detect_encoding_with_bom() {
        // UTF-8 BOM + content
        let mut content = vec![0xEF, 0xBB, 0xBF];
        content.extend_from_slice(b"Hello, world!");
        let result = detect_encoding(&content, None);
        assert_eq!(result.unwrap(), Encoding::Utf8);
    }

    #[test]
    fn test_detect_encoding_no_bom() {
        let content = b"Hello, world! This is regular UTF-8 text.";
        let result = detect_encoding(content, None);
        assert_eq!(result.unwrap(), Encoding::Utf8);
    }

    #[test]
    fn test_detect_encoding_binary() {
        // Create binary content
        let content = [0u8; 200];
        let result = detect_encoding(&content, None);
        assert!(matches!(result, Err(EncodingError::BinaryFile)));
    }

    #[test]
    fn test_detection_config_default() {
        let config = DetectionConfig::default();
        assert_eq!(config.max_null_ratio, 0.1);
        assert_eq!(config.max_control_ratio, 0.3);
        assert_eq!(config.sample_size, 1024);
    }

    #[test]
    fn test_detect_encoding_custom_config() {
        let mut content = [0u8; 100];
        // Add some non-null bytes to reduce null ratio to 90% (90 nulls out of 100)
        for i in 90..100 {
            content[i] = 65; // 'A'
        }
        let config = DetectionConfig {
            max_null_ratio: 0.95, // Allow up to 95% nulls
            max_control_ratio: 0.3,
            sample_size: 1024,
        };
        let result = detect_encoding(&content, Some(config));
        assert_eq!(result.unwrap(), Encoding::Utf8); // Should not be detected as binary
    }

    #[test]
    fn test_detect_encoding_heuristic_latin1() {
        // Create Latin-1 content with extended ASCII characters
        let mut content = b"Hello, world! ".to_vec();
        // Add common Latin-1 characters (0xA0 = non-breaking space, 0xE9 = é, etc.)
        content.extend_from_slice(&[0xA0, 0xE9, 0xE0, 0xE8, 0xF1, 0xFC]);
        content.extend_from_slice(b" This is Latin-1 text with accented characters.");

        let config = DetectionConfig::default();
        let result = detect_encoding_heuristic(&content, config);
        assert_eq!(result.unwrap(), Encoding::Latin1);
    }

    #[test]
    fn test_detect_encoding_heuristic_windows1252() {
        // Create Windows-1252 content with specific characters
        let mut content = b"Hello, Windows ".to_vec();
        // Add Windows-1252 specific characters (0x80-0x9F range)
        content.extend_from_slice(&[0x80, 0x82, 0x83, 0x84, 0x85, 0x86]);
        content.extend_from_slice(b" text with special characters.");

        let config = DetectionConfig::default();
        let result = detect_encoding_heuristic(&content, config);
        assert_eq!(result.unwrap(), Encoding::Windows1252);
    }

    #[test]
    fn test_detect_latin_encoding_empty() {
        let content = [];
        let result = detect_latin_encoding(&content);
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_latin_encoding_ascii_only() {
        let content = b"Hello, world! This is pure ASCII text.";
        let result = detect_latin_encoding(content.as_slice());
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_latin_encoding_few_extended() {
        // Less than 5% extended ASCII, should not detect Latin encoding
        let mut content = vec![0; 100];
        content[95] = 0xE9; // Only 1 extended character out of 100
        let result = detect_latin_encoding(content.as_slice());
        assert!(result.is_none());
    }

    #[test]
    fn test_encoding_display() {
        assert_eq!(format!("{}", Encoding::Utf8), "Utf8");
        assert_eq!(format!("{}", Encoding::Utf16Le), "Utf16Le");
        assert_eq!(format!("{}", Encoding::Latin1), "Latin1");
        assert_eq!(format!("{}", Encoding::Windows1252), "Windows1252");
        assert_eq!(format!("{}", Encoding::Latin9), "Latin9");
        assert_eq!(format!("{}", Encoding::Unknown), "Unknown");
    }

    #[test]
    fn test_encoding_error_display() {
        let err = EncodingError::BinaryFile;
        assert_eq!(format!("{}", err), "File appears to be binary");

        let err = EncodingError::FileTooLarge;
        assert_eq!(format!("{}", err), "File is too large to process");
    }

    #[test]
    fn test_bom_detection_result() {
        let result = BomDetectionResult {
            encoding: Encoding::Utf8,
            bom_length: 3,
        };
        assert_eq!(result.encoding, Encoding::Utf8);
        assert_eq!(result.bom_length, 3);
    }
}
