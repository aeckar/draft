

pub trait SliceExt<'a> {
    /// Returns the top-level domain (TLD) of the link, or `None`.
    fn tld(self) -> Option<&'a [u8]>;
}

impl<'a> SliceExt<'a> for &'a [u8] {
    fn tld(self) -> Option<Self> {
        let mut dot_idx = 0;
        for (idx, &c) in self.iter().enumerate() {
            if c == b'/' {
                if idx == dot_idx + 1 {
                    panic!("Invalid URL");
                }
                return Some(&self[dot_idx + 1..idx]);
            }
            if c == b'.' {
                dot_idx = idx;
            }
        }
        None
    }
}
