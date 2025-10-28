#![allow(dead_code)]

use crate::parser::ast::{AstNode, Pattern, TypeNode};
use std::fmt;

#[derive(Debug)]
pub struct TypeMismatch {
    pub expected: TypeNode,
    pub found: TypeNode,
    pub value: Option<Box<AstNode>>,
    pub line: Option<usize>,
    pub col: Option<usize>,
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

    ParseErrorInModule {
        file: String,
        error: String,
    },
}

impl fmt::Display for TypeNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeNode::Float => write!(f, "Float"),
            TypeNode::Int => write!(f, "Int"),
            TypeNode::String => write!(f, "String"),
            TypeNode::Bool => write!(f, "Bool"),
            TypeNode::Array(t) => write!(f, "Array<{}>", t),
            TypeNode::Map(k, v) => write!(f, "Map<{}, {}>", k, v),
            TypeNode::Tuple(ts) => {
                let parts: Vec<String> = ts.iter().map(|t| t.to_string()).collect();
                write!(f, "({})", parts.join(", "))
            }
            TypeNode::Void => write!(f, "Void"),
            TypeNode::Struct(name, _) => write!(f, "Struct {}", name),
            TypeNode::Enum(name, _) => write!(f, "Enum {}", name),
            TypeNode::Range(a, b, inclusive) => write!(
                f,
                "Range<{}, {}{}>",
                a,
                b,
                if *inclusive { ", inclusive" } else { "" }
            ),
            TypeNode::TypeRef(s) => write!(f, "{}", s),
        }
    }
}

impl fmt::Display for TypeMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expected {}, found {}", self.expected, self.found)
    }
}

