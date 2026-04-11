use pastey::paste;

use crate::markup::lexer_utils::TokenKind as token;
use crate::markup::parse::{NodeDesc, Pattern};
use crate::markup::parser_utils::RuleKind as rule;
use crate::{
    compile::Compile,
    markup::{lexer_utils::TokenSpan, parse::ChildrenDesc, parser_utils::AstNode},
    tape::Tape,
};

macro_rules! group {
    ($($item:expr),* $(,)?) => {
        &[
            $(&$item as &dyn Pattern),*
        ]
    };
}

macro_rules! rule {
    ($name:ident, in_order, [$($item:expr),* $(,)?]) => {
        paste! {
            pub fn $name(mut tape: Tape<'a, TokenSpan<'a>>) -> Option<ChildrenDesc<'a>> {
                // [< $name:camel >] converts snake_case to PascalCase
                tape.parse_in_order(group![$($item),*], rule::[< $name:camel >])?.into()
            }
        }
    };

    ($name:ident, any_of, [$($item:expr),* $(,)?]) => {
        paste! {
            pub fn $name(mut tape: Tape<'a, TokenSpan<'a>>) -> Option<ChildrenDesc<'a>> {
                tape.parse_any_of(group![$($item),*], rule::[< $name:camel >])?.into()
            }
        }
    };
}

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
/// format := InlineFormat & pla & InlineFormat
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
    pub fn markup(mut tape: Tape<'a, TokenSpan<'a>>) -> Option<ChildrenDesc<'a>> {
        tape.parse_n(rule::TopLevelElement, rule::Markup)
    }

    rule!(
        top_level_element,
        any_of,
        [
            token::HorizontalRule,
            token::CodeBlock,
            token::MathBlock,
            rule::Paragraph,
            rule::List,
            rule::Heading,
            rule::LineQuote,
            rule::BlockQuote,
        ]
    );

    rule!(
        heading,
        in_order,
        [token::Heading, rule::Line, token::Newline]
    );

    rule!(
        paragraph,
        any_of,
        [token::Plaintext, token::Literal, rule::Link]
    );

    // Complex case: Handwritten to handle .then() chaining
    pub fn line(mut tape: Tape<'a, TokenSpan<'a>>) -> Option<ChildrenDesc<'a>> {
        tape.parse_n(rule::LineElement, rule::_Defer)?
            .then(group![token::Newline], rule::Line)?
            .into()
    }

    rule!(
        line_element,
        any_of,
        [
            token::Plaintext,
            token::InlineCode,
            token::InlineMath,
            token::InlineRawCode,
            token::Literal,
            rule::Format,
            rule::Link,
            rule::Embed
        ]
    );

    rule!(
        format,
        in_order,
        [token::InlineFormat, token::InlineFormat]
    );

    rule!(
        link,
        in_order,
        [token::LinkMarker, rule::LinkTarget]
    );

    rule!(
        embed,
        in_order,
        [token::EmbedMarker, rule::LinkTarget]
    );

    rule!(
        link_target,
        any_of,
        [token::LinkBody, token::LinkAliasBody]
    );

    rule!(
        line_quote,
        in_order,
        [token::LineQuoteMarker, rule::Line]
    );

    rule!(
        list,
        any_of,
        [rule::OrderedList, rule::NumberedList, rule::Checklist]
    );
    
    // Complex case: Handwritten due to TODO logic
    pub fn block_quote(mut tape: Tape<'a, TokenSpan<'a>>) -> Option<ChildrenDesc<'a>> {
        tape.parse_any_of(group![rule::Line, token::Newline], rule::_Defer)?
            .then(others, parent) //todo
    }


    pub fn ordered_list(mut tape: Tape<'a, TokenSpan<'a>>) -> Option<ChildrenDesc<'a>> {
        todo!()
    }

    pub fn numbered_list(mut tape: Tape<'a, TokenSpan<'a>>) -> Option<ChildrenDesc<'a>> {
        todo!()
    }

    pub fn checklist(mut tape: Tape<'a, TokenSpan<'a>>) -> Option<ChildrenDesc<'a>> {
        todo!()
    }

    pub fn macro_rule(mut tape: Tape<'a, TokenSpan<'a>>) -> Option<ChildrenDesc<'a>> {
        todo!()
    }
}
