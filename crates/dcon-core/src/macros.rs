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

/// Used to convert Rust literals to [Object][`crate::object::Object`] in builder macros.
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
    fn into_obj(self) -> Object {

    }
}

impl Literal for i64 {
    fn into_obj(self) -> Object {
        
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
macro_rules! ty {//todo rename
    // Atomic types
    (any $(,)?) => { $crate::object::ObjectSpec::Any };
    (null $(,)?) => { $crate::object::ObjectSpec::Null };
    (bool $(,)?) => { $crate::object::ObjectSpec::Bool };
    (true $(,)?) => { $crate::object::ObjectSpec::True };
    (false $(,)?) => { $crate::object::ObjectSpec::False };
    (number $(,)?) => { $crate::object::ObjectSpec::Number };
    (string $(,)?) => { $crate::object::ObjectSpec::String };

    // Range
    // lo => hi
    ($lo:expr => $hi:expr $(,)?) => {
        $crate::object::ObjectSpec::Range {
            start: ::ordered_float::NotNan::new($lo as f64).unwrap(),
            end: ::ordered_float::NotNan::new($hi as f64).unwrap(),
        }
    };

    // Exact string
    // Must use double quotes due to Rust convention
    ($expect:literal $(,)?) => {
        $crate::object::ObjectSpec::ExactString($expect)
    };

    // Pattern
    // r"pat"
    // Must use double quotes due to Rust convention
    (r$pat:literal $(,)?) => {
        $crate::object::ObjectSpec::Pattern(::regex::Regex::new($pat).expect(concat!("Invalid regex: ", $pat)))
    };

    // List
    ($ty:tt[] $(,)?) => {
        $crate::object::ObjectSpec::List { ty: Box::new(ty!($ty)) }
    };

    // Sized list
    ([$ty:tt; $n:expr] $(,)?) => {
        $crate::object::ObjectSpec::SizedList { ty: Box::new(ty!($ty)), length: $n as usize }
    };

    // Tuple
    ([$($slot:tt),* $(,)?] $(,)?) => {
        $crate::object::ObjectSpec::Tuple {
            slots: vec![ $( ty!($slot) ),+ ],
        }
    };

    // Map with key-value constraints
    ({ $($key:literal $( ? $opt:tt )? : $ty:tt $(| $rest:tt)* ),* $(,)? } $(,)?) => {
        {
            let mut map = $crate::object::Object::MapProps::new();
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
            $crate::object::ObjectSpec::Map(map)
        }
    };

    // Constraint union
    // Right-associative
    // Flattens nested Union arms so `A | B | C` => `Union([A, B, C])`
    ($head:tt | $($tail:tt)|+ $(,)?) => {{
        let lhs = ty!($head);
        let rhs = ty!($($tail)|+);
        match (lhs, rhs) {
            ($crate::object::ObjectSpec::Union(mut a), $crate::object::ObjectSpec::Union(b)) => {
                a.extend(b);
                $crate::object::ObjectSpec::Union(a)
            }
            ($crate::object::ObjectSpec::Union(mut a), rhs) => {
                a.push(rhs);
                $crate::object::ObjectSpec::Union(a)
            }
            (lhs, $crate::object::ObjectSpec::Union(mut b)) => {
                b.insert(0, lhs);
                $crate::object::ObjectSpec::Union(b)
            }
            (lhs, rhs) => $crate::object::ObjectSpec::Union(vec![lhs, rhs]),
        }
    }};
}

/// Constructs an [ObjectSpec::Map][`crate::object::ObjectSpec::Map`] (a schema definition).
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

/// Construct an [Object][`crate::object::Object`] from literal notation.
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
    (null) => { $crate::object::Object::Null };
    (true) => { $crate::object::Object::Bool(true) };
    (false) => { $crate::object::Object::Bool(false) };

    // Number | String
    ($lit:literal) => {
        {
            use $crate::object::Literal;

            $lit.into_obj()
        }
    };

    (@float $lit:literal) => {
        $crate::object::Object::try_from($lit)
            .expect(concat!("Number must not be NaN: ", stringify!($lit)))
    };

    (@wide_int $lit:literal) => {
        {
            // Maximum/minimum exact integers for `f64`
            const MAX_EXACT: i64 = 9_007_199_254_740_991;
            const MIN_EXACT: i64 = -9_007_199_254_740_991;

            if $lit >= MIN_EXACT && $lit <= MAX_EXACT {
                $crate::object::Object::from(
                    unsafe { ::ordered_float::NotNan::new_unchecked($lit as f64) }
                )
            } else {
                Err("Value is too large/small to be represented losslessly in f64: ")
            }
            $crate::object::Object::try_from($lit)
                .expect(concat!("Number must not be NaN: ", stringify!($lit)))
        }
    };

    // List
    ([ $($item:tt),* $(,)? ]) => {
        $crate::object::Object::List(vec![ $( __obj!($item) ),* ])
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
            $crate::object::Object::Map(props)
        }
    };
}

/// Constructs a basic [Object][`crate::object::Object`] from literal notation.
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

/// Constructs an [`Object::List`][`crate::object::Object::List`].
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

/// Constructs an [`Object::Map`][`crate::object::Object::Map`].
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
