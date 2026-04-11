use simdutf8::basic::Utf8Error;
use thiserror::Error;

use crate::markup::vocab::{Token, TokenSpec};
use crate::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Rule {
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

        rule: Rule, 
    },
    Leaf { spec: TokenSpec<'a> },
    Root,
}

/// `end` is exclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstNode<'a> {
    pub parent: Option<&'a AstNode<'a>>,
    pub children: Vec<AstNode<'a>>,
    pub start: usize,
    pub end: usize,
    pub kind: NodeKind<'a>,
}

impl<'a> AstNode<'a> {
    /// Root nodes must have their metadata assigned after building the tree.
    /// The returned node must exist beforehand to obtain ownership of children.
    pub fn root(len: usize) -> Self {
        Self {
            start: 0,
            end: len,
            parent: None,
            children: vec![],
            kind: NodeKind::Root,
        }
    }

    pub fn branch(rule: Rule, start: usize, end: usize, choice: i8) -> Self {
        Self {
            start,
            end,
            parent: None,
            children: vec![],
            kind: NodeKind::Branch { choice, rule }
        }
    }

    pub fn leaf(token: Token<'a>) -> Self {
        Self {
            start: token.start,
            end: token.end,
            parent: None,
            children: vec![],
            kind: NodeKind::Leaf { spec: token.spec }
        }
    }

    pub fn bind(&mut self, mut child: AstNode<'a>) {
        child.parent = Some(self);
        self.children.push(child)
    }

}

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("No tokens found")]
    MissingTokens(#[from] Utf8Error),

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
/// will always be made.
struct Parser<'a> {
    // All tokens in the markup file.
    tokens: &'a [Token<'a>],
}

impl<'a> Compile for Parser<'a> {
    type Output = Result<AstNode<'a>, ParserError>;

    fn compile(self) -> Self::Output {
        self.markup(Tape::new(self.tokens))
    }
}

impl<'a> Parser<'a> {
    fn markup(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        let mut count = 0;
        while let self.top_level_element(tape) == Ok()
    }

    fn top_level_element(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {}

    fn heading(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn paragraph(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn line(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn line_element(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn format(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn link(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn embed(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn link_target(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn line_quote(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn block_quote(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn list(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn ordered_list(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn numbered_list(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn checklist(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }

    fn macro_rule(&self, mut tape: Tape<'a, Token>) -> Result<AstNode<'a>, ParserError> {
        todo!()
    }
}
