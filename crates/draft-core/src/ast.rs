use crate::token::Token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstNode<'a> {
    pub token: Option<Token<'a>>,
    pub parent: Option<&'a AstNode<'a>>,
    pub children: Vec<AstNode<'a>>,
}

impl<'a> AstNode<'a> {
    pub fn root() -> Self {
        Self {
            token: None,
            parent: None,
            children: vec![],
        }
    }

    pub fn new(token: Token<'a>, parent: &'a AstNode<'a>) -> Self {
        Self {
            token: Some(token),
            parent: Some(parent),
            children: vec![],
        }
    }
}
