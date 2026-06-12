\\?\C:\Users\eckar\Downloads\repos\draft\crates\dcon-core\src\encoding.rs:

//! All `parse_X` functions assume cursor is at a valid character.
use std::{collections::HashMap, fmt::Display, str::Utf8Error};

use ordered_float::{FloatIsNan, NotNan};
use serde::{Deserialize, Serialize};
use strum::EnumDiscriminants;
use taped::{CharExt, Tape, ToTape};
use thiserror::Error;
use unindent::unindent;

use crate::prelude::*;

/// A not-NaN floating-point representation used for storing numbers in object notation.
pub type Number = NotNan<f64>;

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    derive_more::Deref,
    derive_more::Display,
)]
#[serde(transparent)]
pub struct Key(pub(crate) String);

impl TryFrom<String> for Key {
    type Error = InvalidKey;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let mut tape = value.as_bytes().to_tape();
        tape.consume_key();
        if !tape.is_exhausted() {
            let pos = tape.pos; // satisfy borrow checker
            return Err(InvalidKey { id: value, pos });
        }
        Ok(Self(value))
    }
}

impl TryFrom<&str> for Key {
    type Error = InvalidKey;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_owned())
    }
}

#[derive(Debug, Error)]
#[error("Illegal character at index {pos} for key '{id}'")]
pub struct InvalidKey {
    id: String,
    pos: usize,
}

/// Describes and locates a specific error in object notation syntax.
#[derive(Error, Debug, Clone)]
pub enum Error {
    #[error("Expected a value at index {pos}")]
    MissingValue { pos: usize },

    #[error("Illegal character '{ch}' at index {pos}")]
    IllegalCharacter { ch: u8, pos: usize },

    #[error("Invalid number")]
    InvalidNumber { pos: usize, cause: String },

    #[error("Number is NaN")]
    NumberIsNan { pos: usize },

    #[error("Invalid UTF-8")]
    InvalidUtf8(#[from] Utf8Error),

    #[error("Expected a closing '{close}' for '{open}' at {open_pos}")]
    MissingCloser {
        open: u8,
        close: u8,
        open_pos: usize,
    },
}

/// An instance of a data object.
///
/// Roughly reflects JSON data types. Numbers **must** start with a digit, `+`, or `-`.
/// Unlike standard JSON, allows for trailing commas.
///
/// All numbers follow  IEEE 754 64-bit floating-point format, including
/// the infinities (`inf|infinity|+inf|+infinity|-inf|-infinity`) and not-a-number
/// (`nan`, case insensitive).
///
/// Strings may be enclosed using either `'` or `"`, and may contain newlines.
/// `\` can be used to escape the next byte in the sequence. Leading and trailing first newlines
/// are removed, as well as any recognized indentation.
///
/// The `fmt` (and as a result, `to_string`) implementations emit the
/// most concise object notation possible. Pretty printing is supported via the
/// `pfmt` and `to_pstring` functions. Strings are always enclosed using `"`.
///
/// # Implementation
///
/// Canonical representation of data objects is determined first by readability,
/// then by conciseness, and finally by orthogonality. For example, list items
/// are presented on their own line, which makes them most easily recognized.
///
/// No strict size limits are enforced for strings, lists, and maps.
/// This is done to maintain simplicity, and is unlikely to be an issue in real-world examples
/// as configurations are terse by design.
///
/// Since object notation is relatively small compared to markup, we skip `simdutf8`
/// for UTF-8 validation. Instead, we give callers that responsibility (except for slices).
#[derive(Debug, Clone, PartialEq, Eq, EnumDiscriminants, derive_more::From)]
#[strum_discriminants(name(ObjectKind))]
pub enum Object {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    List(Vec<Object>),
    Map(HashMap<Key, Object>),
}

/// Not intended for 64-bit or 128-bit integers,
/// since they do not support lossless conversion to `f64`.
///
/// To convert these into [Object][`crate::object::Object`],
/// users must convert to `f64` first (possibly lossy).
macro_rules! impl_from_exact_int {
    ($($t:ty),+) => {
        $(
            impl From<$t> for Object {
                fn from(value: $t) -> Self {
                    // Safe: integer types are never NaN
                    Self::Number(unsafe { NotNan::new_unchecked(value as f64) })
                }
            }
        )+
    };
}

impl_from_exact_int!(u8, u16, u32, i8, i16, i32);

impl TryFrom<f64> for Object {
    type Error = FloatIsNan;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Ok(Self::Number(NotNan::new(value)?))
    }
}

impl From<&str> for Object {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl TryFrom<HashMap<String, Object>> for Object {
    type Error = InvalidKey;

    fn try_from(value: HashMap<String, Object>) -> Result<Self, Self::Error> {
        let props = value
            .into_iter()
            .map(|(k, v)| Ok((k.try_into()?, v)))
            .collect::<Result<_, _>>()?;
        Ok(Self::Map(props))
    }
}

impl Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(cond) => write!(f, "{cond}"),
            Self::Number(n) => write!(f, "{n}"),
            Self::String(str) => write!(f, "\"{str}\""),
            Self::List(items) => {
                write!(f, "[")?;
                for (idx, item) in items.iter().enumerate() {
                    write!(f, "{item}")?;
                    if idx != items.len() - 1 {
                        write!(f, ",")?;
                    }
                }
                write!(f, "]")
            }
            Self::Map(props) => {
                write!(f, "{{")?;

                // Sort keys for deterministic output
                let mut keys: Vec<&Key> = props.keys().collect();
                keys.sort_unstable();

                for (idx, key) in keys.iter().enumerate() {
                    let val = &props[*key];
                    write!(f, "{key}={val}")?;
                    if idx != keys.len() - 1 {
                        write!(f, ",")?;
                    }
                }
                write!(f, "}}")
            }
        };
        Ok(())
    }
}

impl Object {
    pub fn new(notation: &str) -> Result<Self, Error> {
        Self::parse_any(&mut notation.as_bytes().to_tape())
    }

    #[must_use]
    fn parse_any(tape: &mut Tape<'_, u8>) -> Result<Object, Error> {
        let start = tape.pos;

        // Trivial cases
        if tape.cur().is_none() {
            return Err(Error::MissingValue { pos: start });
        }
        if tape.is_at(b"true") {
            tape.pos += "true".len();
            return Ok(Object::Bool(true));
        }
        if tape.is_at(b"false") {
            tape.pos += "false".len();
            return Ok(Object::Bool(false));
        }
        if tape.is_at(b"null") {
            tape.pos += "null".len();
            return Ok(Object::Null);
        }
        if tape.is_at(b"inf") {
            tape.pos += "inf".len();
            return Ok(Object::Number(unsafe {
                NotNan::new_unchecked(f64::INFINITY)
            }));
        }
        if tape.is_at(b"infinity") {
            tape.pos += "infinity".len();
            return Ok(Object::Number(unsafe {
                NotNan::new_unchecked(f64::INFINITY)
            }));
        }

        // Everything else
        let ch = tape.cur().unwrap();
        match ch {
            b'{' => Self::parse_map(tape),
            b'[' => Self::parse_list(tape),
            b'"' => Self::parse_string(tape, b'"'),
            b'\'' => Self::parse_string(tape, b'\''),
            b'-' | b'+' | b'0'..=b'9' => {
                let pos = tape.pos;
                let n = str::from_utf8(tape.consume(|ch, _| ch != b'\n'))?.parse::<f64>();
                if n.is_err() {
                    return Err(Error::InvalidNumber {
                        pos,
                        cause: n.unwrap_err().to_string(),
                    });
                }
                NotNan::new(n.unwrap())
                    .map_err(|_| Error::InvalidNumber {
                        pos,
                        cause: "Number is NaN".to_string(),
                    })
                    .map(|n| Object::Number(n))
            }
            b';' => {
                // same comment style as markup
                Err(Error::MissingValue { pos: start })
            }
            _ => Err(Error::IllegalCharacter { ch, pos: start }),
        }
    }

    /// Parse a single- or multi-line quoted string.
    ///
    /// Advances `tape` past the closing delimiter.
    /// Supports `\"` / `\'` escape sequences; a raw newline is legal inside
    /// the string (multiline mode).  When a newline is found, the raw body is
    /// fed through `process_multiline_string` to strip common indentation
    /// and surrounding blank lines.
    #[must_use]
    fn parse_string(tape: &mut Tape<'_, u8>, delim: u8) -> Result<Object, Error> {
        let open_pos = tape.pos;
        tape.adv(); // skip opening delimiter
        let body_start = tape.pos;
        let mut escaped = false;
        loop {
            match tape.cur() {
                None => {
                    return Err(Error::MissingCloser {
                        open: delim,
                        close: delim,
                        open_pos,
                    });
                }
                Some(b'\\') => {
                    escaped = !escaped; // cancels escape on next byte
                    tape.adv();
                }
                Some(ch) if ch == delim && !escaped => {
                    // found the unescaped closing delimiter
                    let raw = std::str::from_utf8(&tape[body_start..tape.pos])?;
                    let value = if raw.contains('\n') {
                        // multiline: strip common indent and surrounding blank lines
                        Object::String(unindent(raw))
                    } else {
                        Object::String(raw.to_owned())
                    };
                    tape.adv(); // skip closing delimiter
                    return Ok(value);
                }
                _ => {
                    escaped = false;
                    tape.adv();
                }
            }
        }
    }

    #[must_use]
    fn parse_map(tape: &mut Tape<'_, u8>) -> Result<Object, Error> {
        if tape.cur() != Some(b'{') {
            // should not be checked beforehand
            return Err(Error::IllegalCharacter {
                ch: tape.cur().unwrap_or(0),
                pos: tape.pos,
            });
        }
        let open_pos = tape.pos;
        tape.adv(); // skip '{'
        tape.consume(|ch, _| ch.is_simple_ws());
        let mut map = HashMap::new();
        loop {
            // Allow leading, trailing, and mixed/chained delimiters
            tape.consume(|ch, _| ch.is_simple_ws() || ch == b'\n' || ch == b',');

            // Get current character
            if tape.cur().is_none() {
                return Err(Error::MissingCloser {
                    open: b'{',
                    close: b'}',
                    open_pos,
                });
            }
            let ch = tape[tape.pos];
            if ch == b'}' {
                tape.adv();
                break;
            }

            // Get key
            let key = str::from_utf8(tape.consume_key())?;
            if key.is_empty() {
                return Err(Error::IllegalCharacter { ch, pos: tape.pos });
            }

            // Parse assignment
            if key.chars().last() == Some('.') && tape.cur() == Some(b'{') {
                tape.dec(); // align with '.'
                let Object::Map(inner) = Self::parse_map(tape)? else {
                    unreachable!()
                };
                for (mut k, v) in inner {
                    // flatten keys
                    k.0.insert_str(0, key);
                    map.insert(k, v);
                }
                continue;
            }
            tape.consume(|ch, _| ch.is_simple_ws());
            if tape.cur() != Some(b':') {
                return Err(Error::IllegalCharacter {
                    ch: tape.cur().unwrap_or(0),
                    pos: tape.pos,
                });
            }
            tape.adv(); // skip ':'
            tape.consume(|ch, _| ch.is_simple_ws());
            map.insert(Key(key.to_owned()), Self::parse_any(tape)?);
        }
        Ok(Object::Map(map))
    }

    #[must_use]
    fn parse_list(tape: &mut Tape<'_, u8>) -> Result<Object, Error> {
        let mut items = vec![];
        loop {
            tape.consume(|ch, _| ch.is_simple_ws() || ch == b'\n');
            if tape.cur() == Some(b']') {
                tape.adv();
                break;
            }
            if tape.cur().is_none() {
                return Err(Error::MissingCloser {
                    open: b'[',
                    close: b']',
                    open_pos: tape.pos,
                });
            }
            items.push(Self::parse_any(tape)?);
            tape.consume(|ch, _| ch.is_simple_ws() || ch == b'\n');
            if tape.cur() == Some(b',') {
                tape.adv();
            } else if tape.cur() != Some(b']') {
                return Err(Error::IllegalCharacter {
                    ch: tape.cur().unwrap_or(0),
                    pos: tape.pos,
                });
            }
        }
        Ok(Object::List(items))
    }

    pub fn to_pstring(&self) -> String {
        let mut buf = String::new();
        self.pfmt(&mut buf, 0).unwrap();
        buf
    }

