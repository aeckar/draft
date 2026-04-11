use crate::{markup::{parser_data::AstNode, lexer_data::TokenSpan}, tape::Tape};

/// Used to assemble the AST according to the following grammar:
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
pub struct Rules;

impl<'a> Rules {
    pub fn markup(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        let mut count = 0;
        while let self.top_level_element(tape) == Ok()
    }

    pub fn top_level_element(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{}

    pub fn heading(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn paragraph(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn line(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn line_element(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn format(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn link(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn embed(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn link_target(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn line_quote(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn block_quote(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn list(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn ordered_list(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn numbered_list(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn checklist(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }

    pub fn macro_rule(mut tape: Tape<'a, TokenSpan>) -> Option<AstNode<'a>>{
        todo!()
    }
}
