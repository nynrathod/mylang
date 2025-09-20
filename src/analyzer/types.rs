#![allow(dead_code)]

use crate::parser::ast::TypeNode;

#[derive(Debug)]
pub struct TypeMismatch {
    pub expected: TypeNode,
    pub found: TypeNode,
}

#[derive(Debug)]
pub enum SemanticError {
    VariableRedeclaration {
        name: String,
    },
    UndeclaredVariable {
        name: String,
    },
    VarTypeMismatch(TypeMismatch),
    OperatorTypeMismatch(TypeMismatch),
    EmptyCollectionTypeInferenceError(TypeMismatch),
    ReturnTypeMismatch {
        function: String,
        mismatch: TypeMismatch,
    },
    InvalidConditionType(TypeMismatch),
    InvalidPublicName {
        name: String,
    },
    FunctionParamRedeclaration {
        name: String,
    },
    FunctionRedeclaration {
        name: String,
    },
    MissingParamType {
        name: String,
    },
    MissingFunctionReturn {
        function: String,
    },
    InvalidReturnInVoidFunction {
        function: String,
    },
    OutOfScopeVariable {
        name: String,
    },
}