    pub fn pfmt(&self, f: &mut dyn std::fmt::Write, depth: usize) -> std::fmt::Result {
        let indent = " ".repeat(depth * 4);
        let next_indent = " ".repeat((depth + 1) * 4);
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "\"{s}\""),
            Self::List(items) => {
                if items.is_empty() {
                    return write!(f, "[]");
                }
                writeln!(f, "[")?;
                for item in items {
                    write!(f, "{next_indent}")?;
                    item.pfmt(f, depth + 1)?;
                    writeln!(f, ",")?; // use trailing comma
                }
                write!(f, "]")
            }
            Self::Map(props) => {
                if props.is_empty() {
                    return write!(f, "{{}}");
                }
                if props.len() == 1 {
                    let (key, val) = props.iter().next().unwrap();
                    write!(f, "{{\n{next_indent}{key} = ")?;
                    val.pfmt(f, depth + 1)?;
                    return write!(f, ",\n{indent}}}"); // use trailing comma
                }
                writeln!(f, "{{")?;

                // Sort keys for deterministic output
                let mut sorted_keys: Vec<&Key> = props.keys().collect();
                sorted_keys.sort_unstable();

                let mut i = 0;
                while i < sorted_keys.len() {
                    let key = sorted_keys[i];
                    let key_parts = key.split_once('.');

                    // Unscoped keys
                    if key_parts.is_none() {
                        let val = &props[key];
                        write!(f, "{next_indent}{key} = ")?;
                        val.pfmt(f, depth + 1)?;
                        writeln!(f, ",")?;
                        i += 1;
                        continue;
                    }

                    // Find number of consecutive keys sharing this prefix
                    let (prefix, _) = key_parts.unwrap();
                    let dot_prefix = format!("{}.", prefix);
                    let mut scope_end = i + 1;
                    while scope_end < sorted_keys.len()
                        && sorted_keys[scope_end].starts_with(&dot_prefix)
                    {
                        scope_end += 1;
                    }

                    // If at least two keys share this prefix, group them as key scope
                    if scope_end - i > 1 {
                        write!(f, "{next_indent}{prefix}{{\n")?;

                        // Collect stripped keys as keys in scope
                        let mut key_scope = HashMap::new();
                        for k in &sorted_keys[i..scope_end] {
                            let stripped_key = k.strip_prefix(&dot_prefix).unwrap().to_string();
                            key_scope.insert(stripped_key, props[*k].clone());
                        }

                        // Format key-value pairs with stripped keys
                        let mut scoped_keys: Vec<_> = key_scope.keys().collect();
                        scoped_keys.sort_unstable();
                        let inner_indent = " ".repeat((depth + 2) * 4);
                        for ik in scoped_keys {
                            write!(f, "{inner_indent}{ik} = ")?;
                            key_scope[ik].pfmt(f, depth + 2)?;
                            writeln!(f, ",")?;
                        }

                        writeln!(f, "{next_indent}}},")?;
                        i = scope_end;
                    }
                }

                write!(f, "{indent}}}")
            }
        }
    }
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\dcon-core\src\ext.rs:

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
\\?\C:\Users\eckar\Downloads\repos\draft\crates\dcon-core\src\lib.rs:

mod encoding;
mod ext;
mod schema;

#[cfg(feature = "macros")]
mod macros;

#[cfg(feature = "serde")]
mod serde;

pub use self::{encoding::*, macros::*, schema::*};

pub mod prelude {
    pub use super::ext::*;
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\dcon-core\src\macros.rs:

// ============================================================
//  ty!  —  construct a Constraint from type notation
//  schema!  —  construct a Map Constraint (schema definition)
//  __obj!  —  construct an Object from literal notation
// ============================================================
//
//  Syntax quick-reference
//  ─────────────────────────────────────────────────────────
//  ty!
//    null                    → Constraint::Null
//    bool                    → Constraint::Bool
//    number                  → Constraint::Number (unbounded)
//    number(lo..hi)          → Constraint::Number { range: lo..hi }
//    number(lo..=hi)         → Constraint::Number { range: lo..=hi }
//    str                     → Constraint::String (accepts anything)
//    str(/pattern/)          → Constraint::String { validator: Regex }
//    [_]                     → Constraint::List { length: None }
//    [_ ; N]                 → Constraint::List { length: Some(N) }
//    { pat = ty, … }         → Constraint::Map(…)
//    A | B                   → Constraint::Union(vec![A, B])
//                              (right-associative; chains correctly)
//
//  schema!  —  sugar for a top-level map constraint
//    schema! { key = ty!-expr, … }
//
//  __obj!
//    null                    → Object::Null
//    true / false            → Object::Bool(…)
//    <numeric literal>       → Object::Number(…)
//    "…" / '…'              → Object::String(…)
//    [ expr, … ]            → Object::List(…)     ← NEW: [] for lists
//    { key = expr, … }      → Object::Map { map }  ← NEW: {} for maps
// ============================================================

/*
Because macros expand in the caller's crate, so unqualified std might not resolve if they're using #![no_std] or have a conflicting name in scope. ::std anchors to the crate root, making the path unambiguous regardless of where the macro is used.
 */

use std::sync::LazyLock;

use ordered_float::NotNan;
use regex::Regex;

use crate::Object;

pub static ANY_STRING: LazyLock<Regex> = LazyLock::new(|| Regex::new(".*").unwrap());

/// Used to convert Rust literals to [Object][`crate::Object`] in builder macros.
///
/// Unlike normal conversions, these panic on
pub(crate) trait Literal {
    fn into_obj(self) -> Object;
}

impl Literal for f64 {
    fn into_obj(self) -> Object {
        Object::Number(NotNan::new(self).expect("Number must not be NaN"))
    }
}

impl Literal for &str {
    fn into_obj(self) -> Object {
        Object::String(self.to_owned())
    }
}

impl Literal for u64 {
    fn into_obj(self) -> Object {}
}

impl Literal for i64 {
    fn into_obj(self) -> Object {}
}

macro_rules! impl_literal_exact_int {
    ($($t:ty),+) => {
        $(
            impl Literal for $t {
                fn into_obj(self) -> Object {
                    // Safe: integer types are never NaN
                    Object::Number(unsafe { NotNan::new_unchecked(self as f64) })
                }
            }
        )+
    };
}

impl_literal_exact_int!(u8, u16, u32, i8, i16, i32);

/// Construct a [`Constraint`] using type-notation syntax.
///
/// # Examples
///
/// ```rust
/// let c = ty!(bool | number | null);
/// let c = ty!(/^[a-z]+$/);
/// let c = ty!(0.0..100.0);
/// let c = ty!([4;]);  // list of exactly 4 elements
/// let c = ty!({ name = str, age = number });
/// ```
#[macro_export]
macro_rules! ty {//todo rename
    // Atomic types
    (any $(,)?) => { $crate::ObjectSpec::Any };
    (null $(,)?) => { $crate::ObjectSpec::Null };
    (bool $(,)?) => { $crate::ObjectSpec::Bool };
    (true $(,)?) => { $crate::ObjectSpec::True };
    (false $(,)?) => { $crate::ObjectSpec::False };
    (number $(,)?) => { $crate::ObjectSpec::Number };
    (string $(,)?) => { $crate::ObjectSpec::String };

    // Range
    // lo => hi
    ($lo:expr => $hi:expr $(,)?) => {
        $crate::ObjectSpec::Range {
            start: ::ordered_float::NotNan::new($lo as f64).unwrap(),
            end: ::ordered_float::NotNan::new($hi as f64).unwrap(),
        }
    };

    // Exact string
    // Must use double quotes due to Rust convention
    ($expect:literal $(,)?) => {
        $crate::ObjectSpec::ExactString($expect)
    };

    // Pattern
    // r"pat"
    // Must use double quotes due to Rust convention
    (r$pat:literal $(,)?) => {
        $crate::ObjectSpec::Pattern(::regex::Regex::new($pat).expect(concat!("Invalid regex: ", $pat)))
    };

    // List
    ($ty:tt[] $(,)?) => {
        $crate::ObjectSpec::List { ty: Box::new(ty!($ty)) }
    };

    // Sized list
    ([$ty:tt; $n:expr] $(,)?) => {
        $crate::ObjectSpec::SizedList { ty: Box::new(ty!($ty)), length: $n as usize }
    };

    // Tuple
    ([$($slot:tt),* $(,)?] $(,)?) => {
        $crate::ObjectSpec::Tuple {
            slots: vec![ $( ty!($slot) ),+ ],
        }
    };

    // Map with key-value constraints
    ({ $($key:literal $( ? $opt:tt )? : $ty:tt $(| $rest:tt)* ),* $(,)? } $(,)?) => {
        {
            let mut map = $crate::Object::MapProps::new();
            $(
                let pattern = if $key == "_" {  // validated after match
                    ANY_STRING
                } else {
                    ::regex::Regex::new($key).expect(concat!("Invalid key regex: ", $key))
                };
                map.insert(KeySpec {
                    is_optional: false $( || { let _ = stringify!($opt); true } )?,
                    pattern
                }, Box::new(ty!($ty $(| $rest)*)));
            )*
            $crate::ObjectSpec::Map(map)
        }
    };

    // Constraint union
    // Right-associative
    // Flattens nested Union arms so `A | B | C` => `Union([A, B, C])`
    ($head:tt | $($tail:tt)|+ $(,)?) => {{
        let lhs = ty!($head);
        let rhs = ty!($($tail)|+);
        match (lhs, rhs) {
            ($crate::ObjectSpec::Union(mut a), $crate::ObjectSpec::Union(b)) => {
                a.extend(b);
                $crate::ObjectSpec::Union(a)
            }
            ($crate::ObjectSpec::Union(mut a), rhs) => {
                a.push(rhs);
                $crate::ObjectSpec::Union(a)
            }
            (lhs, $crate::ObjectSpec::Union(mut b)) => {
                b.insert(0, lhs);
                $crate::ObjectSpec::Union(b)
            }
            (lhs, rhs) => $crate::ObjectSpec::Union(vec![lhs, rhs]),
        }
    }};
}

/// Constructs an [ObjectSpec::Map][`crate::ObjectSpec::Map`] (a schema definition).
///
/// # Examples
///
/// ```rust
/// let s = schema! {
///     name: string,
///     age: 0.0 => 150.0,
///     active: bool,
/// };
/// ```
/// # Implementation
///
/// Implemented as sugar over over `ty!({ … })`.
#[macro_export]
macro_rules! schema {
    ($($key:literal $( ? $opt:tt )? : $ty:tt $(| $rest:tt)* ),* $(,)?) => {
        ty!({ $($key $( ? $opt )? : $ty $(| $rest)* ),* })
    };
}

/// Construct an [Object][`crate::Object`] from literal notation.
///
/// # Examples
///
/// ```rust
/// let o = __obj!(null);
/// let o = __obj!(true);
/// let o = __obj!(42.0);
/// let o = __obj!("hello");
/// let o = __obj!(["a", "b", "c"]);         // [] = list
/// let o = __obj!({ "x" = 1.0, "y" = 2.0 }); // {} = map
/// ```
#[doc(hidden)]
#[macro_export]
macro_rules! __obj {
    // Symbolic constants
    (null) => { $crate::Object::Null };
    (true) => { $crate::Object::Bool(true) };
    (false) => { $crate::Object::Bool(false) };

    // Number | String
    ($lit:literal) => {
        {
            use $crate::object::Literal;

            $lit.into_obj()
        }
    };

    (@float $lit:literal) => {
        $crate::Object::try_from($lit)
            .expect(concat!("Number must not be NaN: ", stringify!($lit)))
    };

    (@wide_int $lit:literal) => {
        {
            // Maximum/minimum exact integers for `f64`
            const MAX_EXACT: i64 = 9_007_199_254_740_991;
            const MIN_EXACT: i64 = -9_007_199_254_740_991;

            if $lit >= MIN_EXACT && $lit <= MAX_EXACT {
                $crate::Object::from(
                    unsafe { ::ordered_float::NotNan::new_unchecked($lit as f64) }
                )
            } else {
                Err("Value is too large/small to be represented losslessly in f64: ")
            }
            $crate::Object::try_from($lit)
                .expect(concat!("Number must not be NaN: ", stringify!($lit)))
        }
    };

    // List
    ([ $($item:tt),* $(,)? ]) => {
        $crate::Object::List(vec![ $( __obj!($item) ),* ])
    };

    // Map
    ({ $($($key:tt).* : $val:tt),* $(,)? }) => {
        {

            let mut props = ::std::collections::HashMap::new();
            $(
                props.insert(
                    stringify!($($key).*)
                        .try_into()
                        .expect(concat!("Invalid key: ", stringify!($($key).*))),
                    __obj!($val)
                );
            )*
            $crate::Object::Map(props)
        }
    };
}

/// Constructs a basic [Object][`crate::Object`] from literal notation.
///
/// For lists and maps, use [list][`crate::list`] and [map][`crate::map`], respectively.
///
/// # Examples
///
/// ```rust
/// let o = obj!(null);
/// let o = obj!(true);
/// let o = obj!(42.0);
/// let o = obj!("hello");
/// ```
macro_rules! obj {
    // Symbolic constants
    (null $(,)?) => {
        __obj!(null)
    };
    (true $(,)?) => {
        __obj!(true)
    };
    (false $(,)?) => {
        __obj!(false)
    };

    // Number | String
    ($lit:literal $(,)?) => {
        __obj!($lit)
    };
}

/// Constructs an [`Object::List`][`crate::Object::List`].
///
/// # Examples
///
/// ```rust
/// let l = list![3.14, ["x", "y"], { dest: "." }];
/// let l = list![];    // empty
/// ```
#[macro_export]
macro_rules! list {
    // List
    ($($item:tt),* $(,)?) => { __obj!([ $($item),* ]) };
}

/// Constructs an [`Object::Map`][`crate::Object::Map`].
///
/// Unlike in object notation, commas must be used between **every** property.
///
/// Identifiers with dots are allowed,
/// however those with dollar signs must be escaped using double quotes.
///
/// # Examples
///
/// ```rust
/// let m = map!{ "$key": 1.0, my.value: 2.0 };
/// let m = map!{}; // empty
/// ```
#[macro_export]
macro_rules! map {
    ($($($key:tt).* : $val:tt),* $(,)?) => { __obj!({ $($($key).* : $val),* }) };
}

fn n() {
    let _ = obj!(null);
    let _ = obj!(true);
    let _ = obj!("hello");
    let _ = list![];
    let mymap = map! {
        null: 3,
        b: null,
    };
    let _ = obj!(4.20);
    let _ = list![1, 2, [3, 3], { a: 3 }];
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\dcon-core\src\schema.rs:

use std::{
    hash::{Hash, Hasher},
    ops::BitOr,
};

use regex::Regex;

use crate::Number;

#[derive(Debug, Clone)]
pub struct KeySpec {
    is_optional: bool,
    pattern: Regex,
}

impl PartialEq for KeySpec {
    fn eq(&self, other: &Self) -> bool {
        self.pattern.as_str() == other.pattern.as_str()
    }
}

// Guarantee reflexivity since string equality is reflexive
impl Eq for KeySpec {}

impl Hash for KeySpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pattern.as_str().hash(state);
    }
}

