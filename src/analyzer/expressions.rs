use super::analyzer::SemanticAnalyzer;
use super::types::{NamedError, SemanticError, TypeMismatch};
use crate::lexar::token::TokenType;
use crate::parser::ast::{AstNode, TypeNode};

impl SemanticAnalyzer {
    /// Infers the type of an AST node (expression).
    /// This is the core type inference function for all expressions in the language.
    /// - Returns the type of literals directly.
    /// - Looks up identifiers in the symbol table.
    /// - Checks types for binary/unary expressions, function calls, arrays, maps, etc.
    /// - Returns errors for undeclared variables, type mismatches, or invalid operations.
    pub fn infer_type(&self, node: &AstNode) -> Result<TypeNode, SemanticError> {
        match node {
            // Integer literal: always Int type
            AstNode::NumberLiteral(_) => Ok(TypeNode::Int),
            // String literal: always String type
            AstNode::StringLiteral(_) => Ok(TypeNode::String),
            // Boolean literal: always Bool type
            AstNode::BoolLiteral(_) => Ok(TypeNode::Bool),

            // Identifier (variable name): look up in symbol table
            AstNode::Identifier(name) => {
                if let Some(info) = self.symbol_table.get(name) {
                    // Found in current scope then return its type
                    Ok(info.ty.clone())
                } else if let Some(outer) = &self.outer_symbol_table {
                    // If variable is defined in an outer scope but not accessible here
                    if outer.contains_key(name) {
                        return Err(SemanticError::OutOfScopeVariable(NamedError {
                            name: name.clone(),
                        }));
                    }
                    // If not found in any scope, variable is undeclared
                    else {
                        return Err(SemanticError::UndeclaredVariable(NamedError {
                            name: name.clone(),
                        }));
                    }
                } else {
                    // No outer scope, variable is undeclared
                    Err(SemanticError::UndeclaredVariable(NamedError {
                        name: name.clone(),
                    }))
                }
            }

            // Binary expressions (e.g., arithmetic, comparison, logical, range)
            // Ex., let is_equal = x == y;
            // TODO: check llvm handled for this or not
            AstNode::BinaryExpr { left, op, right } => {
                // Infer types of both sides
                let left_type = self.infer_type(left)?;
                let right_type = self.infer_type(right)?;

                match op {
                    // Comparison operators (==, !=, >, <, etc.)
                    TokenType::EqEq
                    | TokenType::EqEqEq
                    | TokenType::NotEq
                    | TokenType::NotEqEq
                    | TokenType::Gt
                    | TokenType::Lt
                    | TokenType::GtEq
                    | TokenType::LtEq => {
                        // Both sides must be the same type
                        if left_type != right_type {
                            return Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: left_type,
                                found: right_type,
                                value: None,
                            }));
                        }
                        // Comparison always returns Bool
                        Ok(TypeNode::Bool)
                    }

                    // Range operators for loops (.. and ..=)
                    // Ex., for i in 0..10 {
                    // TODO: check llvm handled for this or not
                    TokenType::RangeExc | TokenType::RangeInc => {
                        // Both start and end must be Int
                        if left_type != TypeNode::Int || right_type != TypeNode::Int {
                            return Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: TypeNode::Int,
                                found: if left_type != TypeNode::Int {
                                    left_type
                                } else {
                                    right_type
                                },
                                value: None,
                            }));
                        }
                        // Determine if range is inclusive or exclusive
                        let inclusive = matches!(op, TokenType::RangeInc);
                        // Return Range type
                        Ok(TypeNode::Range(
                            Box::new(TypeNode::Int),
                            Box::new(TypeNode::Int),
                            inclusive,
                        ))
                    }

                    // Logical operators (&&, ||)
                    // Ex., let a = true;
                    // let b = a && c;
                    // TODO: check llvm handled for this or not
                    TokenType::AndAnd | TokenType::OrOr => {
                        // Both sides must be Bool
                        if left_type != TypeNode::Bool || right_type != TypeNode::Bool {
                            return Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: TypeNode::Bool,
                                found: if left_type != TypeNode::Bool {
                                    left_type
                                } else {
                                    right_type
                                },
                                value: None,
                            }));
                        }
                        Ok(TypeNode::Bool)
                    }

                    // Arithmetic operators (+, -, *, /, %)
                    // Ex., let a = "hello" + "world";
                    // Ex., let b = 1 + 2;
                    // TODO: check llvm handled for this or not
                    TokenType::Plus
                    | TokenType::Minus
                    | TokenType::Star
                    | TokenType::Slash
                    | TokenType::Percent => match (left_type.clone(), right_type.clone()) {
                        // both lhs and rhs should match type
                        (TypeNode::Int, TypeNode::Int) => Ok(TypeNode::Int),
                        // String concatenation
                        (TypeNode::String, TypeNode::String) => Ok(TypeNode::String),
                        // Float arithmetic (if supported)
                        (TypeNode::Float, TypeNode::Float) => Ok(TypeNode::Float),
                        // Any other type combination is invalid
                        _ => Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                            expected: left_type,
                            found: right_type,
                            value: None,
                        })),
                    },

                    // Any other operator is not implemented
                    _ => unimplemented!("Operator {:?} not handled", op),
                }
            }

            // Unary expressions (e.g., -x, !x): infer type of the inner expression
            // Ex., let neg = -x;
            // Ex., let not = !flag;
            // TODO: check llvm handled for this or not
            AstNode::UnaryExpr { expr, .. } => self.infer_type(expr),

            // Function call: infer return type from function signature
            // Ex., let result = myFunction(1, "abc");
            AstNode::FunctionCall { func, args: _ } => {
                // Function must be an identifier
                // - Allowed: `myFunction(1, 2)`
                // - Not allowed: `(some_expr)(1, 2)` or `foo.bar(1, 2)`
                let name = if let AstNode::Identifier(n) = &**func {
                    n
                } else {
                    return Err(SemanticError::InvalidFunctionCall {
                        func: format!("{:?}", func),
                    });
                };
                // Look up function in function table
                if let Some((_param_types, ret_ty)) = self.function_table.get(name) {
                    Ok(ret_ty.clone())
                } else {
                    // Function not found
                    Err(SemanticError::UndeclaredFunction(NamedError {
                        name: name.clone(),
                    }))
                }
            }

            // Array literal: infer type of elements
            AstNode::ArrayLiteral(elements) => {
                // Error if array is empty: cannot infer type
                // let empty = [];
                if elements.is_empty() {
                    return Err(SemanticError::EmptyCollectionTypeInferenceError(
                        TypeMismatch {
                            expected: TypeNode::Array(Box::new(TypeNode::Int)),
                            found: TypeNode::Array(Box::new(TypeNode::Void)),
                            value: None,
                        },
                    ));
                }

                // Infer type from first element
                // This check type of element insides
                let first_type = self.infer_type(&elements[0])?;
                // Check all elements for type consistency
                for el in elements.iter() {
                    let t = self.infer_type(el)?;
                    if t != first_type {
                        return Err(SemanticError::VarTypeMismatch(TypeMismatch {
                            expected: first_type.clone(),
                            found: t,
                            value: None,
                        }));
                    }
                }
                // All elements are the same type: return Array of that type
                Ok(TypeNode::Array(Box::new(first_type)))
            }

            // Map literal: infer type of keys and values
            AstNode::MapLiteral(pairs) => {
                // Error if map is empty: cannot infer type
                if pairs.is_empty() {
                    return Err(SemanticError::EmptyCollectionTypeInferenceError(
                        TypeMismatch {
                            expected: TypeNode::Map(
                                Box::new(TypeNode::String),
                                Box::new(TypeNode::Int),
                            ),
                            found: TypeNode::Map(
                                Box::new(TypeNode::Void),
                                Box::new(TypeNode::Void),
                            ),
                            value: None,
                        },
                    ));
                }

                // Infer key and value types from first pair
                let key_type = self.infer_type(&pairs[0].0)?;
                let value_type = self.infer_type(&pairs[0].1)?;

                // Only allow Int, String, or Bool as map keys
                // TODO: check codegen if implemented or not
                match key_type {
                    TypeNode::Int | TypeNode::String | TypeNode::Bool => {}
                    _ => {
                        return Err(SemanticError::InvalidMapKeyType {
                            found: key_type.clone(),
                            expected: TypeNode::Map(
                                Box::new(TypeNode::Int),
                                Box::new(TypeNode::String),
                            ),
                        });
                    }
                }

                // Check all pairs for type consistency
                for (k, v) in pairs.iter() {
                    let kt = self.infer_type(k)?;
                    let vt = self.infer_type(v)?;
                    if kt != key_type {
                        return Err(SemanticError::VarTypeMismatch(TypeMismatch {
                            expected: key_type.clone(),
                            found: kt,
                            value: None,
                        }));
                    }
                    if vt != value_type {
                        return Err(SemanticError::VarTypeMismatch(TypeMismatch {
                            expected: value_type.clone(),
                            found: vt,
                            value: None,
                        }));
                    }
                }

                // All keys and values are consistent: return Map type
                Ok(TypeNode::Map(Box::new(key_type), Box::new(value_type)))
            }

            // Any other AST node (usually statements): return Void type.
            // Actual semantic checking for statements happens elsewhere.
            _ => Ok(TypeNode::Void),
        }
    }
}
