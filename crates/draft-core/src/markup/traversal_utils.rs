use crate::markup::lex::{CheckboxType, Numbering, Token};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Returns true if both kinds of list items can reside within the same list.
    pub fn is_sibling(self, other: Self) -> bool {
        if self == Self::Continuation {
            return other.is_numbering();
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

    pub fn from_token(token: Token) -> Self {
        match token {
            Token::ListItemMarker { .. } => Self::Bullet,
            Token::NumberedItemMarker { ty, .. } => match ty {
                Numbering::Number => Self::Number,
                Numbering::Upper => Self::Upper,
                Numbering::Lower => Self::Lower,
                Numbering::LowerNumeral => Self::LowerNumeral,
                Numbering::UpperNumeral => Self::UpperNumeral,
                Numbering::Continuation => Self::Continuation,
            },
            Token::Checkbox { ty, .. } => match ty {
                CheckboxType::Empty => Self::EmptyBox,
                CheckboxType::Filled => Self::FilledBox,
                CheckboxType::Toggle => Self::ToggleBox,
            },
            _ => panic!("Token is not a list item marker: {token:?}"),
        }
    }

    /// Returns the open tag, or that of `fallback` if this is a continuation.
    pub fn open_tag_or(self, fallback: Self) -> &'static str {
        if self == Self::Continuation {
            fallback.open_tag()
        } else {
            self.open_tag()
        }
    }

    /// Returns the close tag, or that of `fallback` if this is a continuation.
    pub fn close_tag_or(self, fallback: Self) -> &'static str {
        if self == Self::Continuation {
            fallback.close_tag()
        } else {
            self.close_tag()
        }
    }

    /// Returns the open tag, or panics if this is a continuation.
    pub const fn open_tag(self) -> &'static str {
        match self {
            Self::Continuation => panic!("Cannot deduce open tag"),
            Self::Bullet => "ul class='dt-bullet'",
            Self::Number => "ol class='dt-numbering'",
            Self::Lower => "ol type='a' class='dt-numbering'",
            Self::Upper => "ol type='A' class='dt-numbering'",
            Self::LowerNumeral => "ol type='i' class='dt-numbering'",
            Self::UpperNumeral => "ol type='I' class='dt-numbering'",
            Self::EmptyBox => "ol class='dt-checkbox--empty'",
            Self::FilledBox => "ol class='dt-checkbox--filled'",
            Self::ToggleBox => "ol class='det-checkbox--toggle'",
        }
    }

    /// Returns the open tag, or panics if this is a continuation.
    pub const fn close_tag(self) -> &'static str {
        match self {
            Self::Bullet => "ul",
            Self::Continuation => panic!("Cannot deduce close tag"),
            _ => "ol",
        }
    }
}
