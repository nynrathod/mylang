use super::analyzer::SemanticAnalyzer;
use super::types::{SemanticError, TypeMismatch};
use crate::analyzer::analyzer::SymbolInfo;
use crate::analyzer::types::NamedError;
use crate::lexar::token::TokenType;
use crate::parser::ast::{AstNode, Pattern, TypeNode};
use std::collections::HashMap;

impl SemanticAnalyzer {
    /// Analyze an assignment statement
    /// (e.g., `(x, y) = foo()` if lhs x,y types match with right foo return types).
    /// Checks that the left and right sides match in number and type,
    /// and binds variables to the symbol table.
    pub fn analyze_assignment(
        &mut self,
        pattern: &Pattern,
        value: &AstNode,
    ) -> Result<(), SemanticError> {
        // Flatten the LHS pattern (tuple destructuring) and validate identifiers
        let targets = self.collect_and_validate_targets(pattern)?;
        let lhs_count = targets.len();

        // Infer the RHS expression into a list of types
        let rhs_types = self.infer_rhs_types(value, lhs_count)?;

        // Ensure LHS and RHS have the same number of elements
        if rhs_types.len() != lhs_count {
            return Err(SemanticError::TupleAssignmentMismatch {
                expected: rhs_types.len(),
                found: lhs_count,
            });
        }

        // Check mutability for each assignment target
        for (target, _) in targets.iter().zip(rhs_types.iter()) {
            if let Pattern::Identifier(name) = target {
                match self.symbol_table.get(name) {
                    Some(info) => {
                        if !info.mutable {
                            return Err(SemanticError::InvalidAssignmentTarget {
                                target: format!("Cannot assign to immutable variable '{}'", name),
                            });
                        }
                    }
                    None => {
                        return Err(SemanticError::UndeclaredVariable(NamedError {
                            name: name.clone(),
                        }));
                    }
                }
            }
        }

        // Add variables into the symbol table with their inferred types (for new declarations)
        self.bind_targets(&targets, &rhs_types);

        Ok(())
    }

    /// Analyze a compound assignment statement (e.g., `x += 1`, `y *= 2`)
    /// Checks that:
    /// 1. The variable exists and is mutable
    /// 2. The operation is valid for the variable's type
    /// 3. The RHS type matches the variable's type
    pub fn analyze_compound_assignment(
        &mut self,
        pattern: &Pattern,
        op: TokenType,
        value: &AstNode,
    ) -> Result<(), SemanticError> {
        // Only single identifier is allowed for compound assignment
        let var_name = match pattern {
            Pattern::Identifier(name) => name,
            _ => {
                return Err(SemanticError::InvalidAssignmentTarget {
                    target: "Compound assignment only supports single variables".to_string(),
                });
            }
        };

        // Check if variable exists
        let var_info = match self.symbol_table.get(var_name) {
            Some(info) => info.clone(),
            None => {
                return Err(SemanticError::UndeclaredVariable(NamedError {
                    name: var_name.clone(),
                }));
            }
        };

        // Check if variable is mutable
        if !var_info.mutable {
            return Err(SemanticError::InvalidAssignmentTarget {
                target: format!("Cannot assign to immutable variable '{}'", var_name),
            });
        }

        // Infer the type of the RHS expression
        let rhs_type = self.infer_type(value)?;

        // Check if the operation is valid for the variable's type
        // Compound assignment requires both operands to be the same type
        let result_type = match op {
            TokenType::PlusEq | TokenType::MinusEq | TokenType::StarEq | TokenType::SlashEq => {
                match (&var_info.ty, &rhs_type) {
                    (TypeNode::Int, TypeNode::Int) => Ok(TypeNode::Int),
                    (TypeNode::Float, TypeNode::Float) => Ok(TypeNode::Float),
                    (TypeNode::String, TypeNode::String) if matches!(op, TokenType::PlusEq) => {
                        Ok(TypeNode::String)
                    }
                    _ => Err(SemanticError::OperatorTypeMismatch(TypeMismatch {
                        expected: var_info.ty.clone(),
                        found: rhs_type.clone(),
                        value: None,
                        line: None,
                        col: None,
                    })),
                }
            }
            _ => {
                return Err(SemanticError::UnexpectedNode {
                    expected: format!("Invalid compound assignment operator: {:?}", op),
                });
            }
        }?;

        // The result type should match the variable's type
        if result_type != var_info.ty {
            return Err(SemanticError::VarTypeMismatch(TypeMismatch {
                expected: var_info.ty.clone(),
                found: result_type,
                value: None,
                line: None,
                col: None,
            }));
        }

        Ok(())
    }

