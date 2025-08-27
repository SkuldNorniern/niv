use super::{DetectionConfidence, Encoding, EncodingDetectionResult};

/// Detect Latin encodings with conservative confidence scoring.
pub fn detect_latin_encoding(bytes: &[u8]) -> Option<EncodingDetectionResult> {
    if bytes.len() < 10 {
        return None;
    }

    let mut extended = 0usize;
    let mut win1252_specific = 0usize; // 0x80..=0x9F range
    let mut latin9_specific = 0usize; // subset markers

    for &b in bytes {
        if b >= 0x80 {
            extended += 1;
        }
        if (0x80..=0x9F).contains(&b) {
            win1252_specific += 1;
        }
        if matches!(b, 0xA4 | 0xA6 | 0xA8 | 0xB4 | 0xB8 | 0xBC | 0xBD | 0xBE) {
            latin9_specific += 1;
        }
    }

    let extended_ratio = extended as f64 / bytes.len() as f64;
    if extended_ratio < 0.08 {
        return None;
    }

    if win1252_specific > 2 {
        return Some(EncodingDetectionResult {
            encoding: Encoding::Windows1252,
            confidence: DetectionConfidence::Medium,
        });
    }
    if latin9_specific > extended / 10 {
        return Some(EncodingDetectionResult {
            encoding: Encoding::Latin9,
            confidence: DetectionConfidence::Low,
        });
    }
    Some(EncodingDetectionResult {
        encoding: Encoding::Latin1,
        confidence: DetectionConfidence::Low,
    })
}


