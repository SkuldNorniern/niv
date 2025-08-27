/// Check if a byte slice contains valid UTF-8 sequences
pub fn is_valid_utf8(bytes: &[u8]) -> bool {
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            i += 1;
            continue;
        }
        if b & 0xE0 == 0xC0 {
            if i + 1 >= bytes.len() || bytes[i + 1] & 0xC0 != 0x80 {
                return false;
            }
            i += 2;
        } else if b & 0xF0 == 0xE0 {
            if i + 2 >= bytes.len() || bytes[i + 1] & 0xC0 != 0x80 || bytes[i + 2] & 0xC0 != 0x80 {
                return false;
            }
            i += 3;
        } else if b & 0xF8 == 0xF0 {
            if i + 3 >= bytes.len()
                || bytes[i + 1] & 0xC0 != 0x80
                || bytes[i + 2] & 0xC0 != 0x80
                || bytes[i + 3] & 0xC0 != 0x80
            {
                return false;
            }
            i += 4;
        } else {
            return false;
        }
    }
    true
}


