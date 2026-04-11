use crate::markup::vocab::{Token, TokenKind, TokenSpan};
use crate::prelude::*;

/// A token or parser rule that can be matched to some slice of the
/// list of tokens produced after lexing.
pub trait Pattern<'a> {
    fn as_token_kind(&self) -> Option<&TokenKind>;
    fn as_rule(&self)->Option<&Rule<'a>>;
}

pub type Rule<'a> = fn(tape: Tape<'a,TokenSpan<'a>>) -> Match<'a>;

impl<'a> Pattern<'a> for Rule<'a> {
    fn as_rule(&self)->Option<&Rule<'a>> {
        Some(self)
    }

    fn as_token_kind(&self) -> Option<&TokenKind> {
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
    Macro
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind<'a> {
    Branch {
        /// Denotes the type of success by the primary matching logic.
        /// - If the symbol is an alternation, is the 0-based index of the symbol chosen
        /// - If the symbol is an option, is -2 if the argument did not match
        /// - Otherwise, is -1
        choice: i8,

        rule: RuleKind, 
    },
    Leaf { spec: Token<'a> },
    Root,
}

/// `end` is exclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstNode<'a> {
    pub parent: Option<RuleKind>,
    pub children: Vec<AstNode<'a>>,
    pub start: usize,
    pub end: usize,
    pub kind: NodeKind<'a>,
}

impl<'a> AstNode<'a> {
    pub fn branch(rule: RuleKind, parent: RuleKind, start: usize, end: usize, choice: i8) -> Self {
        Self {
            start,
            end,
            parent: Some(parent),
            children: vec![],
            kind: NodeKind::Branch { choice, rule }
        }
    }

    pub fn leaf(span: TokenSpan<'a>, parent: RuleKind) -> Self {
        Self {
            start: span.start,
            end: span.end,
            parent: Some(parent),
            children: vec![],
            kind: NodeKind::Leaf { spec: span.token }
        }
    }
}

/// Assembles the AST according to the following grammar:
/// ```ebnf
/// markup := topLevelElement*
///
/// topLevelElement := HorizontalRule
///     | CodeBlock
///     | MathBlock
///     | paragraph
///     | list
///     | heading
///     | lineQuote
///     | blockQuote
/// heading := Heading
///     & line
///     & Newline
/// paragraph := Plaintext | Literal | link
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
///
/// format := InlineFormat plaintext InlineFormat
/// link := LinkMarker & linkTarget
/// embed := EmbedMarker & linkTarget
/// linkTarget := LinkBody | LinkAliasBody
///
/// lineQuote := LineQuoteMarker & line
/// blockQuote := BlockQuoteOpen
///     & (line | Newline)
///     & topLevelElement+
///     & BlockQuoteClose
///
/// list := orderedList | numberedList | checklist
/// orderedList := (ListItemMarker & line)+
/// numberedList := (NumberedItemMarker & line)+
/// checklist := (Checkbox & line)+
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
/// Since a zero-length input is also accepted, a match (even if partial)
/// will always be made. To check if the entire input is matched, check the root `end`.
struct Parser<'a> {
    // All tokens in the markup file.
    tokens: &'a [TokenSpan<'a>],
}

impl<'a> Compile for Parser<'a> {
    type Output = AstNode<'a>;

    fn compile(self) -> Self::Output {
        self.markup(Tape::new(self.tokens)).unwrap()    // at least 0
    }
}

impl<'a> Parser<'a> {
    fn markup(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        let mut count = 0;
        while let self.top_level_element(tape) == Ok()
    }

    fn top_level_element(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{}

    fn heading(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn paragraph(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn line(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn line_element(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn format(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn link(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn embed(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn link_target(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn line_quote(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn block_quote(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn list(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn ordered_list(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn numbered_list(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn checklist(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    fn macro_rule(&self, mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }
}
