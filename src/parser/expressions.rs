use crate::lexar::token::TokenType;
use crate::parser::ast::AstNode;
use crate::parser::{ParseError, ParseResult, Parser};

impl<'a> Parser<'a> {
    pub fn parse_expression(&mut self) -> ParseResult<AstNode> {
        self.parse_expression_prec(0)
    }

    fn parse_expression_prec(&mut self, min_prec: u8) -> ParseResult<AstNode> {
        // Parse unary first
        let mut left = if let Some(tok) = self.peek() {
            match tok.kind {
                TokenType::Bang | TokenType::Minus | TokenType::Plus => {
                    let op = tok.kind;
                    self.advance(); // consume operator
                    let expr = self.parse_expression_prec(7)?; // unary has high precedence
                    AstNode::UnaryExpr {
                        op,
                        expr: Box::new(expr),
                    }
                }
                _ => self.parse_primary()?,
            }
        } else {
            return Err(ParseError::EndOfInput);
        };

        // Handle binary operators with precedence
        while let Some(tok) = self.peek() {
            let prec = Self::get_precedence(tok.kind);
            if prec < min_prec || prec == 0 {
                break;
            }

            let op = tok.kind;
            self.advance();
            let mut right = self.parse_expression_prec(prec + 1)?;
            left = AstNode::BinaryExpr {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_primary(&mut self) -> ParseResult<AstNode> {
        if let Some(tok) = self.peek() {
            match tok.kind {
                TokenType::Number => {
                    let tok = self.advance().unwrap();
                    Ok(AstNode::NumberLiteral(tok.value.parse::<i32>().unwrap()))
                }
                TokenType::Identifier => {
                    let tok = self.advance().unwrap();
                    let name = tok.value.to_string();

                    if self.peek_is(TokenType::OpenParen) {
                        self.advance(); // consume '('
                        let args = self.parse_comma_separated(
                            |p| p.parse_expression(),
                            TokenType::CloseParen,
                        )?;
                        self.expect(TokenType::CloseParen)?;
                        return Ok(AstNode::FunctionCall {
                            func: Box::new(AstNode::Identifier(name)),
                            args,
                        });
                    }

                    Ok(AstNode::Identifier(name))
                }
                TokenType::String => {
                    let tok = self.advance().unwrap();
                    Ok(AstNode::StringLiteral(tok.value.to_string()))
                }
                TokenType::Boolean => {
                    let tok = self.advance().unwrap();
                    let value = tok.value == "true";
                    Ok(AstNode::BoolLiteral(value))
                }
                TokenType::OpenBracket => self.parse_array_literal(),
                TokenType::OpenBrace => self.parse_map_literal(),
                _ => Err(ParseError::UnexpectedToken(format!(
                    "Expected primary expression, got {:?}",
                    tok.kind
                ))),
            }
        } else {
            Err(ParseError::EndOfInput)
        }
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

    fn get_precedence(op: TokenType) -> u8 {
        match op {
            TokenType::OrOr => 1,
            TokenType::AndAnd => 2,
            TokenType::EqEq | TokenType::NotEq => 3,
            TokenType::Lt | TokenType::Gt | TokenType::LtEq | TokenType::GtEq => 4,
            TokenType::Plus | TokenType::Minus => 5,
            TokenType::Star | TokenType::Slash | TokenType::Percent => 6,
            TokenType::RangeExc | TokenType::RangeInc => 7,
            _ => 0,
        }
    }
}
