use crate::lexar::token::TokenType;
use crate::parser::ast::AstNode;
use crate::parser::{ParseError, ParseResult, Parser};

impl<'a> Parser<'a> {
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
