use crate::lexar::token::TokenType;
use crate::parser::ast::{AstNode, Pattern};
use crate::parser::{ParseError, ParseResult, Parser};

impl<'a> Parser<'a> {
    pub fn parse_assignment(&mut self) -> ParseResult<AstNode> {
        let mut patterns = Vec::new();

        // parse comma-separated LHS patterns
        loop {
            patterns.push(self.parse_pattern()?);

            if !self.consume_if(TokenType::Comma) {
                break;
            }
        }

        // now we must have '=' after LHS
        self.expect(TokenType::Eq)?;

        // parse RHS expression (could be a function call or any expression)
        let rhs = self.parse_expression()?;

        // expect semicolon to finish the statement
        self.expect(TokenType::Semi)?;

        let lhs_pattern = if patterns.len() == 1 {
            patterns.remove(0)
        } else {
            Pattern::Tuple(patterns)
        };

        Ok(AstNode::Assignment {
            pattern: lhs_pattern,
            value: Box::new(rhs),
        })
    }

    pub fn parse_break(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::Break)?; // consume 'break'
        self.expect(TokenType::Semi)?; // consume ';'
        Ok(AstNode::Break)
    }

    pub fn parse_continue(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::Continue)?; // consume 'continue'
        self.expect(TokenType::Semi)?; // consume ';'
        Ok(AstNode::Continue)
    }

    pub fn parse_print(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::Print)?;
        self.expect(TokenType::OpenParen)?;

        let mut args = Vec::new();
        while !self.peek_is(TokenType::CloseParen) {
            let expr = self.parse_expression()?;
            args.push(expr);

            if !self.consume_if(TokenType::Comma) {
                break;
            }
        }

        self.expect(TokenType::CloseParen)?;
        self.expect(TokenType::Semi)?;

        Ok(AstNode::Print { exprs: args })
    }

    pub fn parse_for_stmt(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::For)?;

        // optional pattern
        let pattern = if self.peek_is(TokenType::OpenBrace) {
            Pattern::Wildcard // `for { ... }` infinite loop
        } else {
            // parse comma-separated patterns for tuples
            let mut patterns = Vec::new();
            loop {
                patterns.push(self.parse_pattern()?);
                if !self.consume_if(TokenType::Comma) {
                    break;
                }
            }

            if patterns.len() == 1 {
                patterns.remove(0)
            } else {
                Pattern::Tuple(patterns)
            }
        };

        // optional iterable
        let iterable = if self.peek_is(TokenType::In) {
            self.advance(); // consume 'in'
            Some(Box::new(self.parse_expression()?))
        } else if !self.peek_is(TokenType::OpenBrace) {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        let body = self.parse_braced_block()?;

        Ok(AstNode::ForLoopStmt {
            pattern,
            iterable,
            body,
        })
    }

    pub fn parse_conditional_stmt(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::If)?; // consume 'if'

        // condition expression
        let condition = self.parse_expression()?;

        // then block
        let then_block = self.parse_braced_block()?; // parse statements until '}'

        // optional else / else if
        let mut else_branch = None;
        if let Some(tok) = self.peek() {
            if tok.kind == TokenType::Else {
                self.advance(); // consume 'else'

                if let Some(next) = self.peek() {
                    if next.kind == TokenType::If {
                        // else if -> recursive call
                        let elseif = self.parse_conditional_stmt()?;
                        else_branch = Some(Box::new(elseif));
                    } else {
                        // else { ... }

                        let else_block = self.parse_braced_block()?;
                        else_branch = Some(Box::new(AstNode::Block(else_block)));
                    }
                }
            }
        }

        Ok(AstNode::ConditionalStmt {
            condition: Box::new(condition),
            then_block,
            else_branch,
        })
    }

    pub fn parse_comma_separated<T>(
        &mut self,
        parse_item: impl Fn(&mut Self) -> ParseResult<T>,
        end_token: TokenType,
    ) -> ParseResult<Vec<T>> {
        let mut items = Vec::new();

        while !self.peek_is(end_token) {
            let item = parse_item(self)?;
            items.push(item);

            // If no comma, we are done
            if !self.consume_if(TokenType::Comma) {
                break;
            }
        }

        Ok(items)
    }

    pub fn parse_pattern(&mut self) -> ParseResult<Pattern> {
        if let Some(tok) = self.peek() {
            match tok.kind {
                // Open-Close Paren Checking => for Some(x,y) {}
                // Else checking _, a = GetUser();
                TokenType::Identifier | TokenType::Some => {
                    let name = tok.value.to_string();
                    self.advance();

                    if self.peek_is(TokenType::OpenParen) {
                        self.advance(); // consume '('
                        let mut elements = Vec::new();
                        while !self.peek_is(TokenType::CloseParen) {
                            elements.push(self.parse_pattern()?);
                            if !self.consume_if(TokenType::Comma) {
                                break;
                            }
                        }
                        self.expect(TokenType::CloseParen)?;
                        let inner_pattern = if elements.len() == 1 {
                            Box::new(elements.remove(0))
                        } else {
                            Box::new(Pattern::Tuple(elements))
                        };
                        return Ok(Pattern::EnumVariant(name, inner_pattern));
                    }

                    Ok(Pattern::Identifier(name))
                }

                TokenType::OpenParen => {
                    self.advance(); // consume '('
                    let mut elements = Vec::new();

                    while !self.peek_is(TokenType::CloseParen) {
                        let pat = self.parse_pattern()?;
                        elements.push(pat);

                        if !self.consume_if(TokenType::Comma) {
                            break;
                        }
                    }

                    self.expect(TokenType::CloseParen)?;
                    Ok(Pattern::Tuple(elements))
                }

                TokenType::Underscore => {
                    self.advance();
                    Ok(Pattern::Wildcard)
                }

                _ => Err(ParseError::UnexpectedToken(format!(
                    "Unexpected token {:?} in pattern",
                    tok.kind
                ))),
            }
        } else {
            Err(ParseError::EndOfInput)
        }
    }

    fn parse_block(&mut self) -> ParseResult<Vec<AstNode>> {
        let mut stmts = Vec::new();
        while let Some(tok) = self.peek() {
            if tok.kind == TokenType::CloseBrace {
                break;
            }
            stmts.push(self.parse_statement()?);
        }
        self.expect(TokenType::CloseBrace)?; // consume '}'
        Ok(stmts)
    }

    pub fn parse_braced_block(&mut self) -> ParseResult<Vec<AstNode>> {
        self.expect(TokenType::OpenBrace)?;
        self.parse_block()
    }
}
