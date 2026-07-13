use taped::Tape;

#[derive(Copy, Clone)]
struct CharType(u8);

impl CharType {
    const IS_KEY_PART: Self = Self(0b0001);
    const IS_KEY_START: Self = Self(0b0010);
    const FLAGS_LEN: u32 = 2; // number of flag bits

    #[inline]
    const fn bits(self) -> u8 {
        self.0
    }

    #[inline]
    const fn with_len(self, len: u8) -> u8 {
        self.0 | (len << Self::FLAGS_LEN)
    }
}

/// One byte for every possible `u8` value.
const CHAR_TABLE: [u8; 256] = {
    let mut table = [0u8; 256];

    // Get starts
    let starts = concat!(
        "abcdefghijklmnopqrstuvwxyz",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "$",
    )
    .as_bytes();
    let mut i = 0;
    while i < starts.len() {
        table[starts[i] as usize] = CharType::IS_KEY_START.bits();
        i += 1;
    }

    // Get parts
    let parts = concat!(
        "abcdefghijklmnopqrstuvwxyz",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "0123456789",
        "-_.$",
    )
    .as_bytes();
    let mut i = 0;
    while i < parts.len() {
        table[parts[i] as usize] = CharType::IS_KEY_PART.bits();
        i += 1;
    }

    table
};

pub trait CharExt {
    /// Returns true if this character may be part of an unescaped (without `[]`) key
    /// in object notation.
    ///
    /// Keys must start with a letter or dollar sign (signalling meta-properties).
    ///
    /// Keys are case-insensitive.
    fn is_key_start(self) -> bool;

    /// Returns true if this character may be part of an unescaped (without `[]`) key
    /// in object notation.
    ///
    /// Letters, digits, dashes, underscores, dots, and dollar signs are accepted.
    /// Kebab case is used, with dots used to denote scope and dollar signs
    /// used to denote special keys.
    ///
    /// Underscores are given as alternatives to dashes as a way to keep parity with CSS
    /// if an object is used for styling, and are treated as equivalent during parsing.
    ///
    /// Keys are case-insensitive.
    fn is_key_part(self) -> bool;
}

impl CharExt for u8 {
    #[inline]
    fn is_key_part(self) -> bool {
        (CHAR_TABLE[self as usize] & CharType::IS_KEY_PART.bits()) != 0
    }

    #[inline]
    fn is_key_start(self) -> bool {
        (CHAR_TABLE[self as usize] & CharType::IS_KEY_START.bits()) != 0
    }
}

pub trait TapeExt<'a> {
    /// Consumes the object let notation key at the current position,
    /// returning it if one exists.
    ///
    /// If one does not exist, an empty slice is returned.
    ///
    /// See `CharExt` for more details.
    fn consume_key(&mut self) -> &'a [u8];
}

impl<'a> TapeExt<'a> for Tape<'a, u8> {
    fn consume_key(&mut self) -> &'a [u8] {
        if self.cur().is_none_or(|ch| !ch.is_key_start()) {
            return &self[0..0];
        }

        let start = self.pos;
        self.adv();
        let rest_len = self.consume(|ch, _| ch.is_key_part()).len();
        &self[start..start + 1 + rest_len]
    }
}
