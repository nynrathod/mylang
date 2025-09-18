#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenType {
    // Keywords
    Let, // immutable
    Var, // mutable
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
    In,
    Some,

    // Literals
    Number,
    String,
    Boolean,

    // Identifiers
    Identifier,

    // Operators
    Plus,    // `+`,
    Minus,   // `-`
    Star,    // `*`
    Slash,   // `/`
    Percent, // `%`

    Bang,      // `!`
    Lt,        // `<`
    Gt,        // `>`
    And,       // `&`
    Or,        // `|`,
    Eq,        // `=`
    EqEq,      // `==`
    EqEqEq,    // `===`
    NotEq,     // `!=`
    NotEqEq,   // `!==`
    GtEq,      // `>=`
    LtEq,      // `<=`
    AndAnd,    // `&&`
    OrOr,      // `||`
    PlusEq,    // `+=`
    MinusEq,   // `-=`
    StarEq,    // `*=`
    SlashEq,   // `/=`
    PercentEq, // `%=`
    Arrow,     // `->`
    FatArrow,  // `=>`

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
    RangeInc,     // `..`
    RangeExc,     // `..=`
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
