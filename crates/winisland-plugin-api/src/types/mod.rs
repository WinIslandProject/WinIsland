pub mod metadata;
pub mod v2;

pub const fn str_to_fixed<const N: usize>(s: &str) -> [u8; N] {
    let mut buf = [0u8; N];
    let bytes = s.as_bytes();
    let max_len = N.saturating_sub(1);
    let mut len = if bytes.len() < max_len {
        bytes.len()
    } else {
        max_len
    };
    while len < bytes.len() && len > 0 && bytes[len] & 0b1100_0000 == 0b1000_0000 {
        len -= 1;
    }
    let mut index = 0;
    while index < len {
        buf[index] = bytes[index];
        index += 1;
    }
    buf
}
