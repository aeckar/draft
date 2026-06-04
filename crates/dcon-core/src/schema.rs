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
