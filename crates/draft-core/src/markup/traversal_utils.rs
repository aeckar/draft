use crate::markup::lex::{Numbering, Token};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ListItemKind {
    // Basic
    Continuation,
    Bullet,

    // Numbering
    Number,
    Lower,
    Upper,
    LowerNumeral,
    UpperNumeral,

    // Checkbox
    EmptyBox,
    FilledBox,
    ToggleBox,
}

impl ListItemKind {
    pub fn is_sibling(self, other: ListItemKind) -> bool {
        if self == Self::Continuation {
            return true;
        }
        if self == Self::Bullet {
            return other == Self::Bullet;
        }
        if self.is_numbering() {
            return self == other;
        }
        return other.is_checkbox();
    }

    pub const fn is_numbering(self) -> bool {
        matches!(
            self,
            Self::Number | Self::Lower | Self::Upper | Self::LowerNumeral | Self::UpperNumeral
        )
    }

    pub const fn is_checkbox(self) -> bool {
        matches!(self, Self::EmptyBox | Self::FilledBox | Self::ToggleBox)
    }

    pub const fn from_token_kind(token: Token) -> Self {
        match token {
            Token::ListItemMarker { .. } => Self::Bullet,
            Token::NumberedItemMarker { ty, .. } => match ty {
                Numbering::Upper => Self::Upper,
                Numbering::Lower => Self::Lower,
                Numbering::LowerNumeral => Self::LowerNumeral,
                Numbering::UpperNumeral => Self::UpperNumeral,
                Numbering::Continuation => Self::Continuation,
            },
            Token::Checkbox { ty, .. } => {}
            _ => panic!("Token is not a list item marker: {token:?}")
        }
    }

    pub const fn open_tag(self) -> Option<&'static str> {
        match self {
            Self::Bullet => Some("ul"),
            Self::Number => Some("ol"),
            Self::Lower => Some("ol type='a'"),
            Self::Upper => Some("ol type='A'"),
            Self::LowerNumeral => Some("ol type='i'"),
            Self::UpperNumeral => Some("ol type='I'"),
            Self::Continuation => None,
        }
    }

    pub const fn close_tag(self) -> Option<&'static str> {
        match self {
            Self::Bullet => Some("ol"),
            Self::Number | Self::Lower | Self::Upper | Self::LowerNumeral | Self::UpperNumeral => {
                Some("ol")
            }
            Self::Continuation => None,
        }
    }
}