/// A
///range is end exlusive
/// prefer variants to keep variant info in props, for impl simplicity and faster validation
#[derive(Debug, Clone)]
pub enum ObjectSpec {
    Any,
    Null,
    Bool,
    True,
    False,
    Number,
    Range { start: Number, end: Number },
    String,
    ExactString(String), //no &'static str, not supported by $:literal :(
    Pattern(Regex),
    List { ty: Box<Self> },
    SizedList { ty: Box<Self>, length: usize },
    Tuple { slots: Vec<Self> },
    Map(Vec<(KeySpec, ObjectSpec)>),
    Union(Vec<Self>),
}

impl BitOr for ObjectSpec {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::Union(vec![self, rhs])
    }
}

impl ObjectSpec {
    #[inline]
    pub fn nullable(self) -> Self {
        self | Self::Null
    }
}

/*
    ty!(bool | number? | { _ = "a" | 'b' | \c.\})

    schema!{
        _ = any[]
        big = {
            next = bool | number? | {

            }
            coord = [number, number]
        }
    }
*/

// use strum intostaticstr for decoration set
/*
;cite(mla)
{

}{
    name=
}
*/
\\?\C:\Users\eckar\Downloads\repos\draft\crates\dcon-core\src\serde.rs:

use std::{collections::HashMap, fmt};

use ordered_float::NotNan;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{MapAccess, SeqAccess, Visitor},
};

use crate::Object;

impl Serialize for Object {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(b) => serializer.serialize_bool(*b),
            Self::Number(n) => serializer.serialize_f64(n.into_inner()),
            Self::String(s) => serializer.serialize_str(s),
            Self::List(items) => items.serialize(serializer),
            Self::Map(map) => map.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Object {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ObjectVisitor;

        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = Object;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("any valid object notation primitive or structural type")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
                Ok(Object::Bool(v))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                NotNan::new(v as f64)
                    .map(Object::Number)
                    .map_err(|_| serde::de::Error::custom("Invalid float conversion"))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                NotNan::new(v as f64)
                    .map(Object::Number)
                    .map_err(|_| serde::de::Error::custom("Invalid float conversion"))
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                NotNan::new(v).map(Object::Number).map_err(|_| {
                    serde::de::Error::custom("NaN values are not permitted inside Object::Number")
                })
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
                Ok(Object::String(v.to_owned()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
                Ok(Object::String(v))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Object::Null)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                Deserialize::deserialize(deserializer)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Object::Null)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut vec = Vec::with_capacity(seq.size_hint().unwrap_or(0));
                while let Some(element) = seq.next_element()? {
                    vec.push(element);
                }
                Ok(Object::List(vec))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut hash_map = HashMap::with_capacity(map.size_hint().unwrap_or(0));
                while let Some((key, value)) = map.next_entry()? {
                    hash_map.insert(key, value);
                }
                Ok(Object::Map(hash_map))
            }
        }

        deserializer.deserialize_any(ObjectVisitor)
    }
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-cli\src\main.rs:

//! CAUTION: AI-generated code
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use draft_core::lex_markup::{DynConf, MarkupLexer};

#[derive(Parser, Debug)]
#[command(name = "draft")]
#[command(version = "0.1.0")]
#[command(about = "draft: High-performance programmable markup compiler.", long_about = None)]
struct Args {
    /// The input file or directory to compile.
    /// Defaults to the current directory if not provided.
    #[arg(value_name = "PATH", default_value = ".")]
    input: PathBuf,

    /// Explicitly set the output directory.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Change the working directory before running.
    #[arg(short = 'C', long, value_name = "PATH")]
    workdir: Option<PathBuf>,

    /// Override a configuration setting (e.g., -D finance-mode=true).
    /// Can be used multiple times.
    #[arg(short = 'D', value_name = "KEY=VALUE")]
    config_override: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Handle Workdir Override (like `git -C`)
    if let Some(ref new_dir) = args.workdir {
        std::env::set_current_dir(new_dir)
            .with_context(|| format!("Failed to change directory to {:?}", new_dir))?;
    }

    // 2. Logic to determine if input is File or Dir
    if args.input.is_dir() {
        println!("Processing all files in directory: {:?}", args.input);
    } else {
        println!("Processing single file: {:?}", args.input);
    }

    // 3. Process Config Overrides
    for entry in args.config_override {
        if let Some((key, value)) = entry.split_once('=') {
            println!("Overriding config: {} => {}", key, value);
        }
    }

    Ok(())
}

fn fmt() {}

fn serve() {}

// since using relatively small files, copy entire file to memory

fn build(input: PathBuf) -> Result<()> {
    let markup = MarkupLexer::new(DynConf(), mgc_conf, input);
    Ok(())
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-core\src\ext.rs:

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
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-core\src\formatter\mod.rs:


\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-core\src\lib.rs:

//! # Implementation
//!
//! `#[inline(always)]` should not be used except under extraordinary cirumstances (see `Tape`).
//! One should mark small functions that resolve to non-block expressions with `#[inline]`
//! to enable inlining from external crates. This applies to trait functions as well.
//!
//! When applicable, functions should be marked `const`.
//!
//! Import alias should only be used locally for readability, unless an `enum`
//! is used many times in the same file. Use of star import, except for `use crate::prelude::*`,
//! is discouraged.
#![feature(macro_metavar_expr)]
mod ext;

#[cfg(feature = "parse-markup")] // fixme should this be here???
pub mod markup;

#[cfg(feature = "macros")]
pub mod macros;

#[cfg(feature = "formatter")]
pub mod formatter;

pub mod prelude {
    pub use super::ext::*;
}

/// Unpacks a struct-like enum variant from a value, asserting that
/// the value matches the expected variant.
///
/// This macro expands to a `let` binding with an `else` branch that panics
/// if the pattern does not match. It supports binding variant fields by
/// name, optional aliasing, and an optional `..` to ignore remaining fields.
///
/// # Examples
/// ```rust
/// unpack!(value, MyEnum::Variant { a, b: alias, .. });
/// ```
///
/// # Panics
/// Panics if the provided instance is not the expected variant.
#[macro_export]
macro_rules! unpack {
    ($instance:expr, $variant:pat) => {
        let $variant = $instance else {
            panic!("Unpack failed: Expected {}", stringify!($variant));
        };
    };
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-core\src\macros\mod.rs:

//! currently no way to insert a macro exactly (should use value instead)

mod utils;
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-core\src\macros\utils.rs:

use crate::object::{MapProps, Object};

struct MacroInstance {
    decorations: Vec<String>,
    config: Object,
    bodies: Vec<String>,
}

struct MacroSchema {
    body_count: Option<u8>, // none for variable
    decoration_pool: &'static [&'static str],
    config_schema: MapProps,
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-core\src\markup\config.rs:

/// Dynamic configuration options set by the `\file` macro or by `config.mgon`.
///
/// These options can be changed at any point within a markup file by a macro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynConf {
    pub latex_math: bool,  // `latex` todo
    pub code_lang: String, // `code` todo
}

impl DynConf {
    /// Returns a new instance with the following configuration:
    /// ```toml
    /// latex_math = false
    /// code_lang = "txt"
    /// ```
    fn default() -> Self {
        Self {
            latex_math: false,
            code_lang: "txt".to_string(),
        }
    }
}

/// Static configuration options set using compiler flags or by `config.mgon`.
///
/// These options cannot be changed from within a markup file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticConf {
    /// If true, does not recognize inline math formatting to make writing finances easier.
    pub finance_mode: bool,

    /// If true, does not perform a first pass to ensure the input is valid UTF-8.
    ///
    /// # Safety
    /// This should only be enabled in local environments, where the user can be trusted
    /// to pass valid input to the compiler.
    pub trusted_mode: bool,

    /// If true, recognizes links without having to use link syntax.
    pub infer_links: bool,

    /// If true, paragraph spacing is always 1 (every line is the start of a unique element).
    ///
    /// This should be enabled in environments where paragraphs and list elements wrap to the next
    /// line, and are seperated by a newline.
    pub single_line_mode: bool,
}

impl StaticConf {
    /// Returns a new instance with the following configuration:
    /// ```toml
    /// finance_mode = false
    /// trusted_mode = false
    /// infer_links = true
    /// wrap_mode = false
    /// ```
    fn default() -> Self {
        Self {
            finance_mode: false,
            trusted_mode: false,
            infer_links: true,
            single_line_mode: false,
        }
    }
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-core\src\markup\lexer.rs:

use std::sync::LazyLock;

use linkify::{LinkFinder, LinkKind};
use simdutf8::basic::{self, Utf8Error};
use taped::{CharExt as TapedCharExt, SliceExt as TapedSliceExt, Tape};
use thiserror::Error;

use crate::{
    ext::SliceExt,
    markup::{
        config::{DynConf, StaticConf},
        lex::{CheckboxType, InlineFormat, ListItemKind, Numbering, Token, TokenSpan},
    },
    object::Object,
    prelude::*,
};

static LINK_FINDER: LazyLock<LinkFinder> = LazyLock::new(|| {
    let mut value = LinkFinder::new();
    value
        .kinds(&[LinkKind::Email, LinkKind::Url])
        .email_domain_must_have_dot(true)
        .url_can_be_iri(true)
        .url_must_have_scheme(false);
    value
});

const PRE_ICANN_TLD: [&[u8]; 7] = [b"com", b"org", b"net", b"int", b"edu", b"gov", b"mil"];

#[derive(Error, Debug)]
pub enum LexerError {
    #[error("Invalid UTF-8")]
    InvalidUtf8(#[from] Utf8Error),
}

#[derive(Debug)]
pub struct MarkupSyntax<'a> {
    /// The input text.
    pub input: &'a [u8],

    /// Dynamic configuration.
    pub dyn_conf: &'a DynConf,

    /// Static configuration.
    pub static_conf: &'a StaticConf,
}

impl<'a> Compile for MarkupSyntax<'a> {
    type Output = Result<Vec<TokenSpan<'a>>, LexerError>;

    fn compile(self) -> Self::Output {
        if !self.static_conf.trusted_mode {
            let this = &self;
            basic::from_utf8(this.input)?;
        }
        let tokens = self.parse_virtual_tokens();
        let mut tokens = self.parse_text_tokens(tokens);
        self.convert_bad_tokens(&mut tokens);
        tokens.pop(); // remove `Eof`
        Ok(tokens)
    }
}

impl<'a> MarkupSyntax<'a> {
    pub const fn new(dyn_conf: &'a DynConf, static_conf: &'a StaticConf, input: &'a [u8]) -> Self {
        Self {
            input,
            dyn_conf,
            static_conf,
        }
    }

