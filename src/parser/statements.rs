use crate::lexar::token::TokenType;
use crate::parser::ast::{AstNode, Pattern};
use crate::parser::{ParseError, ParseResult, Parser};

impl<'a> Parser<'a> {
    pub fn parse_assignment(&mut self) -> ParseResult<AstNode> {
        let mut patterns = Vec::new();

        // parse comma-separated LHS patterns
        loop {
            patterns.push(self.parse_pattern()?);

            if self.peek_is(TokenType::Comma) {
                self.advance(); // consume comma and continue
            } else {
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

            if self.peek_is(TokenType::Comma) {
                self.advance();
            } else {
                break;
            }
        }

        self.expect(TokenType::CloseParen)?;
        self.expect(TokenType::Semi)?;

        Ok(AstNode::Print { exprs: args })
    }

    pub fn parse_for_decl(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::For)?;

        // optional pattern
        let pattern = if self.peek_is(TokenType::OpenBrace) {
            Pattern::Wildcard // `for { ... }` infinite loop
        } else {
            // parse comma-separated patterns for tuples
            let mut patterns = Vec::new();
            loop {
                patterns.push(self.parse_pattern()?);
                if self.peek_is(TokenType::Comma) {
                    self.advance(); // consume comma
                } else {
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

        self.expect(TokenType::OpenBrace)?;
        let body = self.parse_block()?;

        Ok(AstNode::ForLoop {
            pattern,
            iterable,
            body,
        })
    }

    pub fn parse_pattern(&mut self) -> ParseResult<Pattern> {
        if let Some(tok) = self.peek() {
            match tok.kind {
                TokenType::Identifier | TokenType::Some => {
                    let name = tok.value.to_string();
                    self.advance();

                    if self.peek_is(TokenType::OpenParen) {
                        self.advance(); // consume '('
                        let mut elements = Vec::new();
                        while !self.peek_is(TokenType::CloseParen) {
                            elements.push(self.parse_pattern()?);
                            if self.peek_is(TokenType::Comma) {
                                self.advance();
                            } else {
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

                        if self.peek_is(TokenType::Comma) {
                            self.advance(); // consume ','
                        } else {
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

    pub fn parse_functional_decl(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::Function)?; // consume 'fn'

        // function name
        let name_tok = self.expect(TokenType::Identifier)?;
        let func_name = name_tok.value.to_string();

        let visibility = if func_name.chars().next().unwrap_or('a').is_uppercase() {
            "Public".to_string()
        } else {
            "Private".to_string()
        };

        self.expect(TokenType::OpenParen)?; // consume '('

        let mut params = Vec::new();

        while let Some(tok) = self.peek() {
            if tok.kind == TokenType::CloseParen {
                break; // done with parameters
            }

            // parse parameter name
            let param_name_tok = self.expect(TokenType::Identifier)?;
            let param_name = param_name_tok.value.to_string();

            // optional type
            let mut param_type = None;
            if let Some(tok) = self.peek() {
                if tok.kind == TokenType::Colon {
                    self.advance(); // consume ':'
                    param_type = Some(self.parse_type_annotation()?);
                }
            }

            params.push((param_name, param_type));

            // consume comma if present
            if let Some(tok) = self.peek() {
                if tok.kind == TokenType::Comma {
                    self.advance(); // consume ',' and continue
                } else if tok.kind != TokenType::CloseParen {
                    return Err(ParseError::UnexpectedToken(format!(
                        "Expected ',' or ')', got {:?}",
                        tok.kind
                    )));
                }
            }
        }

        self.expect(TokenType::CloseParen)?; // consume ')'

        // optional return type
        let mut return_type = None;
        if let Some(tok) = self.peek() {
            if tok.kind == TokenType::Arrow {
                // e.g., '->'
                self.advance();
                return_type = Some(self.parse_type_annotation()?); // or parse multiple types if you want
            }
        }

        self.expect(TokenType::OpenBrace)?; // consume '{'
        let body_block = self.parse_block()?; // parse function body

        Ok(AstNode::FunctionDecl {
            name: func_name,
            visibility,
            params,
            return_type,
            body: body_block,
        })
    }

    pub fn parse_conditional_decl(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::If)?; // consume 'if'

        // condition expression
        let condition = self.parse_expression()?;

        // then block
        self.expect(TokenType::OpenBrace)?;
        let then_block = self.parse_block()?; // parse statements until '}'

        // optional else / else if
        let mut else_branch = None;
        if let Some(tok) = self.peek() {
            if tok.kind == TokenType::Else {
                self.advance(); // consume 'else'

                if let Some(next) = self.peek() {
                    if next.kind == TokenType::If {
                        // else if -> recursive call
                        let elseif = self.parse_conditional_decl()?;
                        else_branch = Some(Box::new(elseif));
                    } else {
                        // else { ... }
                        self.expect(TokenType::OpenBrace)?;
                        let else_block = self.parse_block()?;
                        else_branch = Some(Box::new(AstNode::Block(else_block)));
                    }
                }
            }
        }

        Ok(AstNode::ConditionalDecl {
            condition: Box::new(condition),
            then_block,
            else_branch,
        })
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
    pub fn parse_enum_decl(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::Enum)?; // consume 'enum'

        let name_tok = self.expect(TokenType::Identifier)?;
        let struct_name = name_tok.value.to_string();

        self.expect(TokenType::OpenBrace)?;

        let mut variants = Vec::new();
        while let Some(tok) = self.peek() {
            if tok.kind == TokenType::CloseBrace {
                break;
            }
            // 1. Parse variant name
            let variant_name_tok = self.expect(TokenType::Identifier)?;
            let variant_name = variant_name_tok.value.to_string();

            let mut variant_data = None;

            // 2. Check if variant has associated types (inside parentheses)
            if let Some(tok) = self.peek() {
                if tok.kind == TokenType::OpenParen {
                    self.advance(); // consume '('

                    // Parse one or more type annotations for this variant
                    let types = self.parse_type_annotation()?; // or a loop if multiple types
                    variant_data = Some(types);

                    self.expect(TokenType::CloseParen)?; // consume ')'
                }
            }

            // 3. Push the variant to the enum
            variants.push((variant_name, variant_data));

            if let Some(tok) = self.peek() {
                if tok.kind == TokenType::Comma {
                    self.advance();
                }
            }
        }

        self.expect(TokenType::CloseBrace)?;

        Ok(AstNode::EnumDecl {
            name: struct_name,
            variants,
        })
    }

    pub fn parse_struct_decl(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::Struct)?; // consume 'struct'

        let name_tok = self.expect(TokenType::Identifier)?;
        let struct_name = name_tok.value.to_string();

        self.expect(TokenType::OpenBrace)?;

        let mut fields = Vec::new();
        while let Some(tok) = self.peek() {
            if tok.kind == TokenType::CloseBrace {
                break;
            }
            let field_name_tok = self.expect(TokenType::Identifier)?;
            let field_name = field_name_tok.value.to_string();

            self.expect(TokenType::Colon)?;
            let field_type = self.parse_type_annotation()?;

            fields.push((field_name, field_type));

            if let Some(tok) = self.peek() {
                if tok.kind == TokenType::Comma {
                    self.advance();
                }
            }
        }

        self.expect(TokenType::CloseBrace)?;

        Ok(AstNode::StructDecl {
            name: struct_name,
            fields,
        })
    }

    pub fn parse_var_decl(&mut self) -> ParseResult<AstNode> {
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
