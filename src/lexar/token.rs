#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenType {
    // --- Keywords ---
    Let,      // let
    Mut,      // mutable keyword for let
    Function, // function
    Import,   // import
    Struct,   // struct
    Enum,     // enum
    If,       // if
    Else,     // else
    For,      // for
    In,       // in
    Return,   // return
    Break,    // break
    Continue, // continue
    Some,     // some
    Print,    // print

    // --- Literals ---
    Number,
    String,
    Boolean,

    // --- Identifier ---
    Identifier,

    // --- Operators ---
    // Arithmetic
    Plus,    // +
    Minus,   // -
    Star,    // *
    Slash,   // /
    Percent, // %

    // Assignment
    Eq,        // =
    PlusEq,    // +=
    MinusEq,   // -=
    StarEq,    // *=
    SlashEq,   // /=
    PercentEq, // %=

    // Comparison
    EqEq,    // ==
    EqEqEq,  // ===
    NotEq,   // !=
    NotEqEq, // !==
    Gt,      // >
    Lt,      // <
    GtEq,    // >=
    LtEq,    // <=

    // Logical
    Bang,   // !
    And,    // &
    Or,     // |
    AndAnd, // &&
    OrOr,   // ||

    // Arrow
    Arrow,    // ->
    FatArrow, // =>

    // --- Delimiters & Punctuation ---
    OpenParen,    // (
    CloseParen,   // )
    OpenBrace,    // {
    CloseBrace,   // }
    OpenBracket,  // [
    CloseBracket, // ]
    Comma,        // ,
    Semi,         // ;
    Dot,          // .
    RangeInc,     // ..
    RangeExc,     // ..=
    Colon,        // :
    At,           // @
    Pound,        // #
    Tilde,        // ~
    Question,     // ?
    Dollar,       // $
    Underscore,   // _
}

#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub kind: TokenType,
    pub value: &'a str,
}
