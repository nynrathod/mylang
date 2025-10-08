use crate::lexar::token::TokenType;
use crate::parser::ast::AstNode;
use crate::parser::{ParseError, ParseResult, Parser};

impl<'a> Parser<'a> {
    /// Entry point for parsing any expression.
    /// Delegates to precedence-based parser.
    pub fn parse_expression(&mut self) -> ParseResult<AstNode> {
        self.parse_expression_prec(0)
    }

    /// Parses an expression with operator precedence.
    /// Uses precedence climbing for correct operator grouping.
    /// - `min_prec`: minimum precedence to consider (used for recursion).
    /// Returns the parsed AST node for the expression.
    fn parse_expression_prec(&mut self, min_prec: u8) -> ParseResult<AstNode> {
        let mut left = if let Some(tok) = self.peek() {
            match tok.kind {
                // 🟡 TODO: Handles: -a, !b, +c (Not supported yet)
                TokenType::Bang | TokenType::Minus | TokenType::Plus => {
                    let op = tok.kind;
                    self.advance(); // consume operator
                    let expr = self.parse_expression_prec(7)?; // unary has high precedence
                    AstNode::UnaryExpr {
                        op,
                        expr: Box::new(expr),
                    }
                }
                // Primary expressions:
                // Handles: number, identifier, function call foo(a + b), string, boolean, array, map
                _ => self.parse_primary()?,
            }
        } else {
            return Err(ParseError::EndOfInput);
        };

        // Binary operator expressions:
        // Handles: a + b, x * y - z, a < b, a <= b, a > b, a >= b
        // 🟡 TODO: Operators && , || not supported yet
        // Groups operators according to precedence and left-to-right associativity.
        while let Some(tok) = self.peek() {
            // Get the precedence of the current operator token
            let prec = Self::get_precedence(tok.kind);

            // If the operator's precedence is lower than the minimum required,
            // or if it's not an operator (prec == 0), stop parsing further binary operators
            if prec < min_prec || prec == 0 {
                break;
            }

            let op = tok.kind;
            self.advance();

            // Recursively parse the right-hand side of the expression,
            // using higher precedence to ensure correct grouping
            let mut right = self.parse_expression_prec(prec + 1)?;

            // Build a BinaryExpr AST node with the current left and right expressions
            left = AstNode::BinaryExpr {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Handles literals (number, string, boolean), identifiers
    /// function calls, arrays, and maps.
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

                    // If followed by '(', parse as function call
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

    /// Example: `[1, 2, 3]`
    /// Uses parse_comma_separated to parse elements until ']'.
    fn parse_array_literal(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::OpenBracket)?;

        let elements = self
            .parse_comma_separated(|parser| parser.parse_expression(), TokenType::CloseBracket)?;
        self.expect(TokenType::CloseBracket)?;
        Ok(AstNode::ArrayLiteral(elements))
    }

    /// Parses a map/dictionary literal.
    /// Example: `{ "a": 1, "b": 2 }`
    /// Each entry is a key-value pair separated by ':' and entries separated by ','.
    fn parse_map_literal(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::OpenBrace)?;

        let entries = self.parse_comma_separated(
            |p| {
                let key = p.parse_expression()?; // parse key
                p.expect(TokenType::Colon)?; // expect ':'
                let value = p.parse_expression()?; // parse value
                Ok((key, value))
            },
            TokenType::CloseBrace,
        )?;
        println!("mapsss {:?}", entries);
        self.expect(TokenType::CloseBrace)?;
        Ok(AstNode::MapLiteral(entries))
    }

    /// Returns the precedence value for a given operator token.
    /// Higher numbers mean higher precedence.
    /// Used in precedence climbing for binary expressions.
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
