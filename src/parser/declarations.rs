use crate::lexar::token::TokenType;
use crate::parser::ast::{AstNode, TypeNode};
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

    pub fn parse_var_decl(&mut self) -> ParseResult<AstNode> {
        let first_tok = self.advance().ok_or(ParseError::EndOfInput)?;
        let mutable = match first_tok.kind {
            TokenType::Let => false,
            TokenType::Var => true,
            _ => return Err(ParseError::UnexpectedToken("Expected let or var".into())),
        };

        let name = self.expect_ident()?;

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
            name,
            type_annotation,
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
        let tok = self.peek().ok_or(ParseError::EndOfInput)?;
        if tok.kind != TokenType::Identifier {
            return Err(ParseError::UnexpectedToken(
                "Expected type Identifier".into(),
            ));
        }

        let tok = self.advance().unwrap();

        match tok.value {
            "Int" => Ok(TypeNode::Int),
            "String" => Ok(TypeNode::String),
            "Bool" => Ok(TypeNode::Bool),
            "Array" => {
                self.expect(TokenType::Lt)?;
                let inner_type = self.parse_type_annotation()?;
                self.expect(TokenType::Gt)?;
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
            _ => Err(ParseError::UnexpectedToken(format!(
                "Expected type identifier, got {}",
                tok.value
            ))),
        }
    }

    fn expect_ident(&mut self) -> ParseResult<String> {
        let tok = self.expect(TokenType::Identifier)?;
        Ok(tok.value.to_string())
    }
}
