use crate::tokens::{Token, TokenType};

#[derive(Debug, Clone)]
pub enum TypeNode {
    Int,
    String,
    Bool,
    Array(Box<TypeNode>),              // Array<Int>, Array<String>
    Map(Box<TypeNode>, Box<TypeNode>), // Map<String, Int>
}

#[derive(Debug, Clone)]
pub enum AstNode {
    Program(Vec<AstNode>),
    NumberLiteral(i64),
    Identifier(String),
    StringLiteral(String),
    BoolLiteral(bool),
    ArrayLiteral(Vec<AstNode>),
    MapLiteral(Vec<(AstNode, AstNode)>),

    // 1+2 || a+2
    BinaryExpr {
        left: Box<AstNode>,
        op: TokenType,
        right: Box<AstNode>,
    },

    VarDecl {
        mutable: bool,
        type_annotation: Option<TypeNode>,
        name: String,
        value: Box<AstNode>,
    },
}

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

    fn peek(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.current)
    }

    fn advance(&mut self) -> Option<&Token<'a>> {
        let tok = self.tokens.get(self.current);
        if tok.is_some() {
            self.current += 1;
        }
        tok
    }

    fn expect(&mut self, kind: TokenType) -> ParseResult<&Token<'a>> {
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

    fn parse_type_annotation(&mut self) -> ParseResult<TypeNode> {
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

    fn parse_array_literal(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::OpenBracket)?; // consume '['
        let mut elements = Vec::new();

        while let Some(tok) = self.peek() {
            if tok.kind == TokenType::CloseBracket {
                break;
            }

            let expr = self.parse_expression()?;
            elements.push(expr);

            if let Some(tok) = self.peek() {
                if tok.kind == TokenType::Comma {
                    self.advance(); // consume ','
                } else if tok.kind != TokenType::CloseBracket {
                    return Err(ParseError::UnexpectedToken(format!(
                        "Expected ',' or ']', got {:?}",
                        tok.kind
                    )));
                }
            }
        }

        self.expect(TokenType::CloseBracket)?; // consume ']'

        Ok(AstNode::ArrayLiteral(elements))
    }

    fn parse_map_literal(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::OpenBrace)?; // consume '{'
        let mut entries = Vec::new();

        while let Some(tok) = self.peek() {
            if tok.kind == TokenType::CloseBrace {
                break;
            }

            let key = self.parse_expression()?;
            self.expect(TokenType::Colon)?;
            let value = self.parse_expression()?;
            entries.push((key, value));

            // Consume optional comma
            if let Some(tok) = self.peek() {
                if tok.kind == TokenType::Comma {
                    self.advance();
                }
            }
        }

        self.expect(TokenType::CloseBrace)?; // consume '}'

        Ok(AstNode::MapLiteral(entries))
    }

    fn parse_expression(&mut self) -> ParseResult<AstNode> {
        let left = match self.peek() {
            Some(tok) => match tok.kind {
                TokenType::Number => {
                    let tok = self.advance().unwrap();
                    AstNode::NumberLiteral(tok.value.parse::<i64>().unwrap())
                }
                TokenType::Identifier => {
                    let tok = self.advance().unwrap();
                    AstNode::Identifier(tok.value.to_string())
                }
                TokenType::String => {
                    let tok = self.advance().unwrap();
                    AstNode::StringLiteral(tok.value.to_string()) // new variant in AstNode
                }
                TokenType::Boolean => {
                    let tok = self.advance().unwrap();
                    let value = match tok.value {
                        "true" => true,
                        "false" => false,
                        _ => unreachable!(),
                    };
                    AstNode::BoolLiteral(value)
                }
                TokenType::OpenBracket => self.parse_array_literal()?, // handle [...]
                TokenType::OpenBrace => self.parse_map_literal()?,     // handle {...}

                _ => {
                    return Err(ParseError::UnexpectedToken(format!(
                        "Expected number, identifier or string, got {:?}",
                        tok.kind
                    )))
                }
            },
            None => return Err(ParseError::EndOfInput),
        };

        // Handle '+' operator if needed for numbers
        if let Some(op_tok) = self.peek() {
            if op_tok.kind == TokenType::Plus {
                self.advance();
                let right = self.parse_expression()?;
                return Ok(AstNode::BinaryExpr {
                    left: Box::new(left),
                    op: TokenType::Plus,
                    right: Box::new(right),
                });
            }
        }

        Ok(left)
    }

    fn parse_var_decl(&mut self) -> ParseResult<AstNode> {
        let first_tok = self.advance().ok_or(ParseError::EndOfInput)?;
        let mutable = match first_tok.kind {
            TokenType::Let => false,
            TokenType::Var => true,
            _ => return Err(ParseError::UnexpectedToken("Expected let or var".into())),
        };

        let name_tok = self.expect(TokenType::Identifier)?;
        let name = name_tok.value.to_string();

        let mut type_annotation = None;
        if let Some(tok) = self.peek() {
            if tok.kind == TokenType::Colon {
                self.advance(); // consume ':'
                let parsed_type = self.parse_type_annotation()?;
                type_annotation = Some(parsed_type);
            }
        }

        self.expect(TokenType::Eq)?;
        let value = self.parse_expression()?;

        self.expect(TokenType::Semi)?;

        Ok(AstNode::VarDecl {
            mutable,
            name,
            type_annotation,
            value: Box::new(value),
        })
    }
}
