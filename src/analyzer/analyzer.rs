use crate::analyzer::types::{SemanticError, TypeMismatch};
use crate::lexar::token::TokenType;
use crate::parser::ast::{AstNode, TypeNode};
use std::collections::HashMap;

pub struct SemanticAnalyzer {
    pub(crate) symbol_table: HashMap<String, (TypeNode, bool)>, // current scope variables
    pub(crate) function_table: HashMap<String, TypeNode>,
    pub(crate) outer_symbol_table: Option<HashMap<String, (TypeNode, bool)>>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            symbol_table: HashMap::new(),
            function_table: HashMap::new(),
            outer_symbol_table: None,
        }
    }

    /// Analyze a list of AST nodes (program and block)
    pub fn analyze_program(&mut self, nodes: &mut Vec<AstNode>) -> Result<(), SemanticError> {
        for node in nodes {
            self.analyze_node(node)?;
        }
        Ok(())
    }

    /// Check if any node contains a return statement
    pub fn has_return_statement(&self, nodes: &Vec<AstNode>) -> bool {
        for node in nodes {
            match node {
                AstNode::Return { .. } => return true,
                AstNode::ConditionalStmt {
                    then_block,
                    else_branch,
                    ..
                } => {
                    let then_has = self.has_return_statement(then_block);
                    let else_has = else_branch
                        .as_ref()
                        .map(|b| self.has_return_statement(&vec![*b.clone()]))
                        .unwrap_or(false);
                    if then_has && else_has {
                        return true;
                    }
                }
                AstNode::Block(inner_nodes) => {
                    if self.has_return_statement(inner_nodes) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// Dispatch analysis based on AST node type
    pub fn analyze_node(&mut self, node: &mut AstNode) -> Result<(), SemanticError> {
        match node {
            AstNode::LetDecl { .. } => self.analyze_let_decl(node),
            AstNode::Block(nodes) => self.analyze_program(nodes),
            AstNode::ConditionalStmt {
                condition,
                then_block,
                else_branch,
            } => self.analyze_conditional_stmt(condition, then_block, else_branch),
            AstNode::FunctionDecl {
                name,
                visibility,
                params,
                return_type,
                body,
            } => self.analyze_functional_decl(name, visibility, params, return_type, body),
            AstNode::Return { values } => {
                for v in values {
                    self.infer_type(v)?; // check return value types
                }
                Ok(())
            }
            AstNode::Print { exprs } => {
                for expr in exprs {
                    self.infer_type(expr)?;
                }
                Ok(())
            }
            AstNode::Break | AstNode::Continue => Ok(()),

            // Expressions used as statements
            _ => {
                // Catch-all for any AST nodes not explicitly handled above.
                // We call `infer_type` to:
                // Validate that all identifiers exist in scope.
                // Ensure expressions (literals, binary/unary ops, function calls) are type-correct.
                // Future-proof: new AST node types will still be semantically validated.
                self.infer_type(node)?;
                Ok(())
            }
        }
    }

    /// Infer the type of a given AST node
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
                        return Err(SemanticError::OutOfScopeVariable { name: name.clone() });
                    }
                    // If not found variable declaration
                    else {
                        return Err(SemanticError::UndeclaredVariable { name: name.clone() });
                    }
                } else {
                    Err(SemanticError::UndeclaredVariable { name: name.clone() })
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

                    _ => unimplemented!("Operator {:?} not handled", op),
                }
            }

            AstNode::UnaryExpr { expr, .. } => self.infer_type(expr),

            AstNode::FunctionCall { func, args } => {
                let name = if let AstNode::Identifier(name) = &**func {
                    name
                } else {
                    return Err(SemanticError::UndeclaredVariable {
                        name: format!("{:?}", func),
                    });
                };
                if let Some(ret_ty) = self.function_table.get(name) {
                    Ok(ret_ty.clone())
                } else {
                    Err(SemanticError::UndeclaredVariable { name: name.clone() })
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
