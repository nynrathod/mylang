use crate::lexar::token::{Token, TokenType};
use crate::parser::ast::AstNode;

/// Error type for parser.
/// Used to signal parsing failures, such as unexpected tokens or premature end of input.
#[allow(dead_code)]
#[derive(Debug)]
pub enum ParseError {
    UnexpectedToken(String), // Used if a token doesn't match grammar expectation.
    EndOfInput,              // Used if input ends unexpectedly.
}

/// Standard result type for parsing.
/// Wraps either a successful parse result or a ParseError.
pub type ParseResult<T> = Result<T, ParseError>;

/// The Parser struct is the stateful engine. It consumes tokens (from lexar)
/// and builds AST nodes (for analyzer, codegen, etc).
pub struct Parser<'a> {
    pub tokens: &'a [Token<'a>], // Reference to a slice of tokens from lexar.
    pub current: usize,          // Current index; tracks progress through tokens.
}

impl<'a> Parser<'a> {
    /// Create a new parser for a given token stream.
    pub fn new(tokens: &'a [Token<'a>]) -> Self {
        Parser { tokens, current: 0 }
    }

    /// Peek at the current token without advancing.
    /// Used in almost every parse function to check what's next.
    pub fn peek(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.current)
    }

    /// Checks if the current token matches a given kind.
    pub(crate) fn peek_is(&self, kind: TokenType) -> bool {
        self.peek().map(|tok| tok.kind == kind).unwrap_or(false)
    }

    /// Advance to the next token and return the previous one.
    pub fn advance(&mut self) -> Option<&Token<'a>> {
        let tok = self.tokens.get(self.current);
        if tok.is_some() {
            self.current += 1;
        }
        tok
    }

    /// If the current token matches the given kind, consume it and return true.
    /// Otherwise, do nothing and return false.
    pub(crate) fn consume_if(&mut self, kind: TokenType) -> bool {
        if self.peek_is(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Expect the next token to be of a specific kind.
    /// If it matches, consume and return it.
    /// If not, return a ParseError.
    pub(crate) fn expect(&mut self, kind: TokenType) -> ParseResult<&Token<'a>> {
        match self.advance() {
            Some(tok) if tok.kind == kind => Ok(tok),
            Some(tok) => {
                println!("Token identifier: {:?}", tok);
                Err(ParseError::UnexpectedToken(format!(
                    "Expected {:?}, got {:?} ({:?})",
                    kind, tok.kind, tok.value
                )))
            }
            None => Err(ParseError::EndOfInput),
        }
    }

    /// Parses a single statement.
    /// Dispatches to the correct parse function based on the current token.
    /// Handles declarations, control flow, assignments, and expression statements.
    pub fn parse_statement(&mut self) -> ParseResult<AstNode> {
        match self.peek() {
            Some(tok) => match tok.kind {
                // Declarations
                TokenType::Let => self.parse_let_decl(),
                TokenType::Function => self.parse_functional_decl(),
                TokenType::Struct => self.parse_struct_decl(),
                TokenType::Enum => self.parse_enum_decl(),

                // Statements
                TokenType::If => self.parse_conditional_stmt(),
                TokenType::Return => self.parse_return(),
                TokenType::Print => self.parse_print(),
                TokenType::Break => self.parse_break(),
                TokenType::Continue => self.parse_continue(),
                TokenType::For => self.parse_for_stmt(),

                // Handles statements that start with an identifier.
                // These are NOT declarations (like 'let')
                // Only allow assignment statements for identifiers, not underscores.
                TokenType::Identifier => {
                    // Assignment statements: e.g., x = 5;
                    match self.parse_assignment() {
                        Ok(assign) => Ok(assign),
                        Err(_) => Err(ParseError::UnexpectedToken(
                            "Only single-variable assignment is allowed without 'let'".into(),
                        )),
                    }
                }

                // If the token doesn't match any known statement start, return an error.
                _ => Err(ParseError::UnexpectedToken(format!(
                    "Unexpected token: {:?}",
                    tok.kind
                ))),
            },
            None => Err(ParseError::EndOfInput),
        }
    }

    /// Parses a return statement.
    /// Syntax: `return expr1, expr2, ...;`
    /// Consumes 'return', then parses one or more expressions separated by commas, ending with a semicolon.
    fn parse_return(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::Return)?; // consume 'return'

        let mut values = Vec::new();

        loop {
            let expr = self.parse_expression()?;
            values.push(expr);

            match self.peek() {
                Some(tok) if tok.kind == TokenType::Comma => {
                    self.advance(); // consume ',' and continue parsing next expression
                }
                _ => break, // no more expressions
            }
        }

        self.expect(TokenType::Semi)?; // consume ';' at the end
        Ok(AstNode::Return { values })
    }

    /// Parses an entire program (sequence of statements).
    /// Keeps parsing statements until all tokens are consumed.
    pub fn parse_program(&mut self) -> ParseResult<AstNode> {
        let mut statements = Vec::new();
        while self.current < self.tokens.len() {
            let stmt = self.parse_statement()?;
            statements.push(stmt);
        }
        Ok(AstNode::Program(statements))
    }

    /// Parses a comma-separated list of items until an end token is reached.
    ///
    /// This is a generic helper for parsing lists such as function parameters,
    /// struct fields, enum variants, function parameters, return types
    ///
    /// - `parse_item`: a closure that parses a single item from the stream.
    /// - `end_token`: the token that marks the end of the list (e.g., `)` or `}`).
    ///
    /// Example usage:
    ///     parse_comma_separated(|p| p.parse_type_annotation(), TokenType::CloseParen)
    ///
    /// Parsing stops when the end token is encountered or when there are no more commas.
    /// Returns a vector of parsed items.
    pub fn parse_comma_separated<T, F>(
        &mut self,
        mut parse_item: F,
        end_token: TokenType,
    ) -> ParseResult<Vec<T>>
    where
        F: FnMut(&mut Self) -> ParseResult<T>,
    {
        let mut items = Vec::new();
        // Continue parsing items until the end token is found
        while !self.peek_is(end_token) {
            // Parse a single item using the provided closure
            items.push(parse_item(self)?);
            // If there's a comma, consume it and continue parsing the next item
            // If not, break the loop (list is finished)
            if !self.consume_if(TokenType::Comma) {
                break;
            }
        }
        Ok(items)
    }
}
