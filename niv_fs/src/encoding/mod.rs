use std::fmt;

pub mod latin;
pub mod utf16;
pub mod utf8;
pub mod windows;

pub use latin::detect_latin_encoding;
pub use utf8::is_valid_utf8;
pub use utf16::detect_utf16_pattern;

/// Represents the detected text encoding of a file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Utf32Le,
    Utf32Be,
    Latin1,
    Windows1252,
    Latin9,
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

/// Confidence level for encoding detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DetectionConfidence {
    High = 3,
    Medium = 2,
    Low = 1,
    Unknown = 0,
}

/// Enhanced encoding detection result with confidence information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodingDetectionResult {
    pub encoding: Encoding,
    pub confidence: DetectionConfidence,
}

/// Configuration for encoding detection heuristics
#[derive(Debug, Clone, Copy)]
pub struct DetectionConfig {
    pub max_null_ratio: f64,
    pub max_control_ratio: f64,
    pub sample_size: usize,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        DetectionConfig {
            max_null_ratio: 0.1,
            max_control_ratio: 0.3,
            sample_size: 1024,
        }
    }
}

/// Detect encoding using heuristics when BOM detection fails
pub fn detect_encoding_heuristic(
    bytes: &[u8],
    config: DetectionConfig,
) -> Result<Encoding, crate::EncodingError> {
    let sample = if bytes.len() > config.sample_size {
        &bytes[..config.sample_size]
    } else {
        bytes
    };
    if sample.is_empty() {
        return Ok(Encoding::Utf8);
    }

    let mut null_count = 0;
    let mut control_count = 0;
    for &b in sample {
        if b == 0 {
            null_count += 1;
        } else if b < 32 && b != 9 && b != 10 && b != 13 {
            control_count += 1;
        }
    }
    let null_ratio = null_count as f64 / sample.len() as f64;
    let control_ratio = control_count as f64 / sample.len() as f64;
    if null_ratio > config.max_null_ratio || control_ratio > config.max_control_ratio {
        return Err(crate::EncodingError::BinaryFile);
    }

    if sample.len() >= 32 {
        if let Some(enc) = detect_utf16_pattern(sample) {
            return Ok(enc);
        }
    }
    if is_valid_utf8(sample) {
        return Ok(Encoding::Utf8);
    }
    if let Some(lat) = detect_latin_encoding(sample) {
        return Ok(lat.encoding);
    }
    Ok(Encoding::Utf8)
}

/// Heuristic detection with confidence
pub fn detect_encoding_heuristic_with_confidence(
    bytes: &[u8],
    config: DetectionConfig,
) -> Option<EncodingDetectionResult> {
    let sample = if bytes.len() > config.sample_size {
        &bytes[..config.sample_size]
    } else {
        bytes
    };
    if sample.is_empty() {
        return Some(EncodingDetectionResult {
            encoding: Encoding::Utf8,
            confidence: DetectionConfidence::Unknown,
        });
    }

    let mut null_count = 0;
    let mut control_count = 0;
    for &b in sample {
        if b == 0 {
            null_count += 1;
        } else if b < 32 && b != 9 && b != 10 && b != 13 {
            control_count += 1;
        }
    }
    let null_ratio = null_count as f64 / sample.len() as f64;
    let control_ratio = control_count as f64 / sample.len() as f64;
    if null_ratio > config.max_null_ratio || control_ratio > config.max_control_ratio {
        return None;
    }

    if sample.len() >= 32 {
        if let Some(enc) = detect_utf16_pattern(sample) {
            return Some(EncodingDetectionResult {
                encoding: enc,
                confidence: DetectionConfidence::Medium,
            });
        }
    }
    if is_valid_utf8(sample) {
        return Some(EncodingDetectionResult {
            encoding: Encoding::Utf8,
            confidence: DetectionConfidence::High,
        });
    }
    if let Some(lat) = detect_latin_encoding(sample) {
        return Some(lat);
    }
    Some(EncodingDetectionResult {
        encoding: Encoding::Utf8,
        confidence: DetectionConfidence::Unknown,
    })
}
