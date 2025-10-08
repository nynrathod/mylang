use super::analyzer::SemanticAnalyzer;
use std::collections::HashMap;

use super::types::{NamedError, SemanticError, TypeMismatch};
use crate::analyzer::analyzer::SymbolInfo;
use crate::parser::ast::{AstNode, TypeNode};

impl SemanticAnalyzer {
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
            return Err(SemanticError::FunctionRedeclaration(NamedError {
                name: name.to_string(),
            }));
        }
        let param_types: Vec<TypeNode> = params.iter().map(|(_, t)| t.clone().unwrap()).collect();

        self.function_table.insert(
            name.to_string(),
            (param_types, return_type.clone().unwrap_or(TypeNode::Void)),
        );

        // Is public or private function
        if visibility == "Public" {
            if let Some(first_char) = name.chars().next() {
                if !first_char.is_uppercase() {
                    return Err(SemanticError::InvalidPublicName(NamedError {
                        name: name.to_string(),
                    }));
                }
            }
        }

        // Create a local scope for function parameters
        let mut local_scope: HashMap<String, SymbolInfo> = HashMap::new();

        for (param_name, param_type) in params.iter() {
            // Type is mandator if parameter passed. Check type exists
            let param_type = param_type.as_ref().ok_or_else(|| {
                SemanticError::MissingParamType(NamedError {
                    name: param_name.clone(),
                })
            })?;

            // Check duplicate param names
            if local_scope.contains_key(param_name) {
                return Err(SemanticError::FunctionParamRedeclaration(NamedError {
                    name: param_name.clone(),
                }));
            }

            // Insert parameter into local scope (immutable)
            local_scope.insert(
                param_name.clone(),
                SymbolInfo {
                    ty: param_type.clone(),
                    mutable: false,
                    is_ref_counted: Self::should_be_rc(&param_type),
                },
            );
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

        println!(
            "Restoring symbol table for function {}, outer_symbol_table is: {:?}",
            name, self.outer_symbol_table
        );

        // Restore outer scope
        if let Some(outer) = self.outer_symbol_table.take() {
            self.symbol_table = outer;
        }

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
                            value: None,
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
                                value: None,
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
                            value: None,
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
                            value: None,
                        },
                    });
                }
            }
        }
        Ok(())
    }

    pub fn analyze_let_decl(&mut self, node: &mut AstNode) -> Result<(), SemanticError> {
        match node {
            AstNode::LetDecl {
                mutable,
                type_annotation,
                pattern,
                value,
                is_ref_counted,
            } => {
                // Prevent assignment from variable without type info
                // if type_annotation.is_none() && matches!(**value, AstNode::Identifier(_)) {
                //     return Err(SemanticError::VarTypeMismatch(TypeMismatch {
                //         expected: TypeNode::Void,
                //         found: TypeNode::Void,
                //         value: None,
                //     }));
                // }

                // Determine the type of the RHS (value being assigned)
                let rhs_type = if let Some(annotated_type) = type_annotation.as_ref() {
                    // If the variable has a type annotation, check that RHS matches it
                    let rhs_type = self.infer_type(value)?;
                    if rhs_type != *annotated_type {
                        return Err(SemanticError::VarTypeMismatch(TypeMismatch {
                            expected: annotated_type.clone(),
                            found: rhs_type,
                            value: Some(value.clone()),
                        }));
                    }
                    annotated_type.clone()
                } else {
                    // No annotation: infer type from the value
                    self.infer_type(value)?
                };

                // Update the type annotation to reflect the inferred type if it was missing
                *type_annotation = Some(rhs_type.clone());

                // ✅ Update AST with reference counting info
                // println!("Before: {:?}", is_ref_counted);
                *is_ref_counted = Some(Self::should_be_rc(&rhs_type));
                // println!("After: {:?}", is_ref_counted);

                // Flatten the LHS pattern
                let mut targets = Vec::new();
                Self::flatten_pattern(pattern, &mut targets);

                // If RHS is a tuple, each element must match a pattern
                // Otherwise, treat RHS as a single-element list
                let rhs_types = match &rhs_type {
                    TypeNode::Tuple(types) => types.clone(),
                    t => vec![t.clone()],
                };

                // Check that the number of LHS patterns matches the number of RHS types
                if rhs_types.len() != targets.len() {
                    return Err(SemanticError::TupleAssignmentMismatch {
                        expected: rhs_types.len(),
                        found: targets.len(),
                    });
                }

                // Bind each pattern to its type in the symbol table
                for (target, ty) in targets.iter().zip(rhs_types.iter()) {
                    match target {
                        // Identifier: add to symbol table, mark mutability
                        crate::parser::ast::Pattern::Identifier(name) => {
                            // Prevent redeclaration
                            if self.symbol_table.contains_key(name) {
                                return Err(SemanticError::VariableRedeclaration(NamedError {
                                    name: name.clone(),
                                }));
                            }
                            // Skip wildcards
                            if name != "_" {
                                self.symbol_table.insert(
                                    name.clone(),
                                    SymbolInfo {
                                        ty: ty.clone(),
                                        mutable: *mutable,
                                        is_ref_counted: Self::should_be_rc(&ty),
                                    },
                                );
                            }
                        }
                        // Wildcard: allowed but not stored
                        crate::parser::ast::Pattern::Wildcard => {}
                        // Anything else: invalid pattern
                        _ => {
                            return Err(SemanticError::InvalidAssignmentTarget {
                                target: format!("{:?}", target),
                            });
                        }
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn analyze_struct(&mut self, node: &AstNode) -> Result<(), SemanticError> {
        if let AstNode::StructDecl { name, fields } = node {
            if self.symbol_table.contains_key(name) {
                return Err(SemanticError::StructRedeclaration(NamedError {
                    name: name.clone(),
                }));
            }

            let mut field_map = HashMap::new();
            for (field_name, field_type) in fields {
                if field_map.contains_key(field_name) {
                    return Err(SemanticError::DuplicateField {
                        struct_name: name.clone(),
                        field: field_name.clone(),
                    });
                }
                field_map.insert(field_name.clone(), field_type.clone());
            }

            // Insert struct type into the symbol table
            self.symbol_table.insert(
                name.clone(),
                SymbolInfo {
                    ty: TypeNode::Struct(name.clone(), field_map),
                    mutable: false,
                    is_ref_counted: true,
                },
            );
        }
        Ok(())
    }

    /// Analyze an enum declaration
    pub fn analyze_enum(&mut self, node: &AstNode) -> Result<(), SemanticError> {
        if let AstNode::EnumDecl { name, variants } = node {
            if self.symbol_table.contains_key(name) {
                return Err(SemanticError::EnumRedeclaration(NamedError {
                    name: name.clone(),
                }));
            }

            let mut variant_map = HashMap::new();
            for (variant_name, variant_type) in variants {
                if variant_map.contains_key(variant_name) {
                    return Err(SemanticError::DuplicateEnumVariant {
                        enum_name: name.clone(),
                        variant: variant_name.clone(),
                    });
                }
                variant_map.insert(variant_name.clone(), variant_type.clone());
            }

            // Insert enum type into the symbol table
            self.symbol_table.insert(
                name.clone(),
                SymbolInfo {
                    ty: TypeNode::Enum(name.clone(), variant_map),
                    mutable: false,
                    is_ref_counted: true,
                },
            );
        }
        Ok(())
    }
}
