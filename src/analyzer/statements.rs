use super::analyzer::SemanticAnalyzer;
use std::collections::HashMap;

use super::types::{SemanticError, TypeMismatch};
use crate::parser::ast::{AstNode, TypeNode};

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
            }));
        }

        self.analyze_program(then_block)?;

        if let Some(else_node) = else_branch {
            self.analyze_node(else_node)?;
        }

        Ok(())
    }

    pub fn analyze_functional_decl(
        &mut self,
        name: &str,
        visibility: &str,
        params: &mut Vec<(String, Option<TypeNode>)>,
        return_type: &mut Option<TypeNode>,
        body: &mut Vec<AstNode>,
    ) -> Result<(), SemanticError> {
        // If function already defined
        if self.function_table.contains_key(name) {
            return Err(SemanticError::FunctionRedeclaration {
                name: name.to_string(),
            });
        }

        self.function_table.insert(
            name.to_string(),
            return_type.clone().unwrap_or(TypeNode::Void),
        );

        // Is public or private function
        if visibility == "Public" {
            if let Some(first_char) = name.chars().next() {
                if !first_char.is_uppercase() {
                    return Err(SemanticError::InvalidPublicName {
                        name: name.to_string(),
                    });
                }
            }
        }

        // Create a local scope for function parameters
        let mut local_scope: HashMap<String, (TypeNode, bool)> = HashMap::new();

        for (param_name, param_type) in params.iter() {
            // Type is mandator if parameter passed. Check type exists
            let param_type =
                param_type
                    .as_ref()
                    .ok_or_else(|| SemanticError::MissingParamType {
                        name: param_name.clone(),
                    })?;

            // Check duplicate param names
            if local_scope.contains_key(param_name) {
                return Err(SemanticError::FunctionParamRedeclaration {
                    name: param_name.clone(),
                });
            }

            // Insert parameter into local scope (immutable)
            local_scope.insert(param_name.clone(), (param_type.clone(), false));
        }

        // mark as Void if no return type
        if return_type.is_none() {
            *return_type = Some(TypeNode::Void);

            // Ensure no return values are present
            // If no return type specified, function can't return except Void;
            for node in body.iter() {
                if let AstNode::Return { values } = node {
                    if !values.is_empty() {
                        return Err(SemanticError::InvalidReturnInVoidFunction {
                            function: name.to_string(),
                        });
                    }
                }
            }

            // Append implicit empty return if last statement is not Return
            if let Some(last) = body.last() {
                if !matches!(last, AstNode::Return { .. }) {
                    body.push(AstNode::Return { values: vec![] });
                }
            }
        }

        let outer_symbol_table = Some(self.symbol_table.clone());
        self.outer_symbol_table = outer_symbol_table;
        self.symbol_table = local_scope; // only params visible

        // Check if function has return statement and if it matches the return type
        if let Some(ret_type) = return_type.as_ref() {
            if *ret_type != TypeNode::Void {
                self.ensure_has_return(body, name)?;
                self.verify_return_types(body, ret_type, name)?;
            }
        }

        // Analyze function body with **isolated scope**
        self.analyze_program(body)?;

        // Restore outer scope
        self.symbol_table = self.outer_symbol_table.take().unwrap(); // restore

        // println!(
        //     "{} {:?} {:?} {:?} {:?}",
        //     name, visibility, params, return_type, body
        // );

        Ok(())
    }

    /// Ensure function has at least one return statement
    fn ensure_has_return(&self, body: &Vec<AstNode>, fn_name: &str) -> Result<(), SemanticError> {
        if !self.has_return_statement(body) {
            return Err(SemanticError::MissingFunctionReturn {
                function: fn_name.to_string(),
            });
        }
        Ok(())
    }

    /// Verify each return statement matches the expected return type
    fn verify_return_types(
        &self,
        nodes: &Vec<AstNode>,
        expected: &TypeNode,
        fn_name: &str,
    ) -> Result<(), SemanticError> {
        for node in nodes {
            match node {
                AstNode::Return { values } => {
                    self.verify_single_return(values, expected, fn_name)?;
                }
                AstNode::ConditionalStmt {
                    then_block,
                    else_branch,
                    ..
                } => {
                    self.verify_return_types(then_block, expected, fn_name)?;
                    if let Some(else_node) = else_branch {
                        match &**else_node {
                            AstNode::Block(nodes) => {
                                self.verify_return_types(nodes, expected, fn_name)?
                            }
                            _ => self.verify_return_types(
                                &vec![*else_node.clone()],
                                expected,
                                fn_name,
                            )?,
                        }
                    }
                }
                AstNode::Block(inner_nodes) => {
                    self.verify_return_types(inner_nodes, expected, fn_name)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Verify a single return statement matches the expected type
    fn verify_single_return(
        &self,
        values: &Vec<AstNode>,
        expected: &TypeNode,
        fn_name: &str,
    ) -> Result<(), SemanticError> {
        match expected {
            TypeNode::Tuple(expected_vec) => {
                if values.len() != expected_vec.len() {
                    return Err(SemanticError::ReturnTypeMismatch {
                        function: fn_name.to_string(),
                        mismatch: TypeMismatch {
                            expected: expected.clone(),
                            found: TypeNode::Tuple(
                                values
                                    .iter()
                                    .map(|v| self.infer_type(v))
                                    .collect::<Result<Vec<_>, _>>()?,
                            ),
                        },
                    });
                }
                for (value, expected_type) in values.iter().zip(expected_vec.iter()) {
                    let value_type = self.infer_type(value)?;
                    if &value_type != expected_type {
                        return Err(SemanticError::ReturnTypeMismatch {
                            function: fn_name.to_string(),
                            mismatch: TypeMismatch {
                                expected: expected_type.clone(),
                                found: value_type,
                            },
                        });
                    }
                }
            }
            _ => {
                // single return
                if values.len() != 1 {
                    return Err(SemanticError::ReturnTypeMismatch {
                        function: fn_name.to_string(),
                        mismatch: TypeMismatch {
                            expected: expected.clone(),
                            found: TypeNode::Tuple(
                                values
                                    .iter()
                                    .map(|v| self.infer_type(v))
                                    .collect::<Result<Vec<_>, _>>()?,
                            ),
                        },
                    });
                }
                let value_type = self.infer_type(&values[0])?;
                if &value_type != expected {
                    return Err(SemanticError::ReturnTypeMismatch {
                        function: fn_name.to_string(),
                        mismatch: TypeMismatch {
                            expected: expected.clone(),
                            found: value_type,
                        },
                    });
                }
            }
        }
        Ok(())
    }

    pub fn analyze_let_decl(&mut self, node: &mut AstNode) -> Result<(), SemanticError> {
        if let AstNode::LetDecl {
            mutable,
            type_annotation,
            name,
            value,
        } = node
        {
            // Check redeclaration of variable
            if self.symbol_table.contains_key(name) {
                return Err(SemanticError::VariableRedeclaration { name: name.clone() });
            }

            // Check type
            let inferred_type = if let Some(annotated_type) = type_annotation.as_ref() {
                // println!("User defined type for '{}': {:?}", name, annotated_type);
                // Validate RHS matches annotation
                let rhs_type = self.infer_type(value)?;
                if rhs_type != *annotated_type {
                    return Err(SemanticError::VarTypeMismatch(TypeMismatch {
                        expected: annotated_type.clone(),
                        found: rhs_type,
                    }));
                }
                annotated_type.clone()
            } else {
                // Infer type from value
                self.infer_type(value)?
            };

            // Append type if missing
            // This ensure static type
            *type_annotation = Some(inferred_type.clone());

            // Insert into symbol table
            self.symbol_table
                .insert(name.clone(), (inferred_type, *mutable));

            // println!("Symbol table: {:#?}", self.symbol_table);
        }
        Ok(())
    }
}
