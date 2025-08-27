//! End-of-line detection and normalization utilities.

use std::borrow::Cow;

/// Represents the detected end-of-line type in a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EolType {
    /// Line Feed (Unix/Linux/macOS) - \n
    Lf,
    /// Carriage Return + Line Feed (Windows) - \r\n
    Crlf,
    /// Carriage Return (old macOS) - \r
    Cr,
    /// Mixed end-of-lines detected
    Mixed,
}

/// Detect the predominant end-of-line type in the given bytes.
pub fn detect_eol(bytes: &[u8]) -> EolType {
    let mut lf_count = 0u64;
    let mut crlf_count = 0u64;
    let mut cr_count = 0u64;

    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    crlf_count += 1;
                    i += 2;
                } else {
                    cr_count += 1;
                    i += 1;
                }
            }
            b'\n' => {
                lf_count += 1;
                i += 1;
            }
            _ => i += 1,
        }
    }

    // Determine the predominant type
    let total = lf_count + crlf_count + cr_count;
    if total == 0 {
        return EolType::Lf; // Default for empty or no line endings
    }

    // Find the maximum
    if crlf_count >= lf_count && crlf_count >= cr_count {
        EolType::Crlf
    } else if lf_count >= cr_count {
        EolType::Lf
    } else {
        EolType::Cr
    }
}

/// Normalize end-of-lines to LF (\n) and return the original type.
pub fn normalize_eol(bytes: &[u8]) -> (Cow<[u8]>, EolType) {
    let original_eol = detect_eol(bytes);

    // Check if already normalized
    if matches!(original_eol, EolType::Lf | EolType::Mixed) {
        return (Cow::Borrowed(bytes), original_eol);
    }

    // Perform normalization
    let mut result = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    // CRLF -> LF
                    result.push(b'\n');
                    i += 2;
                } else {
                    // CR -> LF
                    result.push(b'\n');
                    i += 1;
                }
            }
            b'\n' => {
                result.push(b'\n');
                i += 1;
            }
            other => {
                result.push(other);
                i += 1;
            }
        }
    }

    (Cow::Owned(result), original_eol)
}

/// Restore the original end-of-line type to normalized content.
pub fn restore_eol(normalized_bytes: &[u8], original_eol: EolType) -> Cow<[u8]> {
    if matches!(original_eol, EolType::Lf | EolType::Mixed) {
        return Cow::Borrowed(normalized_bytes);
    }

    let mut result = Vec::new();
    let mut i = 0;

    while i < normalized_bytes.len() {
        if normalized_bytes[i] == b'\n' {
            match original_eol {
                EolType::Crlf => {
                    result.extend_from_slice(b"\r\n");
                }
                EolType::Cr => {
                    result.push(b'\r');
                }
                EolType::Lf | EolType::Mixed => {
                    result.push(b'\n');
                }
            }
        } else {
            result.push(normalized_bytes[i]);
        }
        i += 1;
    }

    Cow::Owned(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_eol_lf() {
        let content = b"line1\nline2\nline3";
        assert_eq!(detect_eol(content), EolType::Lf);
    }

    #[test]
    fn test_detect_eol_crlf() {
        let content = b"line1\r\nline2\r\nline3";
        assert_eq!(detect_eol(content), EolType::Crlf);
    }

    #[test]
    fn test_detect_eol_cr() {
        let content = b"line1\rline2\rline3";
        assert_eq!(detect_eol(content), EolType::Cr);
    }

    #[test]
    fn test_detect_eol_mixed() {
        let content = b"line1\nline2\r\nline3\r";
        assert_eq!(detect_eol(content), EolType::Crlf); // CRLF is most common in this case
    }

    #[test]
    fn test_normalize_eol_lf_unchanged() {
        let content = b"line1\nline2\nline3";
        let (normalized, original) = normalize_eol(content);
        assert_eq!(normalized.as_ref(), content);
        assert_eq!(original, EolType::Lf);
    }

    #[test]
    fn test_normalize_eol_crlf() {
        let content = b"line1\r\nline2\r\nline3";
        let (normalized, original) = normalize_eol(content);
        assert_eq!(normalized.as_ref(), b"line1\nline2\nline3");
        assert_eq!(original, EolType::Crlf);
    }

    #[test]
    fn test_normalize_eol_cr() {
        let content = b"line1\rline2\rline3";
        let (normalized, original) = normalize_eol(content);
        assert_eq!(normalized.as_ref(), b"line1\nline2\nline3");
        assert_eq!(original, EolType::Cr);
    }

    #[test]
    fn test_restore_eol_crlf() {
        let normalized = b"line1\nline2\nline3";
        let restored = restore_eol(normalized, EolType::Crlf);
        assert_eq!(restored.as_ref(), b"line1\r\nline2\r\nline3");
    }

    #[test]
    fn test_restore_eol_cr() {
        let normalized = b"line1\nline2\nline3";
        let restored = restore_eol(normalized, EolType::Cr);
        assert_eq!(restored.as_ref(), b"line1\rline2\rline3");
    }

    #[test]
    fn test_restore_eol_lf_unchanged() {
        let normalized = b"line1\nline2\nline3";
        let restored = restore_eol(normalized, EolType::Lf);
        assert_eq!(restored.as_ref(), normalized);
    }
}
