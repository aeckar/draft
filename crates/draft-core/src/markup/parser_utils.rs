use crate::markup::lexer_utils::{Token, TokenKind, TokenSpan};
use crate::markup::parser::Rules;
use crate::prelude::*;

/// A token or parser rule that can be matched to some slice of the
/// list of tokens produced after lexing.
pub trait Pattern {
    fn of_token(&self) -> Option<TokenKind>;
    fn of_rule(&self) -> Option<RuleKind>;
}

pub type Rule<'a> = fn(tape: Tape<'a, TokenSpan<'a>>) -> Option<AstNode<'a>>;

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

    _Defer, // placeholder
}

impl Pattern for RuleKind {
    fn of_rule(self) -> Option<RuleKind> {
        Some(self)
    }

    fn of_token(self) -> Option<TokenKind> {
        None
    }
}

impl<'a> Into<Rule<'a>> for RuleKind {
    fn into(self) -> Rule<'a> {
        match self {
            RuleKind::Markup => Rules::markup as Rule<'a>,
            RuleKind::TopLevelElement => Rules::top_level_element as Rule<'a>,
            RuleKind::Heading => Rules::heading as Rule<'a>,
            RuleKind::Paragraph => Rules::paragraph as Rule<'a>,
            RuleKind::Line => Rules::line as Rule<'a>,
            RuleKind::LineElement => Rules::line_element as Rule<'a>,
            RuleKind::Format => Rules::format as Rule<'a>,
            RuleKind::Link => Rules::link as Rule<'a>,
            RuleKind::Embed => Rules::embed as Rule<'a>,
            RuleKind::LinkTarget => Rules::link_target as Rule<'a>,
            RuleKind::LineQuote => Rules::line_quote as Rule<'a>,
            RuleKind::BlockQuote => Rules::block_quote as Rule<'a>,
            RuleKind::List => Rules::list as Rule<'a>,
            RuleKind::OrderedList => Rules::ordered_list as Rule<'a>,
            RuleKind::NumberedList => Rules::numbered_list as Rule<'a>,
            RuleKind::Checklist => Rules::checklist as Rule<'a>,
            RuleKind::Macro => Rules::macro_rule as Rule<'a>,
        }
    }
}

impl<'a> From<Rule<'a>> for RuleKind {
    fn from(rule: Rule<'a>) -> Self {
        match rule {
            _ if rule == Rules::markup => RuleKind::Markup,
            _ if rule == Rules::top_level_element => RuleKind::TopLevelElement,
            _ if rule == Rules::heading => RuleKind::Heading,
            _ if rule == Rules::paragraph => RuleKind::Paragraph,
            _ if rule == Rules::line => RuleKind::Line,
            _ if rule == Rules::line_element => RuleKind::LineElement,
            _ if rule == Rules::format => RuleKind::Format,
            _ if rule == Rules::link => RuleKind::Link,
            _ if rule == Rules::embed => RuleKind::Embed,
            _ if rule == Rules::link_target => RuleKind::LinkTarget,
            _ if rule == Rules::line_quote => RuleKind::LineQuote,
            _ if rule == Rules::block_quote => RuleKind::BlockQuote,
            _ if rule == Rules::list => RuleKind::List,
            _ if rule == Rules::ordered_list => RuleKind::OrderedList,
            _ if rule == Rules::numbered_list => RuleKind::NumberedList,
            _ if rule == Rules::checklist => RuleKind::Checklist,
            _ if rule == Rules::macro_rule => RuleKind::Macro,
            _ => panic!("Unknown rule: {rule:p}"),
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

impl<'a> Pattern for NodeKind<'a> {
    fn of_token(&self) -> Option<TokenKind> {
        match self {
            Self::Leaf(_) => None,
            Self::Branch(_) => None,
        }
    }

    fn of_rule(&self) -> Option<RuleKind> {
        match self {
            Self::Branch(rule) => Some(*rule),
            Self::Leaf(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeMetadata {
    Count(u8),
    Choice(u8),
    IsPresent(bool),
    None,
}

/// `end` is exclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstNode<'a> {
    pub meta: NodeMetadata,
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
        meta: NodeMetadata,
        start: usize,
        end: usize,
    ) -> Self {
        Self {
            start,
            end,
            parent: Some(parent),
            children,
            meta,
            kind: NodeKind::Branch(rule),
        }
    }

    pub fn leaf(span: TokenSpan<'a>, parent: RuleKind) -> Self {
        Self {
            start: span.start,
            end: span.end,
            parent: Some(parent),
            children: vec![],
            meta: NodeMetadata::None,
            kind: NodeKind::Leaf(span.token),
        }
    }

    pub fn is_leaf(&self) -> bool {
        matches!(self.kind, NodeKind::Leaf(_))
    }

    pub fn is_branch(&self) -> bool {
        matches!(self.kind, NodeKind::Branch(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeDesc<'a> {
    pub node: AstNode<'a>,
    pub meta: NodeMetadata,
    pub tape: Tape<'a, TokenSpan<'a>>,
}

impl<'a> NodeDesc<'a> {
    pub fn leaf(span: TokenSpan<'a>, parent: RuleKind, tape: Tape<'a, TokenSpan<'a>>) -> Self {
        Self {
            node: AstNode::leaf(span, parent),
            meta: NodeMetadata::None,
            tape,
        }
    }

    pub fn branch(
        query: RuleKind,
        parent: RuleKind,
        children: Vec<AstNode<'a>>,
        meta: NodeMetadata,
        start: usize,
        tape: Tape<'a, TokenSpan<'a>>,
    ) -> Self {
        Self {
            node: AstNode::branch(query, parent, children, meta, start, tape.pos),
            meta,
            tape,
        }
    }

    pub fn then(mut self, others: &[&dyn Pattern], parent: RuleKind) -> Option<NodeDesc<'a>> {
        let mut query: Vec<&dyn Pattern> = vec![&self.node.kind];
        query.extend_from_slice(others);
        self.tape
            .parse_in_order(query.as_slice(), parent)
            .inspect(|nd| self.node.parent = Some(nd.node.kind.of_rule().unwrap()))
    }

    pub fn or(mut self, others: &[&dyn Pattern], parent: RuleKind) -> Option<NodeDesc<'a>> {
        let mut query: Vec<&dyn Pattern> = vec![&self.node.kind];
        query.extend_from_slice(others);
        self.tape
            .parse_any_of(query.as_slice(), parent)
            .inspect(|nd| self.node.parent = Some(nd.node.kind.of_rule().unwrap()))
    }

    pub fn n_times(mut self, parent: RuleKind) -> Option<NodeDesc<'a>> {
        let mut query: Vec<&dyn Pattern> = vec![&self.node.kind];
        query.extend_from_slice(others);
        self.tape
            .parse_any_of(query.as_slice(), parent)
            .inspect(|nd| self.node.parent = Some(nd.node.kind.of_rule().unwrap()))
    }
}

pub struct ChildrenDesc<'a> {
    pub nodes: Vec<AstNode<'a>>,
    pub meta: NodeMetadata,
    pub tape: Tape<'a, TokenSpan<'a>>,
}

impl<'a> From<NodeDesc<'a>> for ChildrenDesc<'a> {
    fn from(value: NodeDesc<'a>) -> Self {
        Self {
            nodes: vec![value.node],
            meta: value.meta,
            tape: value.tape,
        }
    }
}
