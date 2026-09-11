/// Fixed-capacity UTF-8 string. No heap on the hot path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TinyStr {
    bytes: [u8; 24],
    len: u8,
}

impl TinyStr {
    pub fn from_str_lossy(s: &str) -> Self {
        let bytes_in = s.as_bytes();
        let mut len = bytes_in.len().min(24);
        while len > 0 && !s.is_char_boundary(len) {
            len -= 1;
        }
        let mut bytes = [0u8; 24];
        bytes[..len].copy_from_slice(&bytes_in[..len]);
        Self { bytes, len: len as u8 }
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("")
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl AsRef<str> for TinyStr {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::TinyStr;

    #[test]
    fn truncates_to_24_bytes_on_char_boundary() {
        let s = TinyStr::from_str_lossy("sonnet-3.7-thinking-extra");
        assert_eq!(s.as_str().len(), 24);
        assert!(s.as_str().is_char_boundary(s.as_str().len()));
    }
}
