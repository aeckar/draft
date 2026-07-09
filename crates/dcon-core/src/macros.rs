//! Declarative macros used to construct object instances idiomatically.
//!
//! Rewritten using Claude.
//!
//! # Implementation
//!
//! Since macros expand in the caller's crate, unqualified `std` might not resolve if using
//! `#![no_std]`` or have a conflicting name in scope. `::std` anchors to the crate root,
//! making the path unambiguous regardless of where the macro is used.

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

use std::sync::LazyLock;

use ordered_float::NotNan;
use regex::Regex;

use crate::Object;

pub static ANY_STRING: LazyLock<Regex> = LazyLock::new(|| Regex::new(".*").unwrap());

/// Used to convert Rust literals to [Object][`crate::Object`] in builder macros.
///
/// Unlike normal conversions, these panic on failure.
pub trait Literal {
    fn into_obj(self) -> Object;
}

impl Literal for f64 {
    fn into_obj(self) -> Object {
        // safe, since NaN can never be a literal
        Object::Number(unsafe { NotNan::new_unchecked(self) })
    }
}

impl Literal for &str {
    fn into_obj(self) -> Object {
        Object::String(self.to_owned())
    }
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
macro_rules! ty {
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

/// Turns a dot-joined key path (`a`, `a.b.c`, …) *or* a single string-literal
/// key (`"weird key"`, `"$dollar"`, …) into the `String` used for map
/// insertion.
///
/// A bare string literal is unquoted and used verbatim — this is the escape
/// hatch for keys containing characters (spaces, `$`, …) that aren't valid
/// in a dotted identifier chain. Without this, `stringify!("$dollar")`
/// would produce the *token text* `"\"$dollar\""`, quote marks included,
/// instead of the string's actual value.
#[doc(hidden)]
#[macro_export]
macro_rules! __keypath {
    // A single string literal: use its value, not its token text.
    ($key:literal) => {
        $key.to_string()
    };
    // One or more dot-joined segments (idents, numbers, etc).
    ($($key:tt).+) => {
        stringify!($($key).+).to_string()
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
/// let o = __obj!(-1.0);                     // unary minus, top-level
/// let o = __obj!((2.0 * radius));           // (expr) = escape hatch for
///                                           // anything a bare literal
///                                           // can't express, e.g. a
///                                           // negative number nested
///                                           // inside a map!/list!
/// let o = __obj!(["a", "b", "c"]);          // [] = list
/// let o = __obj!({ "x" = 1.0, "y" = 2.0 }); // {} = map
/// ```
#[doc(hidden)]
#[macro_export]
macro_rules! __obj {
    // Symbolic constants
    (null $(,)?) => { $crate::Object::Null };
    (true $(,)?) => { $crate::Object::Bool(true) };
    (false $(,)?) => { $crate::Object::Bool(false) };

    // Escape hatch: any parenthesized expression. Since map!/list! entries
    // capture each value as a single token tree, a parenthesized group is
    // the way to smuggle in anything more than a bare literal — negative
    // numbers, arithmetic, a variable, a function call, etc.
    (($e:expr) $(,)?) => {
        {
            use $crate::Literal;
            ($e).into_obj()
        }
    };

    // Sugar for the common case of a *top-level* negative literal, e.g.
    // `obj!(-1.0)`. Inside a map!/list! entry, use the parens form above
    // instead (`key: (-1.0)`), since `-1.0` is two tokens and won't match
    // the single-`tt` value slot those macros use.
    (- $lit:literal $(,)?) => {
        {
            use $crate::Literal;
            (-$lit).into_obj()
        }
    };

    // Number | String
    ($lit:literal $(,)?) => {
        {
            use $crate::Literal;
            $lit.into_obj()
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
                let __key: ::std::string::String = __keypath!($($key).*);
                props.insert(
                    __key.as_str().try_into()
                        .unwrap_or_else(|_| panic!("Invalid key: {}", __key)),
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

    // Escape hatch / negative-literal sugar, same as __obj!.
    (($e:expr) $(,)?) => {
        __obj!(($e))
    };
    (- $lit:literal $(,)?) => {
        __obj!(-$lit)
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
    let _ = obj!(42.0);
    let _ = obj!("hello");
    let _ = obj!(-1.0);
    let _ = list![];
    let mymap = map! {
        a: 3,
        "$dollar": 1,           // quoted key -> literal "$dollar", no escaping needed
        my.value: 2,            // dotted key -> flat "my.value" key
        nested: { n: 2, m: 1 }, // {} nested directly as a value
        neg: (-4.5),            // (expr) escape hatch for non-literal values
        b: null,
    };
    let _ = obj!(4.20);
    let _ = list![1, 2, [3, 3], { a: 3 }, (-2.5)];
}
