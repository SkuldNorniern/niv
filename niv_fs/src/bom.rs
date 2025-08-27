use crate::encoding::Encoding;

/// Result of BOM detection containing the detected encoding and BOM length
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BomDetectionResult {
    pub encoding: Encoding,
    pub bom_length: usize,
}

/// Detect Byte Order Mark (BOM) in the given byte slice.
pub fn detect_bom(bytes: &[u8]) -> BomDetectionResult {
    if bytes.len() < 2 {
        return BomDetectionResult { encoding: Encoding::Unknown, bom_length: 0 };
    }

    if bytes.len() >= 4 {
        if bytes[0] == 0xFF && bytes[1] == 0xFE && bytes[2] == 0x00 && bytes[3] == 0x00 {
            return BomDetectionResult { encoding: Encoding::Utf32Le, bom_length: 4 };
        }
        if bytes[0] == 0x00 && bytes[1] == 0x00 && bytes[2] == 0xFE && bytes[3] == 0xFF {
            return BomDetectionResult { encoding: Encoding::Utf32Be, bom_length: 4 };
        }
    }

    if bytes[0] == 0xFF && bytes[1] == 0xFE {
        return BomDetectionResult { encoding: Encoding::Utf16Le, bom_length: 2 };
    }
    if bytes[0] == 0xFE && bytes[1] == 0xFF {
        return BomDetectionResult { encoding: Encoding::Utf16Be, bom_length: 2 };
    }

    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return BomDetectionResult { encoding: Encoding::Utf8, bom_length: 3 };
    }

    BomDetectionResult { encoding: Encoding::Unknown, bom_length: 0 }
}



