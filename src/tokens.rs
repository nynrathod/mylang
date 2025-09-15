#[derive(Debug, Clone, Copy)]
pub enum TokenType {
    // Keywords
    Let,
    Function,
    Import,
    Struct,
    Enum,
    // Array,
    Map,

    // Literals
    Identifier,
    Number,
    String,
    Boolean,

    // Operators
    Eq,           // `=`
    Plus,         // `+`,
    Minus,        // `-`
    Star,         // `*`
    Slash,        // `/`
    Percent,      // `%`
    Bang,         // `!`
    Lt,           // `<`
    Gt,           // `>`
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
    And,          // `&`
    Or,           // `|`,
    Underscore,   // `_`
}

#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub kind: TokenType,
    pub value: &'a str,
}
