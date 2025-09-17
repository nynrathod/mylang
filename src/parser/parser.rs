use crate::lexar::token::{Token, TokenType};
use crate::parser::ast::{AstNode, TypeNode};

#[derive(Debug)]
pub enum ParseError {
    UnexpectedToken(String),
    EndOfInput,
}

pub type ParseResult<T> = Result<T, ParseError>;

pub struct Parser<'a> {
    tokens: &'a [Token<'a>],
    current: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token<'a>]) -> Self {
        Parser { tokens, current: 0 }
    }

    pub(crate) fn peek(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.current)
    }

    pub(crate) fn advance(&mut self) -> Option<&Token<'a>> {
        let tok = self.tokens.get(self.current);
        if tok.is_some() {
            self.current += 1;
        }
        tok
    }

    pub(crate) fn expect(&mut self, kind: TokenType) -> ParseResult<&Token<'a>> {
        match self.advance() {
            Some(tok) if tok.kind == kind => Ok(tok),
            Some(tok) => Err(ParseError::UnexpectedToken(format!(
                "Expected {:?}, got {:?}",
                kind, tok.kind
            ))),
            None => Err(ParseError::EndOfInput),
        }
    }

    pub fn parse_statement(&mut self) -> ParseResult<AstNode> {
        match self.peek() {
            Some(tok) => match tok.kind {
                TokenType::Let | TokenType::Var => self.parse_var_decl(),
                TokenType::Struct => self.parse_struct_decl(),
                TokenType::Enum => self.parse_enum_decl(),
                TokenType::If => self.parse_conditional_decl(),

                _ => Err(ParseError::UnexpectedToken(format!(
                    "Unexpected token: {:?}",
                    tok.kind
                ))),
            },
            None => Err(ParseError::EndOfInput),
        }
    }

    pub fn parse_program(&mut self) -> ParseResult<AstNode> {
        let mut statements = Vec::new();
        while self.current < self.tokens.len() {
            let stmt = self.parse_statement()?;
            statements.push(stmt);
        }
        Ok(AstNode::Program(statements))
    }

    pub(crate) fn parse_type_annotation(&mut self) -> ParseResult<TypeNode> {
        // e.g., "Int", "Array", "Map", "String"
        let tok = self.expect(TokenType::Identifier)?;

        match tok.value {
            "Int" => Ok(TypeNode::Int),
            "String" => Ok(TypeNode::String),
            "Bool" => Ok(TypeNode::Bool),
            "Array" => {
                self.expect(TokenType::Lt)?; // expect '<'
                let inner_type = self.parse_type_annotation()?;
                self.expect(TokenType::Gt)?; // expect '>'
                Ok(TypeNode::Array(Box::new(inner_type)))
            }
            "Map" => {
                self.expect(TokenType::Lt)?;
                let key_type = self.parse_type_annotation()?;
                self.expect(TokenType::Comma)?;
                let value_type = self.parse_type_annotation()?;
                self.expect(TokenType::Gt)?;
                Ok(TypeNode::Map(Box::new(key_type), Box::new(value_type)))
            }

            _ => {
                return Err(ParseError::UnexpectedToken(format!(
                    "Expected type {:?}",
                    tok.kind
                )))
            }
        }
    }
}
