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
                if let Some((t, _)) = self.symbol_table.get(name) {
                    Ok(t.clone())
                } else if let Some(outer) = &self.outer_symbol_table {
                    // If variable defined out of function
                    if let Some((_t, _)) = outer.get(name) {
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
                            }));
                        }
                        Ok(TypeNode::Bool)
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
                        (TypeNode::Float, TypeNode::Float) => Ok(TypeNode::Float),
                        _ => {
                            return Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: left_type,
                                found: right_type,
                            }));
                        }
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
                if elements.is_empty() {
                    return Err(SemanticError::EmptyCollectionTypeInferenceError(
                        TypeMismatch {
                            expected: TypeNode::Array(Box::new(TypeNode::Int)),
                            found: TypeNode::Array(Box::new(TypeNode::Int)),
                        },
                    ));
                }

                // Use the first element to infer array types,
                // mixing [1, "2", "hello"]; not supported
                let first_type = self.infer_type(&elements[0])?;
                Ok(TypeNode::Array(Box::new(first_type)))
            }

            AstNode::MapLiteral(pairs) => {
                if pairs.is_empty() {
                    return Err(SemanticError::EmptyCollectionTypeInferenceError(
                        TypeMismatch {
                            expected: TypeNode::Map(
                                Box::new(TypeNode::String),
                                Box::new(TypeNode::Int),
                            ),
                            found: TypeNode::Map(
                                Box::new(TypeNode::String),
                                Box::new(TypeNode::Int),
                            ),
                        },
                    ));
                }
                // Use the first key-value pair to infer map types
                // mixing (e.g., { "a": 1, 2: "b" }) is NOT supported
                let key_type = self.infer_type(&pairs[0].0)?;
                let value_type = self.infer_type(&pairs[0].1)?;
                Ok(TypeNode::Map(Box::new(key_type), Box::new(value_type)))
            }

            _ => {
                // For statements, return Void; actual checking happens in analyze_node
                Ok(TypeNode::Void)
            }
        }
    }
}