impl fmt::Display for NamedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl SemanticError {
    pub fn code(&self) -> &'static str {
        match self {
            // Variable Declaration/Assignment Errors
            SemanticError::VariableRedeclaration(_) => "E0001",
            SemanticError::UndeclaredVariable(_) => "E0002",
            SemanticError::VarTypeMismatch(_) => "E0003",
            SemanticError::TupleAssignmentMismatch { .. } => "E0004",
            SemanticError::InvalidAssignmentTarget { .. } => "E0005",
            SemanticError::OutOfScopeVariable(_) => "E0006",
            SemanticError::InvalidMapKeyType { .. } => "E0007",

            // Function Declaration/Call Errors
            SemanticError::FunctionRedeclaration(_) => "E0101",
            SemanticError::FunctionParamRedeclaration(_) => "E0102",
            SemanticError::MissingParamType(_) => "E0103",
            SemanticError::UndeclaredFunction(_) => "E0104",
            SemanticError::InvalidFunctionCall { .. } => "E0105",
            SemanticError::FunctionArgumentMismatch { .. } => "E0106",
            SemanticError::FunctionArgumentTypeMismatch { .. } => "E0107",
            SemanticError::MissingFunctionReturn { .. } => "E0108",
            SemanticError::InvalidReturnInVoidFunction { .. } => "E0109",
            SemanticError::ReturnTypeMismatch { .. } => "E0110",
            SemanticError::InvalidPublicName(_) => "E0111",

            // Type/Operator Errors
            SemanticError::OperatorTypeMismatch(_) => "E0201",
            SemanticError::EmptyCollectionTypeInferenceError(_) => "E0202",
            SemanticError::InvalidConditionType(_) => "E0203",

            // Print
            SemanticError::InvalidPrintType { .. } => "E0301",
            SemanticError::UnexpectedNode { .. } => "E0302",

            // For
            SemanticError::InvalidForIterableType { .. } => "E0401",
            SemanticError::ArrayIterationWithTuple { .. } => "E0402",
            SemanticError::MapIterationRequiresTuple => "E0403",
            SemanticError::NonIterableType { .. } => "E0404",
            SemanticError::InfiniteLoopWithPattern { .. } => "E0405",
            SemanticError::RangeIterationTypeMismatch { .. } => "E0406",

            // Struct
            SemanticError::StructRedeclaration(_) => "E0501",
            SemanticError::DuplicateField { .. } => "E0502",

            // Enum
            SemanticError::EnumRedeclaration(_) => "E0601",
            SemanticError::DuplicateEnumVariant { .. } => "E0602",

            // Module Import / Parse
            SemanticError::ModuleNotFound(_) => "E0701",
            SemanticError::ParseError => "E0702",

            SemanticError::ParseErrorInModule { .. } => "E0703",
        }
    }
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SemanticError as E;
        match self {
            // Variable Declaration/Assignment Errors
            E::VariableRedeclaration(n) => write!(
                f,
                "error[{}]: variable '{}' redeclared in this scope",
                self.code(),
                n
            ),
            E::UndeclaredVariable(n) => write!(
                f,
                "error[{}]: use of undeclared variable '{}'",
                self.code(),
                n
            ),
            E::VarTypeMismatch(m) => write!(f, "error[{}]: type mismatch: {}", self.code(), m),
            E::TupleAssignmentMismatch { expected, found } => write!(
                f,
                "error[{}]: tuple assignment mismatch: expected {} elements, found {}",
                self.code(),
                expected,
                found
            ),
            E::InvalidAssignmentTarget { target } => write!(
                f,
                "error[{}]: invalid assignment target: {}",
                self.code(),
                target
            ),
            E::OutOfScopeVariable(n) => write!(
                f,
                "error[{}]: variable '{}' is out of scope here",
                self.code(),
                n
            ),
            E::InvalidMapKeyType { found, expected } => write!(
                f,
                "error[{}]: invalid map key type: expected {}, found {}",
                self.code(),
                expected,
                found
            ),

            // Function Declaration/Call Errors
            E::FunctionRedeclaration(n) => {
                write!(f, "error[{}]: function '{}' redeclared", self.code(), n)
            }
            E::FunctionParamRedeclaration(n) => write!(
                f,
                "error[{}]: duplicate parameter name '{}'",
                self.code(),
                n
            ),
            E::MissingParamType(n) => write!(
                f,
                "error[{}]: missing type annotation for parameter '{}'",
                self.code(),
                n
            ),
            E::UndeclaredFunction(n) => write!(
                f,
                "error[{}]: call to undeclared function '{}'",
                self.code(),
                n
            ),
            E::InvalidFunctionCall { func } => write!(
                f,
                "error[{}]: invalid function call target: {}",
                self.code(),
                func
            ),
            E::FunctionArgumentMismatch {
                name,
                expected,
                found,
            } => write!(
                f,
                "error[{}]: function '{}' expects {} arguments, found {}",
                self.code(),
                name,
                expected,
                found
            ),
            E::FunctionArgumentTypeMismatch {
                name,
                expected,
                found,
            } => write!(
                f,
                "error[{}]: function '{}' argument type mismatch: expected {}, found {}",
                self.code(),
                name,
                expected,
                found
            ),
            E::MissingFunctionReturn { function } => write!(
                f,
                "error[{}]: function '{}' must return a value",
                self.code(),
                function
            ),
            E::InvalidReturnInVoidFunction { function } => write!(
                f,
                "error[{}]: function '{}' cannot return a value (declared Void)",
                self.code(),
                function
            ),
            E::ReturnTypeMismatch { function, mismatch } => write!(
                f,
                "error[{}]: return type mismatch in '{}': {}",
                self.code(),
                function,
                mismatch
            ),
            E::InvalidPublicName(n) => write!(
                f,
                "error[{}]: public function '{}' must start with an uppercase letter",
                self.code(),
                n
            ),

            // Type/Operator Errors
            E::OperatorTypeMismatch(m) => {
                write!(f, "error[{}]: operator type mismatch: {}", self.code(), m)
            }
            E::EmptyCollectionTypeInferenceError(m) => write!(
                f,
                "error[{}]: cannot infer type of empty collection: {}",
                self.code(),
                m
            ),
            E::InvalidConditionType(m) => {
                write!(f, "error[{}]: invalid condition type: {}", self.code(), m)
            }

            // Print
            E::InvalidPrintType { found } => write!(
                f,
                "error[{}]: cannot print value of type {}",
                self.code(),
                found
            ),
            E::UnexpectedNode { expected } => write!(
                f,
                "error[{}]: unexpected construct: expected {}",
                self.code(),
                expected
            ),

            // For
            E::InvalidForIterableType { found } => write!(
                f,
                "error[{}]: invalid iterable type in for-loop: {}",
                self.code(),
                found
            ),
            E::ArrayIterationWithTuple { tuple_len } => write!(
                f,
                "error[{}]: array iteration does not support tuple pattern of length {}",
                self.code(),
                tuple_len
            ),
            E::MapIterationRequiresTuple => write!(
                f,
                "error[{}]: map iteration requires a tuple pattern (key, value)",
                self.code()
            ),
            E::NonIterableType { found } => write!(
                f,
                "error[{}]: non-iterable type in for-loop: {}",
                self.code(),
                found
            ),
            E::InfiniteLoopWithPattern { pattern } => write!(
                f,
                "error[{}]: infinite loop with pattern is not allowed: {:?}",
                self.code(),
                pattern
            ),
            E::RangeIterationTypeMismatch { expected, found } => write!(
                f,
                "error[{}]: range iteration type mismatch: expected {}, found {}",
                self.code(),
                expected,
                found
            ),

            // Struct
            E::StructRedeclaration(n) => {
                write!(f, "error[{}]: struct '{}' redeclared", self.code(), n)
            }
            E::DuplicateField { struct_name, field } => write!(
                f,
                "error[{}]: struct '{}' has duplicate field '{}'",
                self.code(),
                struct_name,
                field
            ),

            // Enum
            E::EnumRedeclaration(n) => write!(f, "error[{}]: enum '{}' redeclared", self.code(), n),
            E::DuplicateEnumVariant { enum_name, variant } => write!(
                f,
                "error[{}]: enum '{}' has duplicate variant '{}'",
                self.code(),
                enum_name,
                variant
            ),

            // Module Import / Parse
            E::ModuleNotFound(p) => write!(f, "error[{}]: module not found: {}", self.code(), p),
            E::ParseError => write!(f, "error[{}]: parse error in imported module", self.code()),

            E::ParseErrorInModule { file, error } => {
                write!(f, "error[{}] in {}: {}", self.code(), file, error)
            }
        }
    }
}
