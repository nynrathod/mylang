#![allow(dead_code)]

use crate::parser::ast::TypeNode;

#[derive(Debug)]
pub struct TypeMismatch {
    pub expected: TypeNode,
    pub found: TypeNode,
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
}
