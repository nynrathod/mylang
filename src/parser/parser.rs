use crate::lexar::token::{Token, TokenType};
use crate::parser::ast::AstNode;

#[allow(dead_code)]
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

    pub(crate) fn peek_is(&self, kind: TokenType) -> bool {
        self.peek().map(|tok| tok.kind == kind).unwrap_or(false)
    }

    pub(crate) fn advance(&mut self) -> Option<&Token<'a>> {
        let tok = self.tokens.get(self.current);
        if tok.is_some() {
            self.current += 1;
        }
        tok
    }

    pub(crate) fn consume_if(&mut self, kind: TokenType) -> bool {
        if self.peek_is(kind) {
            self.advance();
            true
        } else {
            false
        }
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
    fn parse_return(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::Return)?; // consume 'return'

        let mut values = Vec::new();

        loop {
            let expr = self.parse_expression()?;
            values.push(expr);

            match self.peek() {
                Some(tok) if tok.kind == TokenType::Comma => {
                    self.advance(); // consume ',' and continue parsing next expression
                }
                _ => break, // no more expressions
            }
        }

        self.expect(TokenType::Semi)?; // consume ';' at the end
        Ok(AstNode::Return { values })
    }

    pub fn parse_statement(&mut self) -> ParseResult<AstNode> {
        match self.peek() {
            Some(tok) => match tok.kind {
                TokenType::Let | TokenType::Var => self.parse_var_decl(),
                TokenType::Struct => self.parse_struct_decl(),
                TokenType::Enum => self.parse_enum_decl(),
                TokenType::If => self.parse_conditional_stmt(),
                TokenType::Return => self.parse_return(),
                TokenType::Print => self.parse_print(),
                TokenType::Break => self.parse_break(),
                TokenType::Continue => self.parse_continue(),
                TokenType::Function => self.parse_functional_decl(),
                TokenType::For => self.parse_for_stmt(),
                TokenType::Identifier | TokenType::Underscore | TokenType::OpenParen => {
                    // Try assignment first
                    if let Ok(assign) = self.parse_assignment() {
                        return Ok(assign);
                    } else {
                        // fallback: expression statement
                        let expr = self.parse_expression()?;
                        self.expect(TokenType::Semi)?;
                        Ok(expr)
                    }
                }

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
}
