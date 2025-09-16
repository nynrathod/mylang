#[derive(Debug, Clone, Copy)]
pub enum TokenType {
    // Keywords
    Let,
    Function,
    Import,
    Struct,
    Enum,

    Map,
    If,
    Else,
    For,
    Return,
    Break,
    Continue,

    // Literals
    Number,
    String,
    Boolean,

    // Identifiers
    Identifier,

    // Operators
    Eq,      // `=`
    Plus,    // `+`,
    Minus,   // `-`
    Star,    // `*`
    Slash,   // `/`
    Percent, // `%`
    Bang,    // `!`
    Lt,      // `<`
    Gt,      // `>`
    And,     // `&`
    Or,      // `|`,

    // Delimiters
    OpenParen,    // `(`
    CloseParen,   // `)`
    OpenBrace,    // `{`
    CloseBrace,   // `}`
    OpenBracket,  // `[`
    CloseBracket, // `]`
    Comma,        // `,`
    Semi,         // `;`
    Dot,          // `.`
    Colon,        // `:`,
    At,           // `@`
    Pound,        // `#`
    Tilde,        // `~`
    Question,     // `?`
    Dollar,       // `$`

    Underscore, // `_`
}

#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub kind: TokenType,
    pub value: &'a str,
}
