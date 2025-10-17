#![allow(dead_code)]

use crate::parser::ast::{AstNode, Pattern, TypeNode};

#[derive(Debug)]
pub struct TypeMismatch {
    pub expected: TypeNode,
    pub found: TypeNode,
    pub value: Option<Box<AstNode>>,
}

#[derive(Debug)]
pub struct NamedError {
    pub name: String,
}

#[derive(Debug)]
pub enum SemanticError {
    // Variable Declaration/Assignment Errors
    VariableRedeclaration(NamedError),
    UndeclaredVariable(NamedError),
    VarTypeMismatch(TypeMismatch),
    TupleAssignmentMismatch {
        expected: usize,
        found: usize,
    },
    InvalidAssignmentTarget {
        target: String,
    },
    OutOfScopeVariable(NamedError),
    InvalidMapKeyType {
        found: TypeNode,
        expected: TypeNode,
    },

    // Function Declaration/Call Errors
    FunctionRedeclaration(NamedError),
    FunctionParamRedeclaration(NamedError),
    MissingParamType(NamedError),
    UndeclaredFunction(NamedError),
    InvalidFunctionCall {
        func: String,
    },
    FunctionArgumentMismatch {
        name: String,
        expected: usize,
        found: usize,
    },
    FunctionArgumentTypeMismatch {
        name: String,
        expected: TypeNode,
        found: TypeNode,
    },
    MissingFunctionReturn {
        function: String,
    },
    InvalidReturnInVoidFunction {
        function: String,
    },
    ReturnTypeMismatch {
        function: String,
        mismatch: TypeMismatch,
    },
    InvalidPublicName(NamedError),

    // Type/Operator Errors
    OperatorTypeMismatch(TypeMismatch),
    EmptyCollectionTypeInferenceError(TypeMismatch),
    InvalidConditionType(TypeMismatch),

    // Print
    InvalidPrintType {
        found: TypeNode,
    },
    UnexpectedNode {
        expected: String,
    },

    // For
    InvalidForIterableType {
        found: TypeNode,
    },
    ArrayIterationWithTuple {
        tuple_len: usize,
    },
    MapIterationRequiresTuple,
    NonIterableType {
        found: TypeNode,
    },
    InfiniteLoopWithPattern {
        pattern: Pattern,
    },
    RangeIterationTypeMismatch {
        expected: TypeNode,
        found: TypeNode,
    },

    // Struct
    StructRedeclaration(NamedError),
    DuplicateField {
        struct_name: String,
        field: String,
    },

    // Enum
    EnumRedeclaration(NamedError),

    DuplicateEnumVariant {
        enum_name: String,
        variant: String,
    },

    // --- Module Import Errors ---
    ModuleNotFound(String),
    ParseError,
}
