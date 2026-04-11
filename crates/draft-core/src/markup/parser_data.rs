use crate::markup::parser::Rules;
use crate::markup::lexer_data::{Token, TokenKind, TokenSpan};
use crate::prelude::*;

/// A token or parser rule that can be matched to some slice of the
/// list of tokens produced after lexing.
pub trait Pattern<'a> {
    fn as_token_kind(self) -> Option<TokenKind>;
    fn as_rule(self) -> Option<Rule<'a>>;
}

pub type Rule<'a> = fn(tape: Tape<'a, TokenSpan<'a>>) -> Match<'a>;

impl<'a> Pattern<'a> for Rule<'a> {
    fn as_rule(self) -> Option<Rule<'a>> {
        Some(self)
    }

    fn as_token_kind(self) -> Option<TokenKind> {
        None
    }
}

/// Rule identifiers, decoupled from rule matching logic to promote extensibility.
///
/// The suffix *-Kind* is used instead of *-Id* to avoid confusion with unique serial numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
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
    LinkTarget,
    LineQuote,
    BlockQuote,
    List,
    OrderedList,
    NumberedList,
    Checklist,
    Macro,
}

impl<'a> Into<Rule<'a>> for RuleKind {
    fn into(self) -> Rule<'a> {
        match self {
            RuleKind::Markup => Rules::markup,
            RuleKind::TopLevelElement => Rules::top_level_element,
            RuleKind::Heading => Rules::heading,
            RuleKind::Paragraph => Rules::paragraph,
            RuleKind::Line => Rules::line,
            RuleKind::LineElement => Rules::line_element,
            RuleKind::Format => Rules::format,
            RuleKind::Link => Rules::link,
            RuleKind::Embed => Rules::embed,
            RuleKind::LinkTarget => Rules::link_target,
            RuleKind::LineQuote => Rules::line_quote,
            RuleKind::BlockQuote => Rules::block_quote,
            RuleKind::List => Rules::list,
            RuleKind::OrderedList => Rules::ordered_list,
            RuleKind::NumberedList => Rules::numbered_list,
            RuleKind::Checklist => Rules::checklist,
            RuleKind::Macro => Rules::macro_rule,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind<'a> {
    Branch(RuleKind),
    Leaf(Token<'a>),
}

impl<'a> NodeKind<'a> {
    pub fn rule(self) -> Option<RuleKind> {
        match self {
            Self::Branch(rule) => Some(rule),
            _ => None,
        }
    }

    pub fn token(self) -> Option<Token<'a>> {
        match self {
            Self::Leaf(token) => Some(token),
            _ => None,
        }
    }
}

/// `end` is exclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstNode<'a> {
    /// Contains metadata depending on the function used to create this node.
    /// - `Tape.expect_any`: 0-based index of the symbol chosen
    /// - `Tape.expect_maybe`: -2 if the argument did not match, else -1
    /// - Otherwise, is -1
    pub choice: i8,

    pub parent: Option<RuleKind>,
    pub children: Vec<AstNode<'a>>,
    pub start: usize,
    pub end: usize,
    pub kind: NodeKind<'a>,
}

impl<'a> AstNode<'a> {
    pub fn branch(
        rule: RuleKind,
        parent: RuleKind,
        children: Vec<AstNode<'a>>,
        choice: i8,
        start: usize,
        end: usize,
    ) -> Self {
        Self {
            start,
            end,
            parent: Some(parent),
            children,
            choice,
            kind: NodeKind::Branch { rule },
        }
    }

    pub fn leaf(span: TokenSpan<'a>, parent: RuleKind) -> Self {
        Self {
            start: span.start,
            end: span.end,
            parent: Some(parent),
            children: vec![],
            choice: -1,
            kind: NodeKind::Leaf { token: span.token },
        }
    }

    pub fn is_leaf(&self) -> bool {
        matches!(self.kind, NodeKind::Leaf(_))
    }

    pub fn is_branch(&self) -> bool {
        matches!(self.kind, NodeKind::Branch(_))
    }
}

///
/// Since a zero-length input is also accepted, a match (even if partial)
/// will always be made. To check if the entire input is matched, check the root `end`.
struct Parser<'a> {
    // All tokens in the markup file.
    tokens: &'a [TokenSpan<'a>],
}

impl<'a> Compile for Parser<'a> {
    type Output = AstNode<'a>;

    fn compile(self) -> Self::Output {
        Rules::markup(Tape::new(self.tokens)).unwrap()
    }
}
