use std::{collections::HashMap, fmt::Display, str::Utf8Error};

use ordered_float::{FloatIsNan, NotNan};
use serde::{Deserialize, Serialize};
use strum::EnumDiscriminants;
use taped::{CharExt, Tape, ToTape};
use thiserror::Error;
use unindent::unindent;

use crate::{prelude::*, unpack};

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

/// Object notation syntax.
///
/// On success, calling `compile` returns the decoded data and the number of bytes read.
///
/// # Implementation
/// Since object notation is relatively small compared to markup, we skip `simdutf8`
/// for UTF-8 validation. Instead, we give callers that responsibility (except for slices).
pub struct ObjectSyntax<'a> {
    /// The input text.
    pub input: &'a [u8],

    /// If true, expressions are allowed
    pub expr_mode: bool,
}

impl<'a> Compile for ObjectSyntax<'a> {
    type Output = Result<Object, Error>;

    fn compile(self) -> Self::Output {
        self.parse_any(&mut Tape::new(self.input))
    }
}

/// All `parse_X` functions assume cursor is at a valid character.
impl<'a> ObjectSyntax<'a> {
    #[must_use]
    pub fn new(input: &'a str, expr_mode: bool) -> Self {
        Self {
            input: input.as_bytes(),
            expr_mode,
        }
    }

    #[must_use]
    fn parse_any(&self, tape: &mut Tape<'a, u8>) -> Result<Object, Error> {
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
            b'{' => self.parse_map(tape),
            b'[' => self.parse_list(tape),
            b'"' => self.parse_string(tape, b'"'),
            b'\'' => self.parse_string(tape, b'\''),
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
    fn parse_string(&self, tape: &mut Tape<'a, u8>, delim: u8) -> Result<Object, Error> {
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
    fn parse_map(&self, tape: &mut Tape<'a, u8>) -> Result<Object, Error> {
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
                unpack!(self.parse_map(tape)?, Object::Map(inner));
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
            map.insert(Key(key.to_owned()), self.parse_any(tape)?);
        }
        Ok(Object::Map(map))
    }

    #[must_use]
    fn parse_list(&self, tape: &mut Tape<'a, u8>) -> Result<Object, Error> {
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
            items.push(self.parse_any(tape)?);
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
}
