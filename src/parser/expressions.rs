use crate::lexar::token::TokenType;
use crate::parser::ast::AstNode;
use crate::parser::{ParseError, ParseResult, Parser};

impl<'a> Parser<'a> {
    pub fn parse_expression(&mut self) -> ParseResult<AstNode> {
        if let Some(tok) = self.peek() {
            match tok.kind {
                TokenType::Bang | TokenType::Minus | TokenType::Plus => {
                    let op = tok.kind;
                    self.advance(); // consume operator
                    let expr = self.parse_expression()?; // recurse into right-hand side
                    return Ok(AstNode::UnaryExpr {
                        op,
                        expr: Box::new(expr),
                    });
                }
                _ => {}
            }
        }

        let mut left = match self.peek() {
            Some(tok) => match tok.kind {
                TokenType::Number => {
                    let tok = self.advance().unwrap();
                    AstNode::NumberLiteral(tok.value.parse::<i64>().unwrap())
                }
                TokenType::Identifier => {
                    let tok = self.advance().unwrap();
                    let name = tok.value.to_string();

                    // Check for function call
                    if self.peek_is(TokenType::OpenParen) {
                        self.advance(); // consume '('
                        let mut args = Vec::new();
                        while !self.peek_is(TokenType::CloseParen) {
                            args.push(self.parse_expression()?); // recursive parse
                            if !self.consume_if(TokenType::Comma) {
                                break;
                            }
                        }
                        self.expect(TokenType::CloseParen)?;
                        return Ok(AstNode::FunctionCall {
                            func: Box::new(AstNode::Identifier(name)), // wrap the function name as Identifier
                            args,                                      // your parsed arguments
                        });
                    }

                    AstNode::Identifier(name)
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

        // Inside parse_expression
        while let Some(tok) = self.peek() {
            if !Self::is_binary_op(tok.kind) {
                // <-- use Self::
                break;
            }
            let op = tok.kind;
            self.advance();
            let right = self.parse_expression()?;
            left = AstNode::BinaryExpr {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_array_literal(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::OpenBracket)?;

        let elements = self
            .parse_comma_separated(|parser| parser.parse_expression(), TokenType::CloseBracket)?;

        self.expect(TokenType::CloseBracket)?;
        Ok(AstNode::ArrayLiteral(elements))
    }

    fn parse_map_literal(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::OpenBrace)?;

        let entries = self.parse_comma_separated(
            |parser| {
                let key = parser.parse_expression()?;
                parser.expect(TokenType::Colon)?;
                let value = parser.parse_expression()?;
                Ok((key, value))
            },
            TokenType::CloseBrace,
        )?;

        self.expect(TokenType::CloseBrace)?;
        Ok(AstNode::MapLiteral(entries))
    }

    fn is_binary_op(kind: TokenType) -> bool {
        matches!(
            kind,
            TokenType::Gt
                | TokenType::Lt
                | TokenType::Eq
                | TokenType::EqEq
                | TokenType::EqEqEq
                | TokenType::NotEq
                | TokenType::NotEqEq
                | TokenType::GtEq
                | TokenType::LtEq
                | TokenType::And
                | TokenType::AndAnd
                | TokenType::Or
                | TokenType::OrOr
                | TokenType::Plus
                | TokenType::Minus
                | TokenType::Star
                | TokenType::Slash
                | TokenType::Percent
                | TokenType::PlusEq
                | TokenType::MinusEq
                | TokenType::StarEq
                | TokenType::SlashEq
                | TokenType::PercentEq
                | TokenType::Arrow
                | TokenType::FatArrow
                | TokenType::RangeExc
                | TokenType::RangeInc
        )
    }
}