    #[must_use]
    #[inline(always)]
    fn default_pgraph_spacing(&self) -> u8 {
        if self.static_conf.single_line_mode {
            1
        } else {
            2
        }
    }

    #[must_use]
    fn parse_virtual_tokens(&self) -> Vec<TokenSpan<'a>> {
        let mut scan = Scanner {
            in_alt_text: false,
            pgraph_spacing: self.default_pgraph_spacing(),
            tokens: vec![],
            open_quotes: Vec::with_capacity(2),
            open_fmts: vec![],
            data_values: vec![],
            not_a_url: vec![],
        };
        let mut tape = Tape::new(self.input);

        // Because these symbols may show up in prose,
        // we should expect them to most likely be plain text first.
        //
        // This means we should minimize the # of match arms.
        while let Some(&ch) = self.input.get(tape.pos) {
            let jump: Option<Tape<'a, u8>> = match ch {
                // ordered by expected frequency
                b'\n' => {
                    scan.pgraph_spacing = self.default_pgraph_spacing();
                    scan.emit_inplace(tape, Token::Newline, 1);
                    // Returning a positive result even though the cursor hasn't moved
                    // results in a negligible performance hit
                    // from copying the tape data structure.
                    // It's more important to maintain semantics.
                    Some(tape)
                }
                b'`' => scan.handle_btick(tape),
                b'$' => scan.handle_dollar(tape, self.static_conf.finance_mode),
                b'-' => scan.handle_dash(tape),
                b'.' => scan.handle_dot(tape, self.static_conf.infer_links),
                b'*' => scan.handle_star(tape),
                b'_' => scan.handle_fmt(tape, InlineFormat::UNDERLINE),
                b'|' => scan.handle_fmt(tape, InlineFormat::HIGHLIGHT),
                b'~' => scan.handle_fmt(tape, InlineFormat::STRIKETHROUGH),
                b'[' => scan.handle_obrac(tape),
                b']' => scan.handle_cbrac(tape),
                b'=' => scan.handle_equals(tape),
                b'"' | b'\'' => scan.handle_quote(tape, tape[tape.pos]),
                b';' => scan.handle_semi(tape),
                b'\\' => {
                    // escape character
                    let mut tape = tape;
                    if tape.pos == tape.len() - 1 {
                        None
                    } else {
                        tape.pos += 2;
                        Some(tape)
                    }
                }
                b'#' => {
                    // line comment
                    tape.seek_ch(b'\n');
                    Some(tape)
                }
                _ => None, // includes simple whitespace
            };
            if let Some(jump) = jump {
                tape = jump;
            }
            tape.adv();
        }
        scan.tokens
            .sort_unstable_by(|t1, t2| t1.start.cmp(&t2.start));
        scan.tokens
            .push(TokenSpan::new(Token::Eof, tape.len(), tape.len()));
        scan.tokens
    }

    #[must_use]
    fn parse_text_tokens(&self, tokens: Vec<TokenSpan<'a>>) -> Vec<TokenSpan<'a>> {
        let mut read = 0;
        let mut text_start = 0;
        let mut pos = 0;
        let mut result = vec![];
        while read < tokens.len() {
            // collect plaintext tokens
            let next = &tokens[read];
            if next.start == pos {
                if pos - text_start != 0 {
                    result.push(TokenSpan::new(Token::Plaintext, text_start, pos));
                }
                result.push(*next);
                read += 1;
                pos += next.len();
                text_start = pos;
            } else {
                pos += 1;
            }
        }
        result
    }

    /// Transforms malformed structures into plaintext, including:
    /// - Links/Embeds without a body
    /// - Empty headings
    /// - Empty list items
    /// - Empty quotes
    /// - Empty math blocks
    /// - Empty code blocks
    ///
    /// Malformed tokens found are marked as plaintext.
    ///
    /// Since macro expansion is handled outside of the compiler, we assume that all macro
    /// invocations produce text at this stage.
    fn convert_bad_tokens(&self, tokens: &mut Vec<TokenSpan<'a>>) {
        use Token::*;
        for i in 0..tokens.len() {
            match tokens[i].token {
                // access by index to satisfy borrow checker
                HeadingMarker { .. } | LineQuoteMarker | ListItemMarker { .. }
                    if !tokens.get(i + 1).is_some_and(|t| t.token.is_content()) =>
                {
                    tokens[i].bind_plain();
                }
                LinkMarker | EmbedMarker
                    if tokens
                        .iter()
                        .find(|t| matches!(t.token, LinkBody { .. }) || t.token.is_content())
                        .is_some_and(|t| matches!(t.token, LinkBody { .. })) =>
                {
                    tokens[i].bind_plain();
                }
                CodeBlock { body, .. } | MathBlock { body } if body.is_empty() => {
                    tokens[i].bind_plain();
                }
                BlockQuoteOpen
                    if tokens
                        .iter()
                        .find(|t| t.token == BlockQuoteClose || t.token.is_content())
                        .is_some_and(|t| t.token.is_content()) =>
                {
                    tokens[i].bind_plain();
                }
                _ => {}
            }
        }
    }
}

/// Encapsulates mutable state shared between different handlers during Pass 1.
/// Invalid UTF-8 substrings are treated as plaintext.
///
/// # Implementation
/// All `handle_X` functions assume cursor is at a valid characters.
/// Matching logic should be optimized by performing the cheapest validation first.
struct Scanner<'a> {
    /// Virtual (non-plaintext) tokens.
    tokens: Vec<TokenSpan<'a>>,

    /// The number of spaces used to distinguish between two different paragraphs.
    ///
    /// This is 1 between single-line components (such as headings) and any other type of component,
    /// and 2 for all other components.
    pgraph_spacing: u8,

    /// True if currently within alt text (validated '[').
    in_alt_text: bool,

    /// A FIFO stack of positions of the first character of openers that
    /// have been resolved but not yet paired with a closer.
    ///
    /// The first element of each pair is the flank type mask.
    open_fmts: Vec<(InlineFormat, usize)>,

    /// A FIFO stack of positions of the first character of block quote openers that
    /// have been resolved but not yet paired with a closer.
    ///
    /// Block quotes can be nested, but the characters used must match.
    ///
    /// The first element of each pair is whether double quotes were used.
    open_quotes: Vec<(bool, usize)>,

    /// Tracks dot (`.`) characters that have already been designated as not being where a
    /// URL was found.
    not_a_url: Vec<usize>,

    data_values: Vec<Object>,
}

impl<'a> Scanner<'a> {
    /// Pushes the token nside the input between the start and end indices.
    /// The end index is exclusive.
    #[inline]
    fn emit(&mut self, token: Token<'a>, start: usize, end: usize) {
        self.tokens.push(TokenSpan::new(token, start, end));
    }

    /// Pushes the token whose first character is at the current position
    /// and has the given length.
    //do not return tape for convenience, as `pos` might need to be adjusted before exiting handler.
    #[inline]
    fn emit_inplace(&mut self, tape: Tape<'a, u8>, token: Token<'a>, len: usize) {
        self.tokens
            .push(TokenSpan::new(token, tape.pos, tape.pos + len));
    }

