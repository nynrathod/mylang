use crate::lexar::token::TokenType;
use crate::parser::ast::{AstNode, Pattern, TypeNode};
use crate::parser::{ParseError, ParseResult, Parser};

impl<'a> Parser<'a> {
    pub fn parse_enum_decl(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::Enum)?; // consume 'enum'

        let struct_name = self.expect_ident()?;

        self.expect(TokenType::OpenBrace)?;

        let mut variants = Vec::new();
        while let Some(tok) = self.peek() {
            if tok.kind == TokenType::CloseBrace {
                break;
            }
            // 1. Parse variant name
            let variant_name = self.expect_ident()?;

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

        let struct_name = self.expect_ident()?;

        self.expect(TokenType::OpenBrace)?;

        let mut fields = Vec::new();
        while let Some(tok) = self.peek() {
            if tok.kind == TokenType::CloseBrace {
                break;
            }
            let field_name = self.expect_ident()?;

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

    pub fn parse_let_pattern(&mut self) -> ParseResult<Pattern> {
        let mut patterns = Vec::new();

        // Parse a `let` pattern, which can be a single identifier or a tuple of identifiers.
        // - `x` → single identifier
        // - `x, y, z` → tuple pattern
        loop {
            // Parse a single pattern (could be identifier, wildcard, or nested tuple)
            patterns.push(self.parse_pattern()?);
            // If there's a comma, continue parsing more patterns
            // Otherwise, break the loop
            if !self.consume_if(TokenType::Comma) {
                break;
            }
        }

        if patterns.len() == 1 {
            Ok(patterns.remove(0))
        } else {
            Ok(Pattern::Tuple(patterns))
        }
    }

    pub fn parse_let_decl(&mut self) -> ParseResult<AstNode> {
        let first_tok = self.advance().ok_or(ParseError::EndOfInput)?;
        if first_tok.kind != TokenType::Let {
            return Err(ParseError::UnexpectedToken("Expected 'let'".into()));
        }

        // Check if keyword found
        let mut mutable = false;
        if let Some(tok) = self.peek() {
            if tok.kind == TokenType::Mut {
                self.advance(); // consume 'mut'
                mutable = true;
            }
        }

        // Accept a comma-separated pattern list
        let pattern = self.parse_let_pattern()?;

        // Append type if not explicitly provided
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

        Ok(AstNode::LetDecl {
            mutable,
            type_annotation,
            pattern,
            value: Box::new(value),
        })
    }

    pub fn parse_functional_decl(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::Function)?; // consume 'fn'

        // function name
        let func_name = self.expect_ident()?;

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

            let param_name = self.expect_ident()?;

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
                return_type = Some(self.parse_return_type()?);
                // or parse multiple types if you want
            }
        }

        let body_block = self.parse_braced_block()?; // parse function body

        Ok(AstNode::FunctionDecl {
            name: func_name,
            visibility,
            params,
            return_type,
            body: body_block,
        })
    }

    pub(crate) fn parse_return_type(&mut self) -> ParseResult<TypeNode> {
        if let Some(tok) = self.peek() {
            // Idenntify multiple return type while function declare
            // Ex., fn Foo(a: Int, b: String) -> (String, String) {}
            if tok.kind == TokenType::OpenParen {
                // multiple return types
                self.advance(); // consume '('
                let mut types = Vec::new();
                loop {
                    types.push(self.parse_type_annotation()?);

                    if let Some(tok) = self.peek() {
                        match tok.kind {
                            TokenType::Comma => {
                                self.advance();
                            }

                            TokenType::CloseParen => {
                                self.advance(); // consume ')'
                                break;
                            }
                            _ => {
                                return Err(ParseError::UnexpectedToken(format!(
                                    "Expected ',' or ')', got {:?}",
                                    tok.kind
                                )));
                            }
                        }
                    } else {
                        return Err(ParseError::UnexpectedToken(
                            "Unexpected end of input in return type tuple".into(),
                        ));
                    }
                }
                Ok(TypeNode::Tuple(types))
            } else {
                // single return type
                self.parse_type_annotation()
            }
        } else {
            Err(ParseError::EndOfInput)
        }
    }

    pub(crate) fn parse_type_annotation(&mut self) -> ParseResult<TypeNode> {
        if self.peek_is(TokenType::OpenBracket) {
            self.advance(); // consume '['
            let inner = self.parse_type_annotation()?;
            self.expect(TokenType::CloseBracket)?;
            Ok(TypeNode::Array(Box::new(inner)))
        } else if self.peek_is(TokenType::OpenBrace) {
            self.advance(); // consume '{'
            let key = self.parse_type_annotation()?;
            self.expect(TokenType::Comma)?;
            let value = self.parse_type_annotation()?;
            self.expect(TokenType::CloseBrace)?;
            Ok(TypeNode::Map(Box::new(key), Box::new(value)))
        } else if self.peek_is(TokenType::Identifier) {
            let tok = self.advance().unwrap();
            match tok.value {
                "Int" => Ok(TypeNode::Int),
                "Str" => Ok(TypeNode::String),
                "Bool" => Ok(TypeNode::Bool),
                "Void" => Ok(TypeNode::Void),
                other => {
                    // Accept any previously declared struct as type
                    Ok(TypeNode::TypeRef(other.to_string()))
                }
                _ => Err(ParseError::UnexpectedToken(format!(
                    "Expected type identifier, got {}",
                    tok.value
                ))),
            }
        } else {
            Err(ParseError::UnexpectedToken(
                "Expected type annotation".into(),
            ))
        }
    }

    fn expect_ident(&mut self) -> ParseResult<String> {
        let tok = self.expect(TokenType::Identifier)?;
        Ok(tok.value.to_string())
    }
}
