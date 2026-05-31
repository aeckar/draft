use std::{collections::HashMap, fmt::Display, str::Utf8Error};

use ordered_float::NotNan;
use taped::{CharExt, Tape};
use thiserror::Error;
use unindent::unindent;

use crate::{prelude::*, unpack};

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Object {
    Null,
    Bool(bool),
    Number(NotNan<f64>),
    String(String),
    List(Vec<Object>),
    Map { map: HashMap<String, Object> },
}

impl Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(cond) => write!(f, "{cond}"),
            Self::Number(n) => write!(f, "{n}"),
            Self::String(str) => write!(f, "\"{str}\""),
            Self::List(items) => {
                write!(f, "{{")?;
                for (idx, item) in items.iter().enumerate() {
                    write!(f, "{item}")?;
                    if idx != items.len() - 1 {
                        write!(f, ",")?;
                    }
                }
                write!(f, "}}")
            }
            Self::Map { map } => {
                write!(f, ".{{")?;
                for (idx, (key, val)) in map.iter().enumerate() {
                    write!(f, "{key}={val}")?;
                    if idx != map.len() - 1 {
                        write!(f, ",")?;
                    }
                }
                write!(f, "}}")
            }
        };
        Ok(())
    }
}

struct Name {
    
}
impl Object {
    pub fn to_pstring(&self) -> String {
        let mut buf = String::new();
        // Start with 0 indentation
        self.pfmt(&mut buf, 0).unwrap();
        buf
        l;
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
                    return write!(f, "{{}}");
                }
                writeln!(f, "{{")?;
                for item in items {
                    write!(f, "{next_indent}")?;
                    item.pfmt(f, depth + 1)?;
                    writeln!(f, ",")?; // use trailing comma
                }
                write!(f, "}}")
            }
            Self::Map { map } => {
                if map.is_empty() {
                    return write!(f, ".{{}}");
                }
                if map.len() == 1 {
                    let (key, val)=map.iter().next().unwrap();
                    write!(f, ".{{\n{next_indent}{key} = ")?;
                    val.pfmt(f, depth + 1)?;
                    return write!(f, ",\n{indent}}}"); // use trailing comma
                }
                let mut groups: Vec<Vec<&str>> = Vec::with_capacity(map.len());
                groups.push(map.keys().map(|s| s.as_str()).collect());
                while groups.len() < groups.capacity() {
                    let pool = &groups[0];
                    let Some((a_pre, a_rest)) = groups[groups.len() - 1][0].split_once('.') else {

                        continue;
                    };
                    for key in &pool[1..] {
                        if key.starts_with(prefix) {
                            
                        }
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

    #[error("Invalid number: {reason}")]
    InvalidNumber { reason: &'static str },

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
            b'.' => self.parse_map(tape),
            b'{' => self.parse_list(tape),
            b'"' => self.parse_string(tape, b'"'),
            b'\'' => self.parse_string(tape, b'\''),
            b'-' | b'+' | b'0'..=b'9' => {
                lexical_core::parse_partial_with_options::<f64, NUM_FORMAT>(
                    tape.rest(),
                    &NUM_OPTIONS,
                )
                .inspect(|&(_, len)| tape.pos += len)
                .map(|(n, _)| Object::Number(unsafe { NotNan::new_unchecked(n) }))
                .map_err(|e| e.into())
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
        tape.adv(); // skip '.'
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
                unpack!(
                    self.parse_map(tape)?,
                    Object::Map { map: inner }
                );
                for (mut k, v) in inner {
                    // flatten keys
                    k.insert_str(0, key);
                    map.insert(k, v);
                }
                continue;
            }
            tape.consume(|ch, _| ch.is_simple_ws());
            if tape.cur() != Some(b'=') {
                return Err(Error::IllegalCharacter {
                    ch: tape.cur().unwrap_or(0),
                    pos: tape.pos,
                });
            }
            tape.adv(); // skip '='
            tape.consume(|ch, _| ch.is_simple_ws());
            map.insert(key.to_string(), self.parse_any(tape)?);
        }
        Ok(Object::Map { map })
    }

    #[must_use]
    fn parse_list(&self, tape: &mut Tape<'a, u8>) -> Result<Object, Error> {
        let mut items = vec![];
        loop {
            tape.consume(|ch, _| ch.is_simple_ws() || ch == b'\n');
            if tape.cur() == Some(b'}') {
                tape.adv();
                break;
            }
            if tape.cur().is_none() {
                return Err(Error::MissingCloser {
                    open: b'{',
                    close: b'}',
                    open_pos: tape.pos,
                });
            }
            items.push(self.parse_any(tape)?);
            tape.consume(|ch, _| ch.is_simple_ws() || ch == b'\n');
            if tape.cur() == Some(b',') {
                tape.adv();
            } else if tape.cur() != Some(b'}') {
                return Err(Error::IllegalCharacter {
                    ch: tape.cur().unwrap_or(0),
                    pos: tape.pos,
                });
            }
        }
        Ok(Object::List(items))
    }
}
