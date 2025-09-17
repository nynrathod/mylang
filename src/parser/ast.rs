use crate::lexar::token::{Token, TokenType};

#[derive(Debug, Clone)]
pub enum TypeNode {
    Int,
    String,
    Bool,
    Array(Box<TypeNode>),              // Array<Int>, Array<String>
    Map(Box<TypeNode>, Box<TypeNode>), // Map<String, Int>
}

#[derive(Debug, Clone)]
pub enum AstNode {
    Program(Vec<AstNode>),
    NumberLiteral(i64),
    Identifier(String),
    StringLiteral(String),
    BoolLiteral(bool),
    ArrayLiteral(Vec<AstNode>),
    MapLiteral(Vec<(AstNode, AstNode)>),

    // 1+2 || a+2
    BinaryExpr {
        left: Box<AstNode>,
        op: TokenType,
        right: Box<AstNode>,
    },

    VarDecl {
        mutable: bool,
        type_annotation: Option<TypeNode>,
        name: String,
        value: Box<AstNode>,
    },
}