    /// Attempts to emit a token if the character cluster
    /// belongs to a flanking token, such as an inline format or link.
    ///
    /// The current position should be the first character in the cluster.
    /// Returns `None` if a token was not emitted.
    ///
    /// If `None` is not returned, the length of `self.unclosed_pairs` is always modified
    /// and the cursor of the returned tape is left at the final character of the cluster.
    #[must_use]
    fn handle_fmt(&mut self, mut tape: Tape<'a, u8>, fmt: InlineFormat) -> Option<Tape<'a, u8>> {
        let start = tape.pos;
        let len = fmt.len();
        if tape.is_left_clear(start) && !tape.is_right_clear(tape.pos) {
            // open
            // lack of lookahead prevents bottleneck
            self.open_fmts.push((fmt, start));
            tape.pos += len - 1;
            return Some(tape);
        } else if tape.is_right_clear(start)
            && self
                .open_fmts
                .last()
                .is_some_and(|(last, _)| last.intersects(fmt))
        {
            // close
            let (open_mask, open_pos) = self.open_fmts.pop().unwrap();
            let open_len = InlineFormat::len(open_mask);
            // unsorted tokens don't matter since tokens are sorted after Pass 1
            if (fmt.bits() & open_mask.bits()).ilog2() == 1 {
                // basic pair
                self.emit(
                    Token::InlineFormat {
                        ty: open_mask,
                        twin_pos: start,
                    },
                    open_pos,
                    open_pos + len,
                );
                self.emit_inplace(
                    tape,
                    Token::InlineFormat {
                        ty: open_mask,
                        twin_pos: open_pos,
                    },
                    open_len,
                );
                tape.pos += open_len;
            } else if fmt == InlineFormat::BOLD_ITALIC && open_mask == InlineFormat::BOLD_ITALIC {
                // stop at next format marker appended to this cluster
                self.emit(
                    Token::InlineFormat {
                        ty: InlineFormat::BOLD,
                        twin_pos: start + 1,
                    },
                    open_pos,
                    open_pos + 2,
                );
                self.emit(
                    Token::InlineFormat {
                        ty: InlineFormat::ITALIC,
                        twin_pos: start,
                    },
                    open_pos + 2,
                    open_pos + 3,
                );
                self.emit_inplace(
                    tape,
                    Token::InlineFormat {
                        ty: InlineFormat::ITALIC,
                        twin_pos: open_pos + 2,
                    },
                    1,
                );
                self.emit(
                    Token::InlineFormat {
                        ty: InlineFormat::BOLD,
                        twin_pos: open_pos,
                    },
                    start + 1,
                    start + 3,
                );
            } else {
                // open_mask == InlineFormat::BOLD_ITALIC
                if fmt == InlineFormat::BOLD {
                    self.open_fmts.push((InlineFormat::ITALIC, open_pos));
                    self.emit(
                        Token::InlineFormat {
                            ty: InlineFormat::BOLD,
                            twin_pos: start,
                        },
                        open_pos + 1,
                        open_pos + 3,
                    );
                    self.emit_inplace(
                        tape,
                        Token::InlineFormat {
                            ty: InlineFormat::BOLD,
                            twin_pos: open_pos + 1,
                        },
                        2,
                    );
                } else {
                    self.open_fmts.push((InlineFormat::BOLD, open_pos));
                    self.emit(
                        Token::InlineFormat {
                            ty: InlineFormat::ITALIC,
                            twin_pos: start,
                        },
                        open_pos + 2,
                        open_pos + 3,
                    );
                    self.emit_inplace(
                        tape,
                        Token::InlineFormat {
                            ty: InlineFormat::ITALIC,
                            twin_pos: open_pos + 2,
                        },
                        1,
                    );
                }
            }
            return Some(tape);
        }
        None
    }

    /// Resolves whether a `'` or `"` character belongs to an admonition, a quote
    /// (shorthand or long-form) or plain text.
    ///
    /// Quote blocks of a different sigil can be nested once.
    /// Unlike fenced code blocks, the quote block handler does not consume
    /// inner content indiscrimantly. Instead, it behaves like a link,
    /// with inner markup being seperate from the token itself.
    #[must_use]
    fn handle_quote(&mut self, mut tape: Tape<'a, u8>, quote: u8) -> Option<Tape<'a, u8>> {
        if !tape.is_cur_prefix() {
            return None;
        }
        let start = tape.pos;
        if tape.is_at(&[quote; 2]) {
            // single-line shorthand
            self.emit_inplace(tape, Token::LineQuoteMarker, 2);
            self.pgraph_spacing = 1;
            tape.pos += 2; // consume `""`/`''`
            return Some(tape);
        }
        let delim = &[quote; 3];
        if tape.is_at(delim) {
            tape.pos += 3; // consume `"""`/`'''`
            if let Some(&(double, open_pos)) = self.open_quotes.last()
                && double == (quote == b'"')
            {
                self.emit(Token::BlockQuoteOpen, open_pos, open_pos + 3);
                self.emit_inplace(tape, Token::BlockQuoteClose, 3);
                self.open_quotes.pop();
                return Some(tape);
            }
            self.open_quotes.push((quote == b'"', start));
            return Some(tape);
        }
        None
    }

    /// Resolves whether a '[' character belongs to a link, an embed, an assignment, or plain text.
    #[must_use]
    fn handle_obrac(&mut self, mut tape: Tape<'a, u8>) -> Option<Tape<'a, u8>> {
        if self.in_alt_text {
            return None;
        }
        tape.adv(); // skip '['
        tape.poll_in_pgraph(self.pgraph_spacing, |ch, pos| {
            let next = tape[pos + 1];
            ch == b']' && (next == b'(' || next == b'[')
        })?;
        if tape.peek_back() == Some(b'!') {
            self.emit(Token::EmbedMarker, tape.pos - 1, tape.pos + 1);
        } else {
            self.emit_inplace(tape, Token::LinkMarker, 1);
        }
        self.in_alt_text = true;
        Some(tape)
    }

    /// Resolves whether a ']' character belongs to a link body, an embed body, or plain text.
    #[must_use]
    fn handle_cbrac(&mut self, mut tape: Tape<'a, u8>) -> Option<Tape<'a, u8>> {
        if !self.in_alt_text {
            return None;
        }
        let spacing = self.pgraph_spacing;
        let stop;
        let start = tape.pos;
        tape.adv(); // skip ']'
        match tape.cur() {
            Some(b'[') => stop = b']',
            Some(b'(') => stop = b')',
            _ => {
                return None;
            }
        }
        let body = tape.consume_in_pgraph(spacing, |ch, _| ch != stop);
        if body.is_empty() || tape.cur() != Some(stop) {
            return None;
        }
        if stop == b']' {
            self.emit(Token::LinkAliasBody { alias: body }, start, tape.pos + 1);
        } else {
            self.emit(Token::LinkBody { href: body }, start, tape.pos + 1);
        }
        Some(tape)
    }

    /// Resolves whether a '.' character belongs to an ordered list item,
    /// an inferred link, or plain text.
    ///
    /// Email and SMS links are too vague, so they are not inferred.
    /// All other link types are too niche.
    /// URIs without a scheme must have a suitable TLD (see `PRE_ICANN_TLD`).
    #[must_use]
    fn handle_dot(&mut self, mut tape: Tape<'a, u8>, infer_links: bool) -> Option<Tape<'a, u8>> {
        if tape.is_cur_prefix() {
            self.emit_inplace(
                tape,
                Token::ListItemMarker {
                    indent: tape.count_indent(),
                    kind: ListItemKind::Continuation,
                },
                1,
            );
            self.pgraph_spacing = 1;
            return Some(tape);
        }
        let prev = tape.peek_back();
        if prev.is_none() {
            return None;
        }
        if tape.is_prefix(tape.pos - 1) {
            self.emit(
                Token::ListItemMarker {
                    indent: tape.count_indent(),
                    kind: ListItemKind::Numbered(Numbering::from_marker(prev.unwrap())?),
                },
                tape.pos - 1,
                tape.pos + 1,
            );
            self.pgraph_spacing = 1;
            return Some(tape);
        }
        if !infer_links {
            return None;
        }
        tape.seek_back(|ch, _| ch.is_simple_ws());
        tape.adv();
        let start = tape.pos;
        let href = tape.consume(|ch, _| !ch.is_simple_ws());
        let link = LINK_FINDER.links(str::from_utf8(href).ok()?).next()?;
        if *link.kind() == LinkKind::Url
            && !link.as_str().contains("//")
            && !PRE_ICANN_TLD.contains(&link.as_str().as_bytes().tld()?)
        {
            return None;
        }
        self.emit(Token::InferredLink { href }, start, tape.pos);
        Some(tape)
    }

    /// Resolves whether a '-' character belongs to an unordered list item,
    /// a checkbox, a horizontal rule, or plain text.
    #[must_use]
    fn handle_dash(&mut self, mut tape: Tape<'a, u8>) -> Option<Tape<'a, u8>> {
        if !tape.is_cur_prefix() {
            return None;
        }
        if matches!(tape.peek(), Some(b'o') | Some(b'x') | Some(b'?')) {
            // checkbox
            tape.adv(); // skip '-'
            let marker = tape[tape.pos];
            if marker == b'o' || marker == b'x' {
                tape.peek().filter(|ch| ch.is_simple_ws())?;
            }
            self.emit_inplace(
                tape,
                Token::ListItemMarker {
                    indent: tape.count_indent(),
                    kind: ListItemKind::Checkbox(CheckboxType::from_marker(marker)?),
                },
                2,
            );
            tape.adv();
            self.pgraph_spacing = 1;
            return Some(tape); // stop at '-'
        }
        if tape.is_at(b"--") {
            tape.pos += 2 + tape.consume(|ch, _| ch == b'-').len();
            if tape
                .consume(|ch, _| ch != b'\n')
                .iter()
                .all(|ch| ch.is_simple_ws())
            {
                self.emit_inplace(tape, Token::HorizontalRule, 3);
                tape.dec();
                return Some(tape); // stop at last '-'
            } else {
                return None;
            }
        }
        self.emit_inplace(
            tape,
            Token::ListItemMarker {
                indent: tape.count_indent(),
                kind: ListItemKind::Unordered,
            },
            1,
        );
        self.pgraph_spacing = 1;
        Some(tape) // stop at '-'
    }

    /// Resolves whether a '=' character belongs to a heading or plain text.
    #[must_use]
    fn handle_equals(&mut self, mut tape: Tape<'a, u8>) -> Option<Tape<'a, u8>> {
        if !tape.is_cur_prefix() {
            return None;
        }
        let start = tape.pos;
        let marker = tape.consume_in_pgraph(1, |ch, _| ch == b'=');
        let depth = marker.len();
        if depth > Token::HEADING_MAX {
            return Some(tape); // treat as text, but skip next few '='
        }
        self.emit(Token::HeadingMarker { depth: depth as u8 }, start, tape.pos);
        self.pgraph_spacing = 1;
        tape.dec();
        Some(tape) // stop at final '='
    }

    /// Resolves whether a '$' character belongs to inline math,
    /// a dollar sign literal (if enabled), or plain text.
    #[must_use]
    fn handle_dollar(
        &mut self,
        mut tape: Tape<'a, u8>,
        finance_mode: bool,
    ) -> Option<Tape<'a, u8>> {
        let start = tape.pos;
        if tape.is_at(b"$$") {
            if !tape.is_cur_prefix() {
                return None;
            }
            tape.pos += 2; // consume '$$'
            let body_start = tape.pos + 1;
            if !tape.seek_ch3(b'\n', b'$', b'$') {
                // failed lookahead
                return None;
            }

            self.emit(
                Token::MathBlock {
                    body: &tape[body_start..tape.pos],
                },
                start,
                tape.pos + 1,
            );
            tape.pos += 2; // stop at last '$$'
        }
        if finance_mode {
            return None;
        }
        if !tape.seek_ch_in_pgraph(self.pgraph_spacing, b'$') {
            // failed lookahead
            return None; // stop at '$'
        }
        self.tokens.push(TokenSpan::new(
            Token::InlineMath {
                body: &tape[start + 1..tape.pos],
            },
            start,
            tape.pos + 1,
        ));
        Some(tape) // stop at closing '$'
    }

    /// Resolves whether a '`' character belongs to inline code or plain text.
    #[must_use]
    fn handle_btick(&mut self, mut tape: Tape<'a, u8>) -> Option<Tape<'a, u8>> {
        let start = tape.pos;
        let spacing = self.pgraph_spacing;
        if tape.is_at(b"```") {
            if !tape.is_cur_prefix() {
                return None;
            }
            tape.pos += 3; // consume '```'
            let lang = tape.consume(|ch, _| ch != b'\n');
            let body_start = tape.pos + 1; // after '\n'
            if !tape.seek_at(b"\n```") {
                // failed lookahead
                return None;
            }
            self.emit(
                Token::CodeBlock {
                    body: &tape[body_start..tape.pos],
                    lang: lang.trim_simple_ws(),
                },
                start,
                tape.pos + 1,
            );
            tape.pos += 3; // stop at last '`'
            return Some(tape);
        }
        if tape.is_at(b"``") {
            tape.adv(); // consume first '`' of open
            if !tape.seek_at_in_pgraph(spacing, b"``") {
                return Some(tape); // stop at 2nd '`'; treat as text
            }
            tape.adv(); // consume first '`' of closer
            self.emit(
                Token::InlineRawCode {
                    body: &tape[start + 2..tape.pos],
                },
                start,
                tape.pos + 1,
            );
            return Some(tape);
        }
        if !tape.seek_ch_in_pgraph(spacing, b'`') {
            // failed lookahead
            return None; // stop at '`'
        }
        self.tokens.push(TokenSpan::new(
            Token::InlineCode {
                body: &tape[start + 1..tape.pos],
            },
            start,
            tape.pos + 1,
        ));
        Some(tape) // stop at closing '`'
    }

    /// Resolves whether a `*` character belongs to a bold token,
    /// an italic token, both, or plain text.
    #[must_use]
    fn handle_star(&mut self, tape: Tape<'a, u8>) -> Option<Tape<'a, u8>> {
        if tape.is_at(b"***") {
            self.handle_fmt(tape, InlineFormat::BOLD | InlineFormat::ITALIC)
        } else if tape.is_at(b"**") {
            self.handle_fmt(tape, InlineFormat::BOLD)
        } else {
            // try for '*'
            self.handle_fmt(tape, InlineFormat::ITALIC)
        }
    }

    /// Resolves whether a `\` character belongs to an escape character or plain text.
    #[must_use]
    fn handle_bslash(&mut self, mut tape: Tape<'a, u8>) -> Option<Tape<'a, u8>> {
        if tape.pos == tape.len() - 1 {
            return None;
        }
        tape.pos += 2; // skip over `\` and character
        return Some(tape);
    }

    /// Resolves whether a `;` character belongs to a macro or plain text.
    #[must_use]
    fn handle_semi(&mut self, mut tape: Tape<'a, u8>) -> Option<Tape<'a, u8>> {
        if tape.pos == tape.len() - 1 {
            return None;
        }
        let start = tape.pos; // keep for macro handle token
        tape.adv(); // skip past ';'
        let name = tape.consume_key();
        if name.len() == 0 {
            // treat as semicolon
            return None;
        }
        let mut next_pos = tape.pos;
        let mut cur = tape.cur();
        if cur.is_none_or(|ch| ch != b'[' && ch != b'{') {
            // treat as incomplete macro
            return Some(tape); // stop at the first non-WS character after the macro name
        }
        self.tokens.push(TokenSpan::new(
            Token::MacroHandle { name },
            start,
            start + name.len() + 1,
        ));
        if cur == Some(b'(') {
            if !tape.seek_ch(b')') {
                // treat as incomplete macro
                tape.dec();
                return Some(tape); // stop before '('
            }
            tape.adv(); // skip past ')'
            self.emit(
                Token::MacroDecor {
                    body: &tape[next_pos + 1..tape.pos],
                },
                next_pos,
                tape.pos,
            );
            next_pos = tape.pos;
            cur = tape.cur();
            // stop after ')'
        }
        if cur == Some(b'[') {
            if !tape.seek_ch(b']') {
                // treat as incomplete macro
                tape.dec();
                return Some(tape); // stop before '['
            }
            tape.adv(); // skip past ']'
            self.emit(
                Token::MacroConfig {
                    body: &tape[next_pos + 1..tape.pos],
                },
                next_pos,
                tape.pos,
            );
            next_pos = tape.pos;
            cur = tape.cur();
            // stop after ']'
        }
        while cur == Some(b'{') {
            if !tape.seek_ch(b'}') {
                // treat as incomplete macro
                tape.dec();
                return Some(tape); // stop before '{'
            }
            tape.adv(); // skip past '}'
            self.emit(
                Token::MacroBody {
                    body: &tape[next_pos + 1..tape.pos],
                },
                next_pos,
                tape.pos,
            );
            next_pos = tape.pos;
            cur = tape.cur();
            // stop after '}'
        }
        Some(tape)
    }
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-core\src\markup\lexer_utils.rs:

use std::sync::OnceLock;

use bitflags::bitflags;
use strum::EnumDiscriminants;

use crate::markup::parse::{RuleKind, SymbolKind};

/// Unpacks a specific enum variant from a token, destructuring its fields into local variables.
///
/// This macro simplifies extracting data from `$crate::markup::lex::Token`. It performs
/// an immutable borrow of the token and uses a `let-else` statement to panic if
/// the variant does not match the expected type.
///
/// # Arguments
/// * `$instance` - An expression that provides access to the token (e.g., an AST node or wrapper).
/// * `$variant` - The specific `Token` variant name to match (e.g., `Identifier`).
/// * `{ $($field ... )* }` - A standard destructuring block. Supports field renaming
///   (`field: alias`) and the `..` rest pattern.
///
/// # Panics
/// Panics if the token is `None` (via `.unwrap()`) or if the token's variant
/// does not match `$variant`.
///
/// # Examples
/// ```
/// // Simple destructuring: creates local variables 'name' and 'span'
/// unpack_token!(node, Identifier { name, span });
///
/// // With renaming and rest pattern: creates variable 'val' from 'value'
/// unpack_token!(node, Literal { value: val, .. });
/// ```
#[macro_export]
macro_rules! unpack_token {
    ($instance:expr, $variant:ident { $($field:ident $(: $alias:ident)?),* $(, $(..)?)? }) => {
        let $crate::markup::lex::Token::$variant {
            $($field $(: $alias)?),* , ..
        } = $instance.kind.token().unwrap() else {
            panic!("Unpack failed: Expected {}", stringify!($variant));
        };
    };
}

static FORMAT_VARIANTS: OnceLock<Vec<InlineFormat>> = OnceLock::new();

/// The format in which a numbered list should be displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Numbering {
    Number,
    Lower,
    Upper,
    LowerNumeral,
    UpperNumeral,
}

impl Numbering {
    #[inline]
    pub const fn from_marker(marker: u8) -> Option<Self> {
        match marker {
            b'd' => Some(Numbering::Number),
            b'a' => Some(Numbering::Lower),
            b'A' => Some(Numbering::Upper),
            b'r' => Some(Numbering::LowerNumeral),
            b'R' => Some(Numbering::UpperNumeral),
            _ => None,
        }
    }
}

bitflags::bitflags! {
    /// The type of inline format marker.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct InlineFormat: u8 {
        const BOLD = 0b0000_0001;
        const ITALIC = 0b0000_0010;
        const STRIKETHROUGH = 0b0000_0100;
        const UNDERLINE = 0b000_1000;
        const HIGHLIGHT = 0b0001_0000;

        const BOLD_ITALIC = Self::BOLD.bits() | Self::ITALIC.bits();
    }
}

impl InlineFormat {
    /// Returns the length of the character cluster that denotes the given flag or bitmask.
    #[inline]
    pub fn len(self) -> usize {
        if self == Self::BOLD_ITALIC {
            return 3;
        }
        if self == Self::BOLD {
            return 2;
        }
        return 1;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckboxType {
    Filled,
    Empty,
    Toggle,
}

impl CheckboxType {
    /// Returns the checkbox type according to the
    pub const fn from_marker(marker: u8) -> Option<Self> {
        match marker {
            b'x' => Some(CheckboxType::Filled),
            b'o' => Some(CheckboxType::Empty),
            b'?' => Some(CheckboxType::Toggle),
            _ => None,
        }
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ListItemPos: u8 {
        const Any = 0b0000;
        const First = 0b0001;
        const Last = 0b0010;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListItemKind {
    Unordered,
    Continuation,
    Numbered(Numbering),
    Checkbox(CheckboxType),
}

impl ListItemKind {
    /// Returns true if both kinds of list items can reside within the same list.
    #[inline]
    pub fn is_sibling(self, other: Self) -> bool {
        if self == Self::Unordered {
            return other == Self::Unordered;
        }
        if matches!(self, Self::Numbered(_)) {
            return self == other;
        }
        debug_assert!(matches!(self, Self::Checkbox(_)));
        return matches!(other, Self::Checkbox(_));
    }

    /// Returns the open tag, or panics if this is a continuation.
    #[inline]
    pub const fn open_tag(self) -> &'static str {
        match self {
            Self::Unordered => "ul class='dt-Unordered'",
            Self::Numbered(ty) => match ty {
                Numbering::Number => "ol class='dt-numbering'",
                Numbering::Lower => "ol type='a' class='dt-numbering'",
                Numbering::Upper => "ol type='A' class='dt-numbering'",
                Numbering::LowerNumeral => "ol type='i' class='dt-numbering'",
                Numbering::UpperNumeral => "ol type='I' class='dt-numbering'",
            },
            Self::Checkbox(ty) => match ty {
                CheckboxType::Empty => "ol class='dt-checkbox--empty'",
                CheckboxType::Filled => "ol class='dt-checkbox--filled'",
                CheckboxType::Toggle => "ol class='det-checkbox--toggle'",
            },
            Self::Continuation => panic!("Cannot resolve open tag"),
        }
    }

    /// Returns the open tag, or panics if this is a continuation.
    #[inline]
    pub const fn close_tag(self) -> &'static str {
        match self {
            Self::Unordered => "ul",
            Self::Continuation => panic!("Cannot resolve close tag"),
            _ => "ol",
        }
    }
}

/// The class and payload of a token.
///
/// Tokens are categorized based on their unique function and listener logic.
///
/// Tokens containing each respective type include boundary markers
/// in range they represent (see comments).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumDiscriminants)]
#[strum_discriminants(name(TokenKind))]
pub enum Token<'a> {
    // Content
    Plaintext,
    Literal { ch: u8 },                // preceded by `\`
    LinkBody { href: &'a [u8] },       // ]( )
    LinkAliasBody { alias: &'a [u8] }, // ][ ]
    InferredLink { href: &'a [u8] },
    LinkMarker,
    EmbedMarker,
    MacroHandle { name: &'a [u8] },   // \[ ]
    InlineCode { body: &'a [u8] },    // ` `
    InlineRawCode { body: &'a [u8] }, // `` ``
    InlineMath { body: &'a [u8] },    // $ $
    InlineFormat { ty: InlineFormat, twin_pos: usize },

    // Everything else
    Newline,
    HorizontalRule, // doubles as row divider, if enabled
    LineQuoteMarker,
    BlockQuoteOpen,
    BlockQuoteClose,
    MacroDecor { body: &'a [u8] },  // ( )
    MacroConfig { body: &'a [u8] }, // [ ]
    MacroBody { body: &'a [u8] },   // { }
    HeadingMarker { depth: u8 },
    CodeBlock { body: &'a [u8], lang: &'a [u8] },
    MathBlock { body: &'a [u8] },
    ListItemMarker { indent: u8, kind: ListItemKind },
    Eof, // necessary to find bound for trailing plaintext; pruned before parsing
}

impl Token<'_> {
    pub const HEADING_MAX: usize = 6;

    #[inline]
    pub const fn is_content(self) -> bool {
        matches!(
            self,
            Self::Plaintext
                | Self::Literal { .. }
                | Self::LinkBody { .. }
                | Self::LinkAliasBody { .. }
                | Self::LinkMarker
                | Self::EmbedMarker
                | Self::MacroHandle { .. }
                | Self::InlineCode { .. }
                | Self::InlineRawCode { .. }
                | Self::InlineMath { .. }
                | Self::InlineFormat { .. }
        )
    }

    #[inline]
    pub fn kind(&self) -> TokenKind {
        TokenKind::from(self)
    }
}

impl SymbolKind for TokenKind {
    #[inline]
    fn as_rule_kind(self) -> Option<RuleKind> {
        None
    }

    #[inline]
    fn as_token_kind(self) -> Option<TokenKind> {
        Some(self)
    }
}

/// Represents a range of meaningful content in a markup file.
///
/// The end index is exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenSpan<'a> {
    pub token: Token<'a>,
    pub start: usize,
    pub end: usize,
}

impl<'a> TokenSpan<'a> {
    #[inline]
    pub const fn new(token: Token<'a>, start: usize, end: usize) -> Self {
        Self { token, start, end }
    }

    /// Guaranteed to be nonzero.
    #[inline]
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    /// Marks this span as plaintext.
    #[inline]
    pub const fn bind_plain(&mut self) {
        self.token = Token::Plaintext;
    }
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-core\src\markup\mod.rs:

//! Major compiler passes are split between `X.rs` and `X_utils.rs` files.
//! The first contains the primary logic, whereas the latter contains everything else.
//!
//! Modules should be imported internally using re-export.
pub mod config;
mod lexer;
mod lexer_utils;
mod parser;
mod parser_utils;
mod traversal;
mod traversal_utils;

pub mod lex {
    pub use super::{lexer::*, lexer_utils::*};
}

pub mod parse {
    pub use super::{parser::*, parser_utils::*};
}

pub mod visit {
    pub use super::{traversal::*, traversal_utils::*};
}

use std::vec;

use taped::Tape;

use crate::{
    markup::{
        lex::{ListItemKind, ListItemPos, Token, TokenKind as token, TokenSpan},
        parse::{
            AstNode, AstNode as node, AstOutput,
            Handler, NodeKind, NodeMetadata as meta,
            RuleKind as rule, TokenStream,
        },
    },
    unpack, unpack_token,
};

/// Enumerates the rule names given as an array of tuples, each containing:
/// - The index of the element in the array as `Choice` metadata
/// - The rule handler (in `Rules`)
///
/// Returns `[(choice, handler)]`, or `handler` if a single name is given.
macro_rules! rule_options {
    // Without offset
    ($($name:ident),* $(,)?) => {
        [
            $(
                (
                    meta::Choice(${index()} as u8),
                    Self::$name as Handler<'a>
                )
            ),*
        ]
    };

    // With offset
    ($offset:expr; $($name:ident),* $(,)?) => {
        [
            $(
                (
                    meta::Choice((${index()} + $offset) as u8),
                    Self::$name as Handler<'a>
                )
            ),*
        ]
    };
}

/// Queries the next token span in the tape, if one exists.
/// If so, it is matched against each of the kind of tokens given.
///
/// On a successful match, `tape.pos` is incremented by 1, and the second member
/// of the returned tuple is populated with:
/// - The index of the chosen kind
/// - The AST node
///
/// The first member is the number of kinds passed to this macro.
///
/// Returns `(len, Option(choice, node))`.
macro_rules! token_options {
    ($tape:expr; $($name:ident),* $(,)?) => {
        {
            let tokens = [$(token::$name),*];
            if let Some(span) = $tape.peek() {
                let peek = span.token.kind();
                let choice = tokens.iter().position(|t| *t == peek);
                if let Some(choice) = choice {
                    $tape.adv();
                    (tokens.len(), Some((meta::Choice(choice as u8), node::token(span))))
                } else {
                    (tokens.len(), None)
                }
            } else {
                (tokens.len(), None)
            }
        }
    };
}

/// Queries the next token span in the tape, if one exists.
///
/// Returns `Option(node)`.
macro_rules! try_token {
    ($tape:expr, $name:ident $(,)?) => {
        node::try_token(token::$name, &mut $tape)
    };
}

/// Queries the next token span in the tape, if one exists.
///
/// If the match succeeds, the the first member of the returned tuple is `true`,
/// or `false` otherwise. The second member is always a vector containing
/// the single matched node, or an empty list if the match failed.
///
/// Returns `(is_present, children)`
macro_rules! optional_token {
    ($tape:expr, $name:ident $(,)?) => {{
        let children: Vec<AstNode<'a>> = try_token!($tape, $name).into_iter().collect();
        let is_present = meta::IsPresent(!children.is_empty());
        (is_present, children)
    }};
}

/// Declares a handler for the rule of the given name.
///
/// `body` is passed as a closure (as opposed to a block) to allow for full IntelliSense
/// and formatting.
macro_rules! rule {
    ($name:ident, $body:expr $(,)?) => {
        #[inline(always)]
        pub fn $name(tape: TokenStream<'a>) -> Result<'a> {
            ($body as Handler<'a>)(tape)
        }
    };
}

/// Assembles the abstract syntax tree (AST) from the token stream.
///
/// Since a zero-length input is also accepted, a match (even if partial)
/// will always be made. To check if the entire input is matched,
/// ensure the root `Markup` node contains children.
pub fn assemble_ast<'a>(tokens: &'a [TokenSpan<'a>]) -> Option<AstOutput<'a>> {
    Grammar::markup(Tape::new(tokens))
}

/// Used to assemble the AST according to the following grammar:
/// ```ebnf
/// markup := topLevelElement*
///
/// topLevelElement := Newline
///     | HorizontalRule
///     | CodeBlock
///     | MathBlock
///     | Assignment
///     | list
///     | paragraph
///     | heading
///     | lineQuote
///     | blockQuote
/// heading := HeadingMarker
///     & line
///     & Newline
///
/// # stops at Newline & Newline
/// # consumes continuations that whose bullet type could not be inferred
/// paragraph := ContinuationMarker? & (Newline | lineElement)+
///
/// line := lineElement+
///     & Newline
/// lineElement := Plaintext
///     | InlineCode
///     | InlineMath
///     | InlineRawCode
///     | Literal
///     | format
///     | link
///     | embed
///     | macro
///
/// format := InlineFormat & paragraph & InlineFormat
/// link := InferredLink | LinkMarker & linkTarget
/// embed := EmbedMarker & linkTarget
/// linkTarget := LinkBody | LinkAliasBody
///
/// lineQuote := LineQuoteMarker & line
/// blockQuote := BlockQuoteOpen
///     & (Newline | line)
///     & topLevelElement+
///     & BlockQuoteClose
///
/// # for clarity, no empty lines between list items
/// list := listItem+
/// listItem := (ListItemMarker | NumberedItemMarker | Checkbox) & line
///
/// macro := MacroHandle
///     & MacroArgs?
///     & MacroBody*
/// ```
///
/// For constructing new grammars, the following protocol usually suffices:
/// 1. Make rules for tokens that easily combine
/// 2. Combine rules into abstract concepts
/// 3. Seperate elements by creating rules for top-level and inline nodes
///
/// This parser, like `Lexer`, is hand-written to encourage a simple API and
/// optimal performance.
///
/// It is imperative to keep the DSL-like macro API
/// internal as opposed to transferring it to a library to ensure all project constraints,
/// including performance. This applies to traversal as well.
///
/// Macros for common operations enable rapid iteration for changes in the EBNF.
///
/// Any caching/indexing should occur in the LSP itself, not due to the parser.
pub struct Grammar;

impl<'a> Grammar {
    rule!(markup, |mut tape| {
        let mut children = vec![];
        while let Some((child, jump)) = Self::top_level_element(tape) {
            children.push(child);
            tape = jump
        }
        Some((node::branch(rule::Markup, children, meta::None), tape))
    });

    rule!(top_level_element, |mut tape| {
        let (len, res) = token_options![tape; Newline, HorizontalRule, CodeBlock, MathBlock];
        if let Some((choice, child)) = res {
            return Some((
                node::branch(rule::TopLevelElement, vec![child], choice),
                tape,
            ));
        }
        for (choice, handler) in
            rule_options![len; paragraph, list, heading, line_quote, block_quote]
        {
            if let Some((child, jump)) = handler(tape) {
                return Some((
                    node::branch(rule::TopLevelElement, vec![child], choice),
                    jump,
                ));
            }
        }
        None
    });

    rule!(heading, |mut tape| {
        let mut children = vec![];
        if let Some(child) = try_token!(tape, HeadingMarker) {
            children.push(child);
            let (child, mut tape) = Self::line(tape)?;
            children.push(child);
            if let Some(child) = try_token!(tape, Newline) {
                children.push(child);
                return Some((node::branch(rule::Heading, children, meta::None), tape));
            }
        }
        None
    });

    rule!(line, |mut tape| {
        let mut children_a = vec![];
        while let Some((child_a, jump)) = Self::line_element(tape) {
            children_a.push(child_a);
            tape = jump;
        }
        if children_a.is_empty() {
            return None;
        }
        let a = node::branch(rule::None, children_a, meta::None);
        if let Some(b) = try_token!(tape, Newline) {
            return Some((node::branch(rule::Line, vec![a, b], meta::None), tape));
        }
        None
    });

    rule!(line_element, |mut tape| {
        let (len, res) =
            token_options![tape; Plaintext, InlineCode, InlineMath, InlineRawCode, Literal];
        if let Some((choice, child)) = res {
            return Some((node::branch(rule::LineElement, vec![child], choice), tape));
        }
        for (choice, handler) in rule_options![len; format, link, embed, macro_rule] {
            if let Some((child, jump)) = handler(tape) {
                return Some((node::branch(rule::LineElement, vec![child], choice), jump));
            }
        }
        None
    });

    rule!(paragraph, |mut tape| {
        let mut children = vec![];
        loop {
            let (choice, child_a) = if let Some(child_a) = try_token!(tape, Newline) {
                if tape
                    .peek()
                    .is_some_and(|span| span.token.kind() == token::Newline)
                {
                    break;
                }
                (0, child_a)
            } else if let Some((child_a, jump)) = Self::line_element(tape) {
                tape = jump;
                (1, child_a)
            } else {
                break;
            };
            let child = node::branch(rule::None, vec![child_a], meta::Choice(choice));
            children.push(child);
        }
        if children.is_empty() {
            return None;
        }
        Some((node::branch(rule::None, children, meta::None), tape))
    });

    rule!(format, |mut tape| {
        let a = try_token!(tape, InlineFormat)?;
        let (b, mut tape) = Self::paragraph(tape)?;
        let closer = tape.next().filter(|span| matches!(span.token, Token::InlineFormat { twin_pos, .. } if twin_pos == a.start))?;
        let c = AstNode {
            start: closer.start,
            end: closer.end,
            parent: None,
            children: vec![],
            meta: meta::None,
            kind: NodeKind::Token(closer.token),
        };
        Some((node::branch(rule::Format, vec![a, b, c], meta::None), tape))
    });

    //InferredLink | LinkMarker & linkTarget
    rule!(link, |mut tape| {
        match try_token!(tape, InferredLink) {
            Some(a) => Some((node::branch(rule::Link, vec![a], meta::Choice(0)), tape)),
            None => {
                let a = try_token!(tape, LinkMarker)?;
                let (b, tape) = Self::link_target(tape)?;
                let ab = node::branch(rule::None, vec![a, b], meta::None);
                Some((node::branch(rule::Link, vec![ab], meta::Choice(1)), tape))
            }
        }
    });

    rule!(embed, |mut tape| {
        let a = try_token!(tape, EmbedMarker)?;
        let (b, tape) = Self::link_target(tape)?;
        Some((node::branch(rule::Embed, vec![a, b], meta::None), tape))
    });

    rule!(link_target, |mut tape| {
        let (_, res) = token_options![tape; LinkBody, LinkAliasBody];
        if let Some((choice, child)) = res {
            return Some((node::branch(rule::LinkTarget, vec![child], choice), tape));
        }
        None
    });

    rule!(list, |mut tape| {
        let mut children_a = vec![];
        let mut node = node::new(rule::List, vec![], tape.peek()?.start, meta::None);
        while let Some((child_a, jump)) = Self::list_item(tape, &mut node) {
            children_a.push(child_a);
            tape = jump;
        }
        let prev = children_a.last_mut()?;
        unpack!(prev.meta, meta::ListItem { kind, pos });

        // mark last item
        let pos = ListItemPos::from_bits(pos.bits() | ListItemPos::Last.bits()).unwrap();

        prev.meta = meta::ListItem { kind, pos };
        let a = node::branch(rule::None, children_a, meta::None);
        Some((node::branch(rule::List, vec![a], meta::None), tape))
    });

    pub fn list_item(mut tape: TokenStream<'a>, parent: &mut AstNode<'a>) -> Result<'a> {
        let mut a = try_token!(tape, ListItemMarker)?;
        unpack_token!(
            a,
            ListItemMarker {
                indent: indent_a,
                kind: kind_a
            }
        );
        let kind_a = if kind_a == ListItemKind::Continuation {
            unpack_token!(
                parent.children.iter().rev().find(|node| {
                    matches!(node.kind.token().unwrap(),
                        Token::ListItemMarker { indent, kind }
                        if indent == indent_a && kind != ListItemKind::Continuation
                    )
                })?,
                ListItemMarker { kind, .. }
            );
            kind
        } else {
            kind_a
        };
        if parent.children.is_empty() {
            a.meta = meta::ListItem {
                kind: kind_a,
                pos: ListItemPos::First,
            };
        } else {
            let mut pos = ListItemPos::Any;
            let prev = parent.children.last_mut().unwrap();
            unpack_token!(
                prev,
                ListItemMarker {
                    indent: prev_indent,
                    ..
                }
            );
            if prev_indent > indent_a {
                pos |= ListItemPos::First;
            } else if prev_indent < indent_a {
                unpack!(prev.meta, meta::ListItem { kind, pos });
                prev.meta = meta::ListItem {
                    kind,
                    pos: pos.intersection(ListItemPos::Last),
                };
            }
            a.meta = meta::ListItem { kind: kind_a, pos };
        }
        let (b, tape) = Self::line(tape)?;
        Some((node::branch(rule::ListItem, vec![a, b], meta::None), tape))
    }

    rule!(line_quote, |mut tape| {
        let a = try_token!(tape, LineQuoteMarker)?;
        let (b, tape) = Self::link_target(tape)?;
        Some((node::branch(rule::LineQuote, vec![a, b], meta::None), tape))
    });

    rule!(block_quote, |mut tape| {
        let a = try_token!(tape, BlockQuoteOpen)?;
        let choice: u8;
        let child_b = if let Some(child_b) = try_token!(tape, Newline) {
            choice = 0;
            child_b
        } else {
            choice = 1;
            let (child_b, jump) = Self::line(tape)?;
            tape = jump;
            child_b
        };
        let b = node::branch(rule::None, vec![child_b], meta::Choice(choice));
        let mut children_c = vec![];
        while let Some((child_c, jump)) = Self::top_level_element(tape) {
            children_c.push(child_c);
            tape = jump;
        }
        if children_c.is_empty() {
            return None;
        }
        let c = node::branch(rule::None, children_c, meta::None);
        let d = try_token!(tape, BlockQuoteClose)?;
        Some((
            node::branch(rule::BlockQuote, vec![a, b, c, d], meta::None),
            tape,
        ))
    });

    rule!(macro_rule, |mut tape| {
        let a = try_token!(tape, MacroHandle)?;
        let (is_present, children_b) = optional_token!(tape, MacroArgs);
        let b = node::new(rule::None, children_b, a.end, is_present);
        let mut children_c = vec![];
        while let Some(child_c) = try_token!(tape, MacroBody) {
            children_c.push(child_c);
        }
        let is_present = !children_c.is_empty();
        let c = node::new(rule::None, children_c, b.end, meta::IsPresent(is_present));
        Some((node::branch(rule::Macro, vec![a, b, c], meta::None), tape))
    });
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-core\src\markup\parser_utils.rs:

use std::ops::Deref;

use taped::Tape;

use crate::markup::{
    lex::{ListItemKind, ListItemPos, Token, TokenKind, TokenSpan},
    parse::NodeMetadata as meta,
};

pub type TokenStream<'a> = Tape<'a, TokenSpan<'a>>;
pub type Handler<'a> = fn(TokenStream<'a>) -> Option<(AstNode<'a>, TokenStream<'a>)>;

/// Returned by [`assemble_ast`][`crate::markup::assemble_ast`].
pub struct AstOutput<'a> {
    root: AstNode<'a>,
    rest: TokenStream<'a>,
}

/// A token or parser rule that can be matched to some slice of the
/// list of tokens produced after lexing.
pub trait SymbolKind {
    fn as_token_kind(self) -> Option<TokenKind>;
    fn as_rule_kind(self) -> Option<RuleKind>;
}

/// Rule identifiers, decoupled from rule matching logic to promote extensibility.
///
/// The suffix *-Kind* is used instead of *-Id* to avoid confusion with unique serial numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleKind {
    Markup,
    TopLevelElement,
    Heading,
    Paragraph,
    Line,
    LineElement,
    Format,
    Link,
    Embed,
    List,
    LinkTarget,
    LineQuote,
    BlockQuote,
    ListItem,
    Macro,

    None,
}

impl SymbolKind for RuleKind {
    #[inline]
    fn as_rule_kind(self) -> Option<RuleKind> {
        Some(self)
    }

    #[inline]
    fn as_token_kind(self) -> Option<TokenKind> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind<'a> {
    Rule(RuleKind),
    Token(Token<'a>),
}

impl<'a> NodeKind<'a> {
    #[inline]
    pub const fn token(self) -> Option<Token<'a>> {
        match self {
            Self::Token(token) => Some(token),
            _ => None,
        }
    }
}

impl<'a> SymbolKind for NodeKind<'a> {
    #[inline]
    fn as_token_kind(self) -> Option<TokenKind> {
        match self {
            Self::Token(_) => None,
            Self::Rule(_) => None,
        }
    }

    #[inline]
    fn as_rule_kind(self) -> Option<RuleKind> {
        match self {
            Self::Rule(rule) => Some(rule),
            Self::Token(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeMetadata {
    // 0-indexed option choice.
    Choice(u8),

    // If true, target of optional was matched.
    IsPresent(bool),

    ListItem {
        kind: ListItemKind,
        pos: ListItemPos,
    },
    None,
}

/// Dereferences to its `children` vector.
///
/// `end` is exclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstNode<'a> {
    pub children: Vec<AstNode<'a>>,

    pub meta: NodeMetadata,
    pub parent: Option<RuleKind>,
    pub start: usize,
    pub end: usize,
    pub kind: NodeKind<'a>,
}

impl<'a> Deref for AstNode<'a> {
    type Target = Vec<AstNode<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.children
    }
}

impl<'a> AstNode<'a> {
    /// Returns a rule node that may be either a leaf or a branch.
    pub fn new(
        rule: RuleKind,
        mut children: Vec<AstNode<'a>>,
        pos: usize,
        meta: NodeMetadata,
    ) -> Self {
        if children.is_empty() {
            return Self {
                start: pos,
                end: pos,
                parent: None,
                children,
                meta,
                kind: NodeKind::Rule(rule),
            };
        }
        for child in children.iter_mut() {
            child.parent = Some(rule)
        }
        Self {
            start: children[0].start,
            end: children.last().unwrap().end,
            parent: None,
            children,
            meta,
            kind: NodeKind::Rule(rule),
        }
    }

    /// Returns a rule branch node.
    ///
    /// # Panics
    /// Panics if `children` is empty.
    pub fn branch(rule: RuleKind, mut children: Vec<AstNode<'a>>, meta: NodeMetadata) -> Self {
        if children.is_empty() {
            panic!("Missing children for rule {rule:?}")
        }
        for child in children.iter_mut() {
            child.parent = Some(rule)
        }
        Self {
            start: children[0].start,
            end: children.last().unwrap().end,
            parent: None,
            children,
            meta,
            kind: NodeKind::Rule(rule),
        }
    }

    /// Returns a token leaf node using the next token span in the tape.
    ///
    /// # Panics
    /// Panics if `tape` is exhausted.
    #[inline]
    pub fn token(span: TokenSpan<'a>) -> Self {
        Self {
            start: span.start,
            end: span.end,
            parent: None,
            children: vec![],
            meta: meta::None,
            kind: NodeKind::Token(span.token),
        }
    }

    /// Returns a token leaf node using the next token span in the tape,
    /// incrementing `tape.pos` on success.
    ///
    /// # Panics
    /// Panics if the tape is exhausted.
    pub fn try_token(token: TokenKind, tape: &mut TokenStream<'a>) -> Option<Self> {
        if tape.peek().is_none_or(|span| span.token.kind() != token) {
            return None;
        }
        let span = tape.next().unwrap();
        Some(Self {
            start: span.start,
            end: span.end,
            parent: None,
            children: vec![],
            meta: meta::None,
            kind: NodeKind::Token(span.token),
        })
    }

    #[inline]
    pub fn is_leaf(&self) -> bool {
        matches!(self.kind, NodeKind::Token(_))
    }

    #[inline]
    pub fn is_branch(&self) -> bool {
        matches!(self.kind, NodeKind::Rule(_))
    }
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-core\src\markup\traversal.rs:

use pastey::paste;

use crate::{
    markup::{
        lex::InlineFormat,
        parse::{AstNode, SymbolKind},
        traversal_utils::Visitor,
    },
    unpack_token,
};

//no pretty printing, user can set up formatter in their editor
macro_rules! visits {
    ($name:ident $(,)?) => {
        paste! {
            fn [< visit_ $name >](&mut self, node: &AstNode<'a>);
        }
    };
}

macro_rules! visitor {
    ($name:ident, $body:expr $(,)?) => {
        paste! {
            #[inline(always)]
            fn [< visit_ $name >](&mut self, node: &AstNode<'a>) {
                ($body as Visitor<'a,_>)(self, node)
            }
        }
    };
    ($name:ident $(,)?) => {
        paste! {
            #[inline(always)]
            fn [< visit_ $name >](&mut self, _: &AstNode<'a>) {}
        }
    };
}

macro_rules! emit {
    ($model:ident, $($arg:tt)*) => {
        $model.out.push_str(&format!($($arg)*))
    };
}

/// A visitor trait for traversing and processing AST (Abstract Syntax Tree) nodes.
///
/// This trait defines a set of methods for visiting different types of nodes in the AST.
/// Implementors can use mutable references to maintain state during traversal.
///
/// # Note
///
/// Passing mutable references to visitors is permissible as there is no intention to enable
/// parallelization at this time.
///
/// # Methods
///
/// The trait provides specialized visitor methods for different node types:
/// - `visit_ast_node`: Visit a generic AST node
/// - `visit_block_node`: Visit a block-level node
/// - `visit_list_node`: Visit a list node
/// - `visit_inline_node`: Visit an inline node
///
/// # Generic Parameter
///
/// Each method is generic over return type `Self::Output`, allowing visitors to return different types
/// of results depending on the implementation and context.
pub trait AstVisitor<'a> {
    // Rules
    visits!(markup);
    visits!(top_level_element);
    visits!(heading);
    visits!(line);
    visits!(line_element);
    visits!(format);
    visits!(link);
    visits!(embed);
    visits!(paragraph);
    visits!(link_target);
    visits!(list);
    visits!(list_item);
    visits!(line_quote);
    visits!(block_quote);
    visits!(macro_rule);

    // Tokens
    visits!(plaintext);
    visits!(literal);
    visits!(link_body);
    visits!(link_alias_body);
    visits!(inferred_link);
    visits!(link_marker);
    visits!(embed_marker);
    visits!(macro_handle);
    visits!(inline_code);
    visits!(inline_raw_code);
    visits!(inline_math);
    visits!(inline_format);
    visits!(newline);
    visits!(horizontal_rule);
    visits!(line_quote_marker);
    visits!(block_quote_open);
    visits!(block_quote_close);
    visits!(macro_args);
    visits!(macro_body);
    visits!(heading_marker);
    visits!(code_block);
    visits!(math_block);
    visits!(checkbox);
    visits!(list_item_marker);
    visits!(numbered_item_marker);
    visits!(assignment_marker);

    // Everything else
    visits!(none);
}

/// Emitted CSS obeys block-element-modifier (BEM) rules:
/// - **Block:** `.block`
/// - **Element:** `.block__element`
/// - **Modifier:** `.block--modifier` or `.block__element--modifier`
#[cfg(feature = "to-html")]
pub struct HtmlVisitor {
    out: String,
    in_pgraph: bool,
}

#[cfg(feature = "to-html")]
impl HtmlVisitor {
    #[inline]
    pub const fn new() -> Self {
        Self {
            out: String::new(),
            in_pgraph: false,
        }
    }
}

#[cfg(feature = "to-html")]
impl<'a> AstVisitor<'a> for HtmlVisitor {
    visitor!(none, |model: &mut HtmlVisitor, node| {
        match node.kind {
            parse::NodeKind::Rule(rule_kind) => todo!(),
            parse::NodeKind::Token(token) => todo!(),
        }
    });

    visitor!(paragraph, |model: &mut HtmlVisitor, node| {
        model.in_pgraph = true;
        emit!(model, "<p class='dt-pgraph'>");
        node[0]
            .iter()
            .filter(|&child| child.kind.as_rule_kind().is_some())
            .for_each(|child| {
                model.visit_line_element(child);
            });
        emit!(model, "</p>");
        model.in_pgraph = false;
    });

    visitor!(newline, |model: &mut HtmlVisitor, _| {
        emit!(model, " ");
    });

    visitor!(list, |model: &mut HtmlVisitor, node| { todo!() });

    visitor!(markup, |model: &mut HtmlVisitor, node| {});

    visitor!(heading, |model: &mut HtmlVisitor, node| {
        unpack_token!(node[0], HeadingMarker { depth });
        emit!(model, "<h{depth}>");
        model.visit_line(&node[1]);
        emit!(model, "</h{depth}>");
    });

    visitor!(format, |model: &mut HtmlVisitor, node| {
        unpack_token!(node[0], InlineFormat { ty });
        match ty {
            InlineFormat::BOLD => {
                emit!(model, "<b class='dt-bold'>");
                model.visit_paragraph(&node[1]);
                emit!(model, "</b>");
            }
            InlineFormat::HIGHLIGHT => {
                emit!(model, "<mark class='dt-hl'>");
                model.visit_paragraph(&node[1]);
                emit!(model, "</mark>");
            }
            InlineFormat::ITALIC => {
                emit!(model, "<i class='dt-italic'>");
                model.visit_paragraph(&node[1]);
                emit!(model, "</i>");
            }
            InlineFormat::STRIKETHROUGH => {
                emit!(model, "<s class='dt-rem'>");
                model.visit_paragraph(&node[1]);
                emit!(model, "</s>");
            }
            InlineFormat::UNDERLINE => {
                emit!(model, "<u class='dt-under'>");
                model.visit_paragraph(&node[1]);
                emit!(model, "</u>");
            }
            _ => panic!("Invalid format"),
        }
    });

    visitor!(horizontal_rule, |model: &mut HtmlVisitor, _| {
        emit!(model, "<hr>");
    });

    visitor!(line_quote, |model: &mut HtmlVisitor, node| {
        emit!(model, "<blockquote>");
        model.visit_line(&node[1]);
        emit!(model, "</blockquote>");
    });

    visitor!(block_quote, |model: &mut HtmlVisitor, node| {
        emit!(model, "<blockquote>"); // todo admonition
        model.visit_line(&node[2]);
        emit!(model, "</blockquote>");
    });

    visitor!(line_quote_marker);
    visitor!(block_quote_open);
    visitor!(block_quote_close);
}

/// Transforms an AST into Github-flavored Markdown (GFM)
#[cfg(feature = "to-markdown")]
pub struct MarkdownVisitor {
    out: String,
}

#[cfg(feature = "to-markdown")]
impl MarkdownVisitor {
    #[inline]
    pub const fn new() -> Self {
        Self { out: String::new() }
    }
}

#[cfg(feature = "to-markdown")]
impl<'a> AstVisitor<'a> for MarkdownVisitor {
    visitor!(horizontal_rule, |model: &mut MarkdownVisitor, _| {
        emit!(model, "---");
    });

    visitor!(newline, |model: &mut MarkdownVisitor, node| {
        emit!(model, "\n");
    });

    visitor!(heading, |model: &mut MarkdownVisitor, node| {
        unpack_token!(node[0], HeadingMarker { depth });
        emit!(model, "{:#>1$} ", "", depth as usize);
        model.visit_line(&node[1]);
    });

    visitor!(format, |model: &mut MarkdownVisitor, node| {
        unpack_token!(node[0], InlineFormat { ty });
        match ty {
            InlineFormat::BOLD => {
                emit!(model, "**");
                model.visit_paragraph(&node[1]);
                emit!(model, "**");
            }
            InlineFormat::HIGHLIGHT => {
                emit!(model, "***"); // default to bold-italic
                model.visit_paragraph(&node[1]);
                emit!(model, "***");
            }
            InlineFormat::ITALIC => {
                emit!(model, "*");
                model.visit_paragraph(&node[1]);
                emit!(model, "*");
            }
            InlineFormat::STRIKETHROUGH => {
                emit!(model, "~~");
                model.visit_paragraph(&node[1]);
                emit!(model, "~~");
            }
            InlineFormat::UNDERLINE => {
                emit!(model, "<ins>");
                model.visit_paragraph(&node[1]);
                emit!(model, "</ins>");
            }
            _ => panic!("Invalid format"),
        }
    });

    visitor!(line_quote, |model: &mut MarkdownVisitor, node| {
        emit!(model, "> ");
        model.visit_line(&node[1]);
        emit!(model, "\n");
    });

    visitor!(block_quote, |model: &mut MarkdownVisitor, node| {
        emit!(model, "<blockquote>"); // todo admonition
        model.visit_line(&node[2]);
        emit!(model, "</blockquote>");
    });

    visitor!(line_quote_marker);
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-core\src\markup\traversal_utils.rs:

use indoc::formatdoc;

use crate::markup::{parse::AstNode, visit::AstVisitor};

// todo yt link
// todo media link

pub type Visitor<'a, T: AstVisitor<'a>> = fn(&mut T, node: &AstNode<'a>);

#[cfg(feature = "to-html")]
pub fn media_html(tag: &str, url: &str) -> String {
    formatdoc! {"
        <{tag} src='{url}' controls>\
            <span class='dt-error'>Your browser does not support the &lt;$tag&gt; tag.</span>\
        </{tag}>\
    "}
}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-editor\src\main.rs:

use std::{
    fs::File,
    io::{BufWriter, Result as IoResult, Write},
};

pub const MAX_LINE_COUNT: u32 = u32::MAX;
pub const MAX_COLUMN_COUNT: u32 = u32::MAX;

// data is check box by line and whether it is toggled.
fn store_checkbox_state(data: &[(u32, bool)], path: &str) -> IoResult<()> {
    let file = File::create(path)?;
    let mut buf = BufWriter::new(file);
    for (line, cond) in data {
        buf.write_all(&line.to_le_bytes())?;
        let bool_byte = if *cond { 1u8 } else { 0u8 };
        buf.write_all(&[bool_byte])?;
    }
    buf.flush()?; // ensure everything is pushed to disk
    Ok(())
}

pub fn main() {}
\\?\C:\Users\eckar\Downloads\repos\draft\crates\draft-lsp\src\main.rs:

pub fn main() {}