    /// Flattens a pattern (e.g., `(x, y, _)`) into a flat list of variables.
    /// Ensures each identifier is valid (not reserved, not empty, etc.).
    pub fn collect_and_validate_targets(
        &self,
        pattern: &Pattern,
    ) -> Result<Vec<Pattern>, SemanticError> {
        let mut targets = Vec::new();
        match pattern {
            Pattern::Identifier(name) if !Self::is_valid_identifier(name) => {
                return Err(SemanticError::InvalidAssignmentTarget {
                    target: name.clone(),
                });
            }
            Pattern::Identifier(_) | Pattern::Wildcard => {
                targets.push(pattern.clone());
            }
            Pattern::Tuple(names) => {
                for p in names {
                    match p {
                        Pattern::Identifier(_) | Pattern::Wildcard => {
                            targets.push(p.clone());
                        }
                        _ => {
                            return Err(SemanticError::InvalidAssignmentTarget {
                                target: format!("{:?}", p),
                            });
                        }
                    }
                }
            }
            _ => {
                return Err(SemanticError::InvalidAssignmentTarget {
                    target: format!("{:?}", pattern),
                });
            }
        }
        Ok(targets)
    }

    /// Adds all identifiers from the LHS into the current scope’s symbol table.
    /// Ignores wildcards (`_`), only inserts real variable names.
    fn bind_targets(&mut self, targets: &[Pattern], rhs_types: &[TypeNode]) {
        for (target, ty) in targets.iter().zip(rhs_types.iter()) {
            if let Pattern::Identifier(name) = target {
                // Ignore wildcard `_`, only insert real names
                if name != "_" {
                    self.symbol_table.insert(
                        name.clone(),
                        SymbolInfo {
                            ty: ty.clone(),
                            mutable: true,
                            is_ref_counted: Self::should_be_rc(&ty),
                        },
                    );
                }
            }
        }
    }

    /// Infers the types of the RHS expression and returns them as a vector.
    /// - Functions can return single or multiple values.
    /// - Tuples spread into multiple values.
    /// - Simple expressions just return one type.
    pub fn infer_rhs_types(
        &self,
        value: &AstNode,
        lhs_count: usize,
    ) -> Result<Vec<TypeNode>, SemanticError> {
        match value {
            // Function call: check validity and return types
            AstNode::FunctionCall { func, args } => self.check_function_call(func, args),

            // Tuple literal: infer each element's type
            AstNode::TupleLiteral(elements) => {
                elements.iter().map(|e| self.infer_type(e)).collect()
            }

            // If LHS expects multiple values but RHS isn’t tuple/function → error
            _ if lhs_count > 1 => Err(SemanticError::InvalidFunctionCall {
                func: format!("{:?}", value),
            }),

            // Single assignment: just infer the one type
            _ => Ok(vec![self.infer_type(value)?]),
        }
    }

    /// Validates a function call:
    /// - Checks if the function exists.
    /// - Verifies number and types of arguments.
    /// - Returns function’s declared return type(s).
    fn check_function_call(
        &self,
        func: &AstNode,
        args: &[AstNode],
    ) -> Result<Vec<TypeNode>, SemanticError> {
        // Ensure the call target is a simple identifier (not `foo.bar` or similar yet)
        let name = if let AstNode::Identifier(n) = &*func {
            n
        } else {
            return Err(SemanticError::InvalidFunctionCall {
                func: format!("{:?}", func),
            });
        };

        // Look up function definition in the table
        if let Some((param_types, ret_ty)) = self.function_table.get(name.as_str()) {
            // Check number of arguments
            if args.len() != param_types.len() {
                return Err(SemanticError::FunctionArgumentMismatch {
                    name: name.clone(),
                    expected: param_types.len(),
                    found: args.len(),
                });
            }

            // Check argument types
            for (arg, expected_ty) in args.iter().zip(param_types.iter()) {
                let arg_ty = self.infer_type(arg)?;
                if &arg_ty != expected_ty {
                    return Err(SemanticError::FunctionArgumentTypeMismatch {
                        name: name.clone(),
                        expected: expected_ty.clone(),
                        found: arg_ty,
                    });
                }
            }

            // Return type(s)
            Ok(match ret_ty {
                TypeNode::Tuple(types) => types.clone(), // multi-value
                t => vec![t.clone()],                    // single value
            })
        } else {
            Err(SemanticError::UndeclaredFunction(NamedError {
                name: name.clone(),
            }))
        }
    }

