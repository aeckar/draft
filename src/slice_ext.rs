use crate::char_ext::CharExt;

pub trait SliceExt {
    /// Returns this slice as a UTF-8 string without performing validation. 
    fn to_utf8(&self) -> String;

    /// 
    fn to_utf8_trimmed(&self) -> String;
}

impl SliceExt for &[u8] {
    #[inline(always)]
    fn to_utf8(&self) -> String {
        unsafe { String::from_utf8_unchecked(self.to_vec()) }
    }

    #[inline(always)]
    fn to_utf8_trimmed(&self) -> String {
        let mut bytes = *self;

        // Trim the front: peel off while it's whitespace
        while let [first, rest @ ..] = bytes {
            if first.is_ws() {
                bytes = rest;
            } else {
                break;
            }
        }

        // Trim the back: peel off from the end
        while let [rest @ .., last] = bytes {
            if last.is_ws() {
                bytes = rest;
            } else {
                break;
            }
        }

        // Now that the slice is "shrunk," convert the remaining part
        bytes.to_utf8()
    }
}