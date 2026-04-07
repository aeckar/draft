use crate::token::CheckboxType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AstNode<'a> {
    Block(BlockNode<'a>),
    Inline(InlineNode<'a>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockNode<'a> {
    Heading {
        depth: u8,
        content: Vec<InlineNode<'a>>,
    },
    Paragraph(Vec<InlineNode<'a>>),
    LineQuote(Vec<InlineNode<'a>>),
    CodeBlock {
        lang: &'a [u8],
        body: &'a [u8],
    },
    MathBlock {
        body: &'a [u8],
    },
    List(Vec<ListNode<'a>>),
    BlockQuote(Vec<AstNode<'a>>),
    HorizontalRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListNode<'a> {
    Item {
        depth: u8,
        checkbox: Option<CheckboxType>,
        content: Vec<InlineNode<'a>>,
    },
    Sublist(Vec<ListNode<'a>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineNode<'a> {
    Text(&'a [u8]),
    Bold(Vec<InlineNode<'a>>),
    Italic(Vec<InlineNode<'a>>),
    InlineCode(&'a [u8]),
    Link {
        label: Vec<InlineNode<'a>>,
        href: &'a [u8],
    },
    Macro {
        name: &'a [u8],
        args: Option<&'a [u8]>,
        bodies: Vec<&'a [u8]>,
    },
}
