use super::analyzer::SemanticAnalyzer;
use super::types::{SemanticError, TypeMismatch};
use crate::analyzer::types::NamedError;
use crate::parser::ast::{AstNode, Pattern, TypeNode};

impl SemanticAnalyzer {
    pub fn analyze_conditional_stmt(
        &mut self,
        condition: &mut AstNode,
        then_block: &mut Vec<AstNode>,
        else_branch: &mut Option<Box<AstNode>>,
    ) -> Result<(), SemanticError> {
        // Expect bool in ConditionalStmt always
        let cond_type = self.infer_type(condition)?;
        if cond_type != TypeNode::Bool {
            return Err(SemanticError::InvalidConditionType(TypeMismatch {
                expected: TypeNode::Bool,
                found: cond_type,
                value: None,
            }));
        }

        self.analyze_program(then_block)?;

        if let Some(else_node) = else_branch {
            self.analyze_node(else_node)?;
        }

        Ok(())
    }

    pub fn analyze_print(&mut self, node: &mut AstNode) -> Result<(), SemanticError> {
        if let AstNode::Print { exprs } = node {
            for expr in exprs.iter_mut() {
                // Infer type for each expression
                let ty = self.infer_type(expr)?;
                // Optionally: You can restrict print to only supported types
                match ty {
                    TypeNode::Int
                    | TypeNode::Float
                    | TypeNode::Bool
                    | TypeNode::String
                    | TypeNode::Array(_)
                    | TypeNode::Map(_, _)
                    | TypeNode::Tuple(_) => {
                        // OK to print
                    }
                    _ => {
                        return Err(SemanticError::InvalidPrintType { found: ty });
                    }
                }

                // For expressions that are variables, arrays, maps, or nested,
                // recursively analyze them if needed
                self.analyze_node(expr)?;
            }
            Ok(())
        } else {
            // Not a print node, shouldn't reach here
            Err(SemanticError::UnexpectedNode {
                expected: "print".to_string(),
            })
        }
    }

    pub fn analyze_for_stmt(
        &mut self,
        pattern: &mut Pattern,
        iterable: Option<&mut AstNode>,
        body: &mut Vec<AstNode>,
    ) -> Result<(), SemanticError> {
        let outer_table = self.symbol_table.clone();
        let mut loop_scope = outer_table.clone();
        std::mem::swap(&mut self.symbol_table, &mut loop_scope);

        if let Some(iter_node) = iterable {
            let iter_type = self.infer_type(iter_node)?;

            match iter_type {
                TypeNode::Array(elem_type) => {
                    // Arrays: tuple pattern must have 1 element
                    if let Pattern::Tuple(patterns) = pattern {
                        if patterns.len() != 1 {
                            return Err(SemanticError::InvalidAssignmentTarget {
                                target: "Cannot use tuple pattern when iterating an array"
                                    .to_string(),
                            });
                        }
                        self.bind_pattern_to_type(&mut patterns[0], &*elem_type)?;
                    } else {
                        self.bind_pattern_to_type(pattern, &*elem_type)?;
                    }
                }
                TypeNode::Map(key_type, value_type) => match pattern {
                    // Maps: tuple pattern must have 2 elements (key, value)
                    Pattern::Tuple(patterns) => {
                        if patterns.len() != 2 {
                            return Err(SemanticError::TupleAssignmentMismatch {
                                expected: 2,
                                found: patterns.len(),
                            });
                        }
                        self.bind_pattern_to_type(&mut patterns[0], &*key_type)?;
                        self.bind_pattern_to_type(&mut patterns[1], &*value_type)?;
                    }
                    _ => {
                        return Err(SemanticError::InvalidAssignmentTarget {
                            target: "Expected tuple pattern (key, value) when iterating a map"
                                .to_string(),
                        })
                    }
                },
                TypeNode::Range(start_type, _end_type, _inclusive) => {
                    // Range: only identifier or wildcard allowed
                    if let Pattern::Identifier(_) | Pattern::Wildcard = pattern {
                        self.bind_pattern_to_type(pattern, &*start_type)?;
                    } else {
                        return Err(SemanticError::InvalidAssignmentTarget {
                            target: "Loop variable pattern does not match range type".to_string(),
                        });
                    }
                }
                _ => {
                    return Err(SemanticError::InvalidAssignmentTarget {
                        target: "Cannot iterate non-iterable type".to_string(),
                    })
                }
            }
        } else {
            // Infinite loop: only `_` allowed
            match pattern {
                Pattern::Wildcard => {}
                _ => {
                    return Err(SemanticError::InvalidAssignmentTarget {
                        target: "Cannot use a loop variable pattern without an iterable"
                            .to_string(),
                    });
                }
            }
        }

        self.analyze_program(body)?;
        self.symbol_table = outer_table;
        Ok(())
    }

    fn bind_pattern_to_type(
        &mut self,
        pattern: &mut Pattern,
        ty: &TypeNode,
    ) -> Result<(), SemanticError> {
        match pattern {
            Pattern::Identifier(name) => {
                if self.symbol_table.contains_key(name) {
                    return Err(SemanticError::VariableRedeclaration(NamedError {
                        name: name.clone(),
                    }));
                }
                self.symbol_table.insert(name.clone(), (ty.clone(), false));
            }
            Pattern::Wildcard => {} // `_` ignores type
            Pattern::Tuple(patterns) => match ty {
                TypeNode::Tuple(types) if types.len() == patterns.len() => {
                    for (p, t) in patterns.iter_mut().zip(types.iter()) {
                        self.bind_pattern_to_type(p, t)?;
                    }
                }
                TypeNode::Array(_) | TypeNode::Map(_, _) | TypeNode::Range(_, _, _) => {
                    return Err(SemanticError::TupleAssignmentMismatch {
                        expected: 1,
                        found: patterns.len(),
                    });
                }
                _ => {
                    return Err(SemanticError::InvalidAssignmentTarget {
                        target: format!("{:?}", pattern),
                    });
                }
            },
            _ => {
                return Err(SemanticError::InvalidAssignmentTarget {
                    target: format!("{:?}", pattern),
                });
            }
        }
        Ok(())
    }
}
