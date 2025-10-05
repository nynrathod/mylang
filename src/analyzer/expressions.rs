use super::analyzer::SemanticAnalyzer;
use super::types::{NamedError, SemanticError, TypeMismatch};
use crate::lexar::token::TokenType;
use crate::parser::ast::{AstNode, TypeNode};

impl SemanticAnalyzer {
    pub fn infer_type(&self, node: &AstNode) -> Result<TypeNode, SemanticError> {
        match node {
            AstNode::NumberLiteral(_) => Ok(TypeNode::Int),
            AstNode::StringLiteral(_) => Ok(TypeNode::String),
            AstNode::BoolLiteral(_) => Ok(TypeNode::Bool),

            AstNode::Identifier(name) => {
                if let Some(info) = self.symbol_table.get(name) {
                    Ok(info.ty.clone())
                } else if let Some(outer) = &self.outer_symbol_table {
                    // If variable defined out of function
                    if outer.contains_key(name) {
                        return Err(SemanticError::OutOfScopeVariable(NamedError {
                            name: name.clone(),
                        }));
                    }
                    // If not found variable declaration
                    else {
                        return Err(SemanticError::UndeclaredVariable(NamedError {
                            name: name.clone(),
                        }));
                    }
                } else {
                    Err(SemanticError::UndeclaredVariable(NamedError {
                        name: name.clone(),
                    }))
                }
            }

            AstNode::BinaryExpr { left, op, right } => {
                let left_type = self.infer_type(left)?;
                let right_type = self.infer_type(right)?;

                match op {
                    // Comparison operators
                    TokenType::EqEq
                    | TokenType::EqEqEq
                    | TokenType::NotEq
                    | TokenType::NotEqEq
                    | TokenType::Gt
                    | TokenType::Lt
                    | TokenType::GtEq
                    | TokenType::LtEq => {
                        if left_type != right_type {
                            return Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: left_type,
                                found: right_type,
                                value: None,
                            }));
                        }
                        Ok(TypeNode::Bool)
                    }

                    // Ranges for loops
                    // TokenType::RangeExc = exclusive range (e.g., 0..5)
                    // TokenType::RangeInc = inclusive range (e.g., 0..=5)
                    TokenType::RangeExc | TokenType::RangeInc => {
                        // Both start (left) and end (right) of the range must be integers
                        if left_type != TypeNode::Int || right_type != TypeNode::Int {
                            return Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: TypeNode::Int, // expected type is Int
                                found: if left_type != TypeNode::Int {
                                    left_type
                                } else {
                                    right_type
                                },
                                value: None,
                            }));
                        }

                        // Determine if the range is inclusive (..=) or exclusive (..)
                        let inclusive = matches!(op, TokenType::RangeInc);

                        // Return the type of the range: Range<Int, Int, inclusive>
                        Ok(TypeNode::Range(
                            Box::new(TypeNode::Int), // start type
                            Box::new(TypeNode::Int), // end type
                            inclusive,               // inclusive/exclusive
                        ))
                    }

                    // Logical gates
                    TokenType::AndAnd | TokenType::OrOr => {
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

                    // Arithmetic operators
                    TokenType::Plus
                    | TokenType::Minus
                    | TokenType::Star
                    | TokenType::Slash
                    | TokenType::Percent => match (left_type.clone(), right_type.clone()) {
                        (TypeNode::Int, TypeNode::Int) => Ok(TypeNode::Int),
                        // allow string concatenation
                        (TypeNode::String, TypeNode::String) => Ok(TypeNode::String),
                        // Float is not supported for now
                        (TypeNode::Float, TypeNode::Float) => Ok(TypeNode::Float),
                        _ => Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                            expected: left_type,
                            found: right_type,
                            value: None,
                        })),
                    },

                    _ => unimplemented!("Operator {:?} not handled", op),
                }
            }

            AstNode::UnaryExpr { expr, .. } => self.infer_type(expr),

            AstNode::FunctionCall { func, args: _ } => {
                let name = if let AstNode::Identifier(n) = &**func {
                    n
                } else {
                    return Err(SemanticError::InvalidFunctionCall {
                        func: format!("{:?}", func),
                    });
                };
                if let Some((_param_types, ret_ty)) = self.function_table.get(name) {
                    Ok(ret_ty.clone())
                } else {
                    Err(SemanticError::UndeclaredFunction(NamedError {
                        name: name.clone(),
                    }))
                }
            }

            AstNode::ArrayLiteral(elements) => {
                // Error if Array is empty, can't infer types
                if elements.is_empty() {
                    return Err(SemanticError::EmptyCollectionTypeInferenceError(
                        TypeMismatch {
                            expected: TypeNode::Array(Box::new(TypeNode::Int)),
                            found: TypeNode::Array(Box::new(TypeNode::Void)),
                            value: None,
                        },
                    ));
                }

                // Use first element to infer array type
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
                Ok(TypeNode::Array(Box::new(first_type)))
            }

            AstNode::MapLiteral(pairs) => {
                // Error if map is empty, can't infer types
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

                Ok(TypeNode::Map(Box::new(key_type), Box::new(value_type)))
            }

            _ => {
                // For statements, return Void; actual checking happens in analyze_node
                Ok(TypeNode::Void)
            }
        }
    }
}