    /// Checks if an identifier name is valid (not a keyword, not empty, not starting with a digit).
    /// Used for variable and function names.
    fn is_valid_identifier(name: &str) -> bool {
        // List of reserved keywords (sync with your lexer)
        const KEYWORDS: &[&str] = &[
            "let", "fn", "import", "struct", "enum", "map", "if", "else", "for", "in", "return",
            "break", "continue", "print", "true", "false",
        ];
        // Disallow empty, reserved, or starts with digit
        if name.is_empty() || KEYWORDS.contains(&name) || name.chars().next().unwrap().is_digit(10)
        {
            return false;
        }
        true
    }

    /// - Checks that each expression in the print statement is of a printable type.
    /// - Infers the type of each expression and ensures it's allowed
    /// (int, float, bool, string, array, map, tuple).
    /// Note: Float not supported yet as type yet, TODO later
    pub fn analyze_print(&mut self, node: &mut AstNode) -> Result<(), SemanticError> {
        if let AstNode::Print { exprs } = node {
            for expr in exprs.iter_mut() {
                let ty = self.infer_type(expr)?;
                // Only allow printing of supported types.
                match ty {
                    TypeNode::Int
                    | TypeNode::Float
                    | TypeNode::Bool
                    | TypeNode::String
                    | TypeNode::Array(_)
                    | TypeNode::Map(_, _)
                    | TypeNode::Tuple(_) => {
                        // Supported type for printing.
                    }
                    _ => {
                        // If the type is not supported, return an error.
                        return Err(SemanticError::InvalidPrintType { found: ty });
                    }
                }
                // Recursively analyze the expression for semantic correctness.
                self.analyze_node(expr)?;
            }
            Ok(())
        } else {
            // NOTE: This branch should never be reached in normal operation.
            // It exists only as a safeguard in case the dispatcher calls this function
            // with a non-Print node, which would indicate a bug elsewhere in the analyzer.
            Err(SemanticError::UnexpectedNode {
                expected: "print".to_string(),
            })
        }
    }

    /// Analyze a conditional statement (if/else).
    /// - Ensures the condition expression evaluates to a boolean type.
    /// - Returns an error if the condition is not a boolean.
    /// - Creates a new scope for the then and else blocks to ensure proper scope isolation.
    pub fn analyze_conditional_stmt(
        &mut self,
        condition: &mut AstNode,
        then_block: &mut Vec<AstNode>,
        else_branch: &mut Option<Box<AstNode>>,
    ) -> Result<(), SemanticError> {
        // The condition of an if/else must always be a boolean.
        let cond_type = self.infer_type(condition)?;
        if cond_type != TypeNode::Bool {
            // If the condition is not a boolean, return an error.
            return Err(SemanticError::InvalidConditionType(TypeMismatch {
                expected: TypeNode::Bool,
                found: cond_type,
                value: None,
                line: None,
                col: None,
            }));
        }

        // Create a new scope for the 'then' block that inherits from the current scope
        let saved_table = self.symbol_table.clone();
        self.scope_stack.push(saved_table.clone());
        // Keep the current symbol table so variables from outer scope are visible
        // New variables declared in the then block will be added to this table

        // Analyze the 'then' block with its own scope
        self.analyze_program(then_block)?;

        // Restore the original symbol table to ensure variables from 'then' block don't leak
        if let Some(prev_scope) = self.scope_stack.pop() {
            self.symbol_table = prev_scope;
        }

        // If there is an 'else' branch, analyze it with its own scope as well
        if let Some(else_node) = else_branch {
            // Create a new scope for the 'else' branch that inherits from the current scope
            let else_saved_table = self.symbol_table.clone();
            self.scope_stack.push(else_saved_table.clone());
            // Keep the current symbol table so variables from outer scope are visible

            // Analyze the 'else' branch
            self.analyze_node(else_node)?;

            // Restore the symbol table after analyzing the 'else' branch
            if let Some(prev_scope) = self.scope_stack.pop() {
                self.symbol_table = prev_scope;
            }
        }

        Ok(())
    }

