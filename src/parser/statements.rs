use crate::lexar::token::TokenType;
use crate::parser::ast::AstNode;
use crate::parser::{ParseError, ParseResult, Parser};

impl<'a> Parser<'a> {
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
