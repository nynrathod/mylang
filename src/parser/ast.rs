#![allow(dead_code)]

use crate::lexar::token::TokenType;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeNode {
    Int,
    String,
    Bool,
    Array(Box<TypeNode>),              // Array<Int>, Array<String>
    Map(Box<TypeNode>, Box<TypeNode>), // Map<String, Int>
    Tuple(Vec<TypeNode>),
    Void,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Identifier(String),
    Tuple(Vec<Pattern>),
    Wildcard,
    EnumVariant(String, Box<Pattern>),
}

#[derive(Debug, Clone)]
pub enum AstNode {
    TupleIdentifier(Vec<String>),
    Program(Vec<AstNode>),
    NumberLiteral(i64),
    Identifier(String),
    StringLiteral(String),
    BoolLiteral(bool),
    ArrayLiteral(Vec<AstNode>),
    MapLiteral(Vec<(AstNode, AstNode)>),
    UnaryExpr {
        op: TokenType,
        expr: Box<AstNode>,
    },

    // 1+2 || a+2
    BinaryExpr {
        left: Box<AstNode>,
        op: TokenType,
        right: Box<AstNode>,
    },

    LetDecl {
        mutable: bool,
        type_annotation: Option<TypeNode>,
        pattern: Pattern,
        value: Box<AstNode>,
    },

    StructDecl {
        name: String,
        fields: Vec<(String, TypeNode)>,
    },

    EnumDecl {
        name: String,
        variants: Vec<(String, Option<TypeNode>)>,
    },

    ConditionalStmt {
        condition: Box<AstNode>,
        then_block: Vec<AstNode>,
        else_branch: Option<Box<AstNode>>,
    },
    Block(Vec<AstNode>),
    Return {
        values: Vec<AstNode>, // multiple expressions can be returned
    },
    Print {
        exprs: Vec<AstNode>,
    },
    Break,
    Continue,

    Assignment {
        pattern: Pattern,
        value: Box<AstNode>,
    },

    FunctionDecl {
        name: String,
        visibility: String,
        params: Vec<(String, Option<TypeNode>)>,
        return_type: Option<TypeNode>,
        body: Vec<AstNode>,
    },
    FunctionCall {
        func: Box<AstNode>, // usually an Identifier node
        args: Vec<AstNode>,
    },

    ForLoopStmt {
        pattern: Pattern,
        iterable: Option<Box<AstNode>>,
        body: Vec<AstNode>, // keep Vec (block already returns Vec)
    },

    TupleLiteral(Vec<AstNode>),
}