    /// - Sets up a new scope for loop variables.
    /// - Checks the type of the iterable expression.
    /// - For arrays: expects a single variable pattern.
    /// - For maps: expects a tuple pattern (key, value).
    /// - For ranges: expects a single variable or wildcard.
    /// - For infinite loops (no iterable): only allows wildcard.
    /// - Binds loop variables to their types in the symbol table.
    /// - Restores the outer symbol table after the loop.
    /// - Returns errors for invalid patterns or non-iterable types.
    pub fn analyze_for_stmt(
        &mut self,
        pattern: &mut Pattern,
        iterable: Option<&mut AstNode>,
        body: &mut Vec<AstNode>,
    ) -> Result<(), SemanticError> {
        // Save the outer symbol table and create a new scope for the loop.
        let outer_table = self.symbol_table.clone();
        let mut loop_scope = outer_table.clone();
        // Swap the current symbol table with the loop scope.
        // This ensures variables declared inside the loop are scoped to the loop body
        // and do not leak into the outer scope. We'll restore the outer scope after analysis.
        std::mem::swap(&mut self.symbol_table, &mut loop_scope);

        if let Some(iter_node) = iterable {
            // Infer the type of the iterable expression.
            let iter_type = self.infer_type(iter_node)?;

            match iter_type {
                TypeNode::Array(elem_type) => {
                    // For arrays, only a single variable pattern is allowed.
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
                    // For maps, expect a tuple pattern with two elements (key, value).
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
                TypeNode::Range(_, _, _) => {
                    // For ranges, only a single variable or wildcard is allowed.
                    if let Pattern::Identifier(_) | Pattern::Wildcard = pattern {
                        // Range iterator is always Int.
                        self.bind_pattern_to_type(pattern, &TypeNode::Int)?;
                    } else {
                        return Err(SemanticError::InvalidAssignmentTarget {
                            target: "Loop variable pattern does not match range type".to_string(),
                        });
                    }
                }

                _ => {
                    // If the iterable is not an array, map, or range, return an error.
                    return Err(SemanticError::InvalidAssignmentTarget {
                        target: "Cannot iterate non-iterable type".to_string(),
                    });
                }
            }
        } else {
            // For infinite loops (no iterable), only wildcard is allowed.
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

        // Increment loop depth before analyzing the loop body
        self.loop_depth += 1;
        // Analyze the loop body for semantic correctness.
        self.analyze_program(body)?;
        // Decrement loop depth after analyzing the loop body
        self.loop_depth -= 1;
        // Restore the outer symbol table after the loop.
        self.symbol_table = outer_table;
        Ok(())
    }

    /// Binds a pattern to a type in the symbol table.
    /// - For identifiers: adds the variable to the symbol table with the given type.
    /// - For wildcards: ignores (does not bind).
    /// - For tuple patterns: recursively binds each element to the corresponding type in a tuple.
    /// - Returns errors for redeclaration, mismatched tuple lengths, or invalid patterns.
    fn bind_pattern_to_type(
        &mut self,
        pattern: &mut Pattern,
        ty: &TypeNode,
    ) -> Result<(), SemanticError> {
        match pattern {
            Pattern::Identifier(name) => {
                // If the variable is already declared in the current scope, return an error.
                if self.symbol_table.contains_key(name) {
                    return Err(SemanticError::VariableRedeclaration(NamedError {
                        name: name.clone(),
                    }));
                }
                // Add the variable to the symbol table with the given type.
                self.symbol_table.insert(
                    name.clone(),
                    SymbolInfo {
                        ty: ty.clone(),
                        mutable: false,
                        is_ref_counted: Self::should_be_rc(&ty),
                    },
                );
            }
            Pattern::Wildcard => {
                // Wildcard pattern `_` does not bind any variable.
            }
            Pattern::Tuple(patterns) => match ty {
                // For tuple patterns, recursively bind each element to the corresponding type.
                TypeNode::Tuple(types) if types.len() == patterns.len() => {
                    for (p, t) in patterns.iter_mut().zip(types.iter()) {
                        self.bind_pattern_to_type(p, t)?;
                    }
                }
                // If the type is not a tuple or the lengths do not match, return an error.
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
                // Any other pattern is invalid.
                return Err(SemanticError::InvalidAssignmentTarget {
                    target: format!("{:?}", pattern),
                });
            }
        }
        Ok(())
    }
}
