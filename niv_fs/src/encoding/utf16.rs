use super::Encoding;

/// Detect UTF-16 patterns based on characteristic null/data positions.
pub fn detect_utf16_pattern(bytes: &[u8]) -> Option<Encoding> {
    if bytes.len() < 32 {
        return None;
    }

    let mut even_null = 0usize;
    let mut odd_null = 0usize;
    let mut even_ascii = 0usize;
    let mut odd_ascii = 0usize;

    for (i, &b) in bytes.iter().enumerate() {
        if i % 2 == 0 {
            if b == 0 {
                even_null += 1;
            } else if (32..=126).contains(&b) {
                even_ascii += 1;
            }
        } else {
            if b == 0 {
                odd_null += 1;
            } else if (32..=126).contains(&b) {
                odd_ascii += 1;
            }
        }
    }

    let half = bytes.len() / 2;
    let even_null_ratio = even_null as f64 / half as f64;
    let odd_null_ratio = odd_null as f64 / half as f64;
    let even_ascii_ratio = even_ascii as f64 / half as f64;
    let odd_ascii_ratio = odd_ascii as f64 / half as f64;

    if even_null_ratio > 0.85 && odd_ascii_ratio > 0.4 {
        return Some(Encoding::Utf16Le);
    }
    if odd_null_ratio > 0.85 && even_ascii_ratio > 0.4 {
        return Some(Encoding::Utf16Be);
    }
    None
}
