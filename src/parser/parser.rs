use crate::lexar::token::{Token, TokenType};
use crate::parser::ast::AstNode;
use std::fmt;

/// Error type for parser.
/// Used to signal parsing failures, such as unexpected tokens or premature end of input.
#[allow(dead_code)]
#[derive(Debug)]
pub enum ParseError {
    UnexpectedToken(String), // Legacy: without position
    UnexpectedTokenAt {
        msg: String,
        line: usize,
        col: usize,
    },
    EndOfInput, // Used if input ends unexpectedly.
}

/// Standard result type for parsing.
/// Wraps either a successful parse result or a ParseError.
pub type ParseResult<T> = Result<T, ParseError>;

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedToken(msg) => write!(f, "parse error: {}", msg),
            ParseError::UnexpectedTokenAt { msg, line, col } => {
                write!(f, "parse error at {}:{}: {}", line, col, msg)
            }
            ParseError::EndOfInput => write!(f, "parse error: unexpected end of input"),
        }
    }
}

/// The Parser struct is the stateful engine. It consumes tokens (from lexar)
/// and builds AST nodes (for analyzer, codegen, etc).
#[derive(Debug)]
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
            Some(tok) => Err(ParseError::UnexpectedTokenAt {
                msg: format!("Expected {:?}, got {:?} ({:?})", kind, tok.kind, tok.value),
                line: tok.line,
                col: tok.col,
            }),
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

                // Import statement
                TokenType::Import => self.parse_import(),

                // Statements
                TokenType::If => self.parse_conditional_stmt(),
                TokenType::For => self.parse_for_stmt(),
                TokenType::Return => self.parse_return(),
                TokenType::Break => self.parse_break(),
                TokenType::Continue => self.parse_continue(),
                TokenType::Print => self.parse_print(),

                // Handles statements that start with an identifier.
                // Could be assignment (x = 5;) or expression statement (abc();)
                TokenType::Identifier => {
                    // Try to parse as expression first (handles function calls)
                    let expr = self.parse_expression()?;

                    // Check if it's followed by '=' (assignment)
                    if self.peek_is(TokenType::Eq) {
                        self.advance(); // consume '='
                        let value = self.parse_expression()?;
                        self.expect(TokenType::Semi)?;

                        // Extract identifier from expr for assignment
                        if let AstNode::Identifier(name) = expr {
                            return Ok(AstNode::Assignment {
                                pattern: crate::parser::ast::Pattern::Identifier(name),
                                value: Box::new(value),
                            });
                        } else {
                            return Err(ParseError::UnexpectedToken(
                                "Only single-variable assignment is allowed without 'let'".into(),
                            ));
                        }
                    } else {
                        // It's an expression statement (like function call)
                        self.expect(TokenType::Semi)?;
                        return Ok(expr);
                    }
                }

                TokenType::Number | TokenType::Float => {
                    // Allow number/float literals as statements (for testing, REPL, etc.)
                    let expr = self.parse_expression()?;
                    self.expect(TokenType::Semi)?;
                    Ok(expr)
                }

                // If the token doesn't match any known statement start, check for Unknown token and handle error.
                _ => Err(ParseError::UnexpectedTokenAt {
                    msg: format!("Unexpected token: {:?}", tok.kind),
                    line: tok.line,
                    col: tok.col,
                }),
            },
            None => Err(ParseError::EndOfInput),
        }
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

    /// Parses an import statement.
    /// Syntax: import models::User::Createuser;
    /// Path will be ["models", "User"], symbol will be Some("Createuser")
    pub fn parse_import(&mut self) -> ParseResult<AstNode> {
        self.expect(TokenType::Import)?;
        // Parse all identifiers separated by ::
        let mut all_parts = Vec::new();

        // Parse first identifier
        let first = self.expect(TokenType::Identifier)?;
        all_parts.push(first.value.to_string());

        // Parse :: separated path
        while self.peek_is(TokenType::Colon) {
            self.advance(); // :
            self.expect(TokenType::Colon)?; // second :
            let next = self.expect(TokenType::Identifier)?;
            all_parts.push(next.value.to_string());
        }

        self.expect(TokenType::Semi)?;

        // Split into path and symbol
        // Last element is the symbol (function/type name)
        // Everything before is the module path (file path)
        let symbol = all_parts.pop(); // Remove and return last element
        let path = all_parts; // Remaining elements are the path

        Ok(AstNode::Import { path, symbol })
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

    /// Parses a single simple expression, including literals.
    pub fn parse_simple_expression(&mut self) -> ParseResult<AstNode> {
        match self.peek() {
            Some(tok) => {
                match tok.kind {
                    TokenType::Number => {
                        let tok = self.advance().unwrap();
                        let value_str = tok.value;
                        let value_line = tok.line;
                        let value_col = tok.col;
                        // Mutable borrow ends here, now peek is allowed
                        if let Some(next) = self.peek() {
                            if next.kind == TokenType::Dot
                                || next.kind == TokenType::RangeExc
                                || next.kind == TokenType::RangeInc
                            {
                                return Err(ParseError::UnexpectedTokenAt {
                                    msg: format!(
                                        "Invalid number/range/dot sequence after number: {:?}",
                                        next.kind
                                    ),
                                    line: next.line,
                                    col: next.col,
                                });
                            }
                        }
                        let value = value_str.parse::<i32>().map_err(|_| {
                            ParseError::UnexpectedTokenAt {
                                msg: format!("Invalid integer literal: {}", value_str),
                                line: value_line,
                                col: value_col,
                            }
                        })?;
                        Ok(AstNode::NumberLiteral(value))
                    }
                    TokenType::Float => {
                        let tok = self.advance().unwrap();
                        let value_str = tok.value;
                        let value_line = tok.line;
                        let value_col = tok.col;
                        // Mutable borrow ends here, now peek is allowed
                        if let Some(next) = self.peek() {
                            if next.kind == TokenType::Dot
                                || next.kind == TokenType::RangeExc
                                || next.kind == TokenType::RangeInc
                            {
                                return Err(ParseError::UnexpectedTokenAt {
                                    msg: format!(
                                        "Invalid number/range/dot sequence after float: {:?}",
                                        next.kind
                                    ),
                                    line: next.line,
                                    col: next.col,
                                });
                            }
                        }
                        let value = value_str.parse::<f64>().map_err(|_| {
                            ParseError::UnexpectedTokenAt {
                                msg: format!("Invalid float literal: {}", value_str),
                                line: value_line,
                                col: value_col,
                            }
                        })?;
                        Ok(AstNode::FloatLiteral(value))
                    }
                    TokenType::String => {
                        let tok = self.advance().unwrap();
                        Ok(AstNode::StringLiteral(tok.value.to_string()))
                    }
                    // ... handle other expression types as before ...
                    _ => {
                        // Fallback to existing logic or error
                        Err(ParseError::UnexpectedTokenAt {
                            msg: format!("Unexpected token in expression: {:?}", tok.kind),
                            line: tok.line,
                            col: tok.col,
                        })
                    }
                }
            }
            None => Err(ParseError::EndOfInput),
        }
    }
}
