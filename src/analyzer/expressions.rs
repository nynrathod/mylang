use super::analyzer::SemanticAnalyzer;
use super::types::{NamedError, SemanticError, TypeMismatch};
use crate::lexar::token::TokenType;
use crate::parser::ast::{AstNode, TypeNode};

/// Helper to extract line/col from an AstNode
/// For now, returns None since parser hasn't been updated yet
fn get_node_location(_node: &AstNode) -> (Option<usize>, Option<usize>) {
    // TODO: Once parser is updated to include line/col in AST nodes,
    // implement proper extraction here
    (None, None)
}

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
            // Float literal: always Float type
            AstNode::FloatLiteral(_) => Ok(TypeNode::Float),
            // String literal: always String type
            AstNode::StringLiteral(s) => {
                // Reject string interpolation syntax ${...}
                if s.contains("${") {
                    return Err(SemanticError::UndeclaredFunction(NamedError {
                        name: "String interpolation with ${...} is not supported".to_string(),
                    }));
                }
                Ok(TypeNode::String)
            }
            // Boolean literal: always Bool type
            AstNode::BoolLiteral(_) => Ok(TypeNode::Bool),

            // Identifier (variable name): look up in symbol table (with shadowing support)
            AstNode::Identifier(name) => {
                if let Some(info) = self.lookup_variable(name) {
                    Ok(info.ty.clone())
                } else if let Some(outer) = &self.outer_symbol_table {
                    if outer.contains_key(name) {
                        return Err(SemanticError::OutOfScopeVariable(NamedError {
                            name: name.clone(),
                        }));
                    }
                    Err(SemanticError::UndeclaredVariable(NamedError {
                        name: name.clone(),
                    }))
                } else {
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
                            let (line, col) = get_node_location(node);
                            return Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: left_type,
                                found: right_type,
                                value: None,
                                line,
                                col,
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
                            let (line, col) = get_node_location(node);
                            return Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: TypeNode::Int,
                                found: if left_type != TypeNode::Int {
                                    left_type
                                } else {
                                    right_type
                                },
                                value: None,
                                line,
                                col,
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
                            let (line, col) = get_node_location(node);
                            return Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: TypeNode::Bool,
                                found: if left_type != TypeNode::Bool {
                                    left_type
                                } else {
                                    right_type
                                },
                                value: None,
                                line,
                                col,
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
                        _ => {
                            let (line, col) = get_node_location(node);
                            Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: left_type,
                                found: right_type,
                                value: None,
                                line,
                                col,
                            }))
                        }
                    },

                    // Any other operator is not implemented
                    _ => unimplemented!("Operator {:?} not handled", op),
                }
            }

            // Unary expressions (e.g., -x, !x): infer type of the inner expression
            // Ex., let neg = -x;
            // Ex., let not = !flag;
            // TODO: check llvm handled for this or not
            AstNode::UnaryExpr { op, expr } => {
                let expr_type = self.infer_type(expr)?;
                match op {
                    TokenType::Minus => match expr_type {
                        TypeNode::Int | TypeNode::Float => Ok(expr_type),
                        _ => {
                            let (line, col) = get_node_location(expr);
                            Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: TypeNode::Int,
                                found: expr_type,
                                value: None,
                                line,
                                col,
                            }))
                        }
                    },
                    TokenType::Bang => {
                        if expr_type == TypeNode::Bool {
                            Ok(TypeNode::Bool)
                        } else {
                            let (line, col) = get_node_location(expr);
                            Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: TypeNode::Bool,
                                found: expr_type,
                                value: None,
                                line,
                                col,
                            }))
                        }
                    }
                    _ => Err(SemanticError::UnexpectedNode {
                        expected: "Minus or Bang operator".to_string(),
                    }),
                }
            }

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
                    // Allow empty array: infer type from annotation if present, otherwise default to Array<Int>
                    // If you want to support type annotation, you can pass it in or check node context.
                    // For now, default to Array<Int>
                    return Ok(TypeNode::Array(Box::new(TypeNode::Int)));
                }

                // Infer type from first element
                // This check type of element insides
                let first_type = self.infer_type(&elements[0])?;
                // Check all elements for type consistency
                for el in elements.iter() {
                    let t = self.infer_type(el)?;
                    if t != first_type {
                        let (line, col) = get_node_location(el);
                        return Err(SemanticError::VarTypeMismatch(TypeMismatch {
                            expected: first_type.clone(),
                            found: t,
                            value: None,
                            line,
                            col,
                        }));
                    }
                }
                // All elements are the same type: return Array of that type
                Ok(TypeNode::Array(Box::new(first_type)))
            }

            // Map literal: infer type of keys and values
            AstNode::MapLiteral(pairs) => {
                // Allow empty map: infer type from annotation if present, otherwise default to Map<String, Int>
                if pairs.is_empty() {
                    // If you want to support type annotation, you can pass it in or check node context.
                    // For now, default to Map<String, Int>
                    return Ok(TypeNode::Map(
                        Box::new(TypeNode::String),
                        Box::new(TypeNode::Int),
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
                        let (line, col) = get_node_location(k);
                        return Err(SemanticError::VarTypeMismatch(TypeMismatch {
                            expected: key_type.clone(),
                            found: kt,
                            value: None,
                            line,
                            col,
                        }));
                    }
                    if vt != value_type {
                        let (line, col) = get_node_location(v);
                        return Err(SemanticError::VarTypeMismatch(TypeMismatch {
                            expected: value_type.clone(),
                            found: vt,
                            value: None,
                            line,
                            col,
                        }));
                    }
                }

                // All keys and values are consistent: return Map type
                Ok(TypeNode::Map(Box::new(key_type), Box::new(value_type)))
            }

            // Element access: arr[index] or map[key]
            // Infer type of the array/map and the index/key
            AstNode::ElementAccess { array, index } => {
                let array_type = self.infer_type(array)?;
                let index_type = self.infer_type(index)?;

                // Reject negative indices for arrays
                if let AstNode::UnaryExpr {
                    op: TokenType::Minus,
                    expr: _,
                } = &**index
                {
                    return Err(SemanticError::InvalidAssignmentTarget {
                        target: "Array indices cannot be negative".to_string(),
                    });
                }

                match array_type {
                    // Array element access: arr[Int] -> T
                    TypeNode::Array(element_type) => {
                        // Index must be an Int
                        if index_type != TypeNode::Int {
                            let (line, col) = get_node_location(index);
                            return Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: TypeNode::Int,
                                found: index_type,
                                value: None,
                                line,
                                col,
                            }));
                        }
                        // Return the element type
                        Ok(*element_type)
                    }
                    // Map element access: map[Key] -> Value
                    TypeNode::Map(key_type, value_type) => {
                        // Index must match the key type
                        if index_type != *key_type {
                            let (line, col) = get_node_location(index);
                            return Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                                expected: *key_type,
                                found: index_type,
                                value: None,
                                line,
                                col,
                            }));
                        }
                        // Return the value type
                        Ok(*value_type)
                    }
                    // Element access on non-indexable type
                    _ => {
                        let (line, col) = get_node_location(array);
                        Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                            expected: TypeNode::Array(Box::new(TypeNode::Int)),
                            found: array_type,
                            value: None,
                            line,
                            col,
                        }))
                    }
                }
            }

            // Any other AST node (usually statements): return Void type.
            // Actual semantic checking for statements happens elsewhere.
            _ => Ok(TypeNode::Void),
        }
    }
}
