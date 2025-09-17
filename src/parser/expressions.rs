use crate::lexar::token::TokenType;
use crate::parser::ast::AstNode;
use crate::parser::{ParseError, ParseResult, Parser};

impl<'a> Parser<'a> {
    pub fn parse_expression(&mut self) -> ParseResult<AstNode> {
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
                    AstNode::StringLiteral(tok.value.to_string())
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
                TokenType::OpenBracket => self.parse_array_literal()?,
                TokenType::OpenBrace => self.parse_map_literal()?,
                _ => {
                    return Err(ParseError::UnexpectedToken(format!(
                        "Expected number, identifier or string, got {:?}",
                        tok.kind
                    )))
                }
            },
            None => return Err(ParseError::EndOfInput),
        };

        // Handle binary '+' operator (you can extend later)
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

    fn parse_array_literal(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::OpenBracket)?;
        let mut elements = Vec::new();

        while let Some(tok) = self.peek() {
            if tok.kind == TokenType::CloseBracket {
                break;
            }

            let expr = self.parse_expression()?;
            elements.push(expr);

            if let Some(tok) = self.peek() {
                if tok.kind == TokenType::Comma {
                    self.advance();
                } else if tok.kind != TokenType::CloseBracket {
                    return Err(ParseError::UnexpectedToken(format!(
                        "Expected ',' or ']', got {:?}",
                        tok.kind
                    )));
                }
            }
        }

        self.expect(TokenType::CloseBracket)?;
        Ok(AstNode::ArrayLiteral(elements))
    }

    fn parse_map_literal(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::OpenBrace)?;
        let mut entries = Vec::new();

        while let Some(tok) = self.peek() {
            if tok.kind == TokenType::CloseBrace {
                break;
            }

            let key = self.parse_expression()?;
            self.expect(TokenType::Colon)?;
            let value = self.parse_expression()?;
            entries.push((key, value));

            if let Some(tok) = self.peek() {
                if tok.kind == TokenType::Comma {
                    self.advance();
                }
            }
        }

        self.expect(TokenType::CloseBrace)?;
        Ok(AstNode::MapLiteral(entries))
    }
}
