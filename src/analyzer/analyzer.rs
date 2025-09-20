use crate::analyzer::types::{NamedError, SemanticError, TypeMismatch};
use crate::lexar::token::TokenType;
use crate::parser::ast::{AstNode, Pattern, TypeNode};
use std::collections::HashMap;

pub struct SemanticAnalyzer {
    pub(crate) symbol_table: HashMap<String, (TypeNode, bool)>, // current scope variables
    pub(crate) function_table: HashMap<String, (Vec<TypeNode>, TypeNode)>,

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
            AstNode::Assignment { pattern, value } => self.analyze_assignment(pattern, value),

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
            AstNode::Print { .. } => {
                // Delegate to the dedicated analyze_print method
                self.analyze_print(node)
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

    fn is_valid_identifier(name: &str) -> bool {
        // List of reserved keywords (sync with your lexer)
        const KEYWORDS: &[&str] = &[
            "let", "var", "fn", "import", "struct", "enum", "map", "if", "else", "for", "in",
            "return", "break", "continue", "Some", "print", "true", "false",
        ];
        // Disallow empty, reserved, or starts with digit
        if name.is_empty() || KEYWORDS.contains(&name) || name.chars().next().unwrap().is_digit(10)
        {
            return false;
        }
        true
    }

    /// Analyze an assignment statement (e.g.,`(x, y) = foo()`)
    pub fn analyze_assignment(
        &mut self,
        pattern: &Pattern,
        value: &AstNode,
    ) -> Result<(), SemanticError> {
        // Flatten the LHS pattern (tuple destructuring) and validate identifiers
        // data, user =
        let targets = self.collect_and_validate_targets(pattern)?;
        let lhs_count = targets.len();

        // Infer the RHS expression into a list of types
        // RHS type means return type of function
        let rhs_types = self.infer_rhs_types(value, lhs_count)?;

        // Ensure LHS and RHS have the same number of elements
        if rhs_types.len() != lhs_count {
            return Err(SemanticError::TupleAssignmentMismatch {
                expected: rhs_types.len(),
                found: lhs_count,
            });
        }

        // Add variables into the symbol table with their inferred types
        self.bind_targets(&targets, &rhs_types);

        Ok(())
    }

    /// Flatten a pattern (e.g., `(x, y, _)`) into a flat list of variables
    /// and ensure each identifier is valid (not reserved, not empty, etc.)
    fn collect_and_validate_targets(
        &self,
        pattern: &Pattern,
    ) -> Result<Vec<Pattern>, SemanticError> {
        let mut targets = Vec::new();
        Self::flatten_pattern(pattern, &mut targets);

        for target in &targets {
            match target {
                // Identifiers must be valid names
                Pattern::Identifier(name) if !Self::is_valid_identifier(name) => {
                    return Err(SemanticError::InvalidAssignmentTarget {
                        target: name.clone(),
                    });
                }
                // Identifiers and wildcards are always okay
                // Wildcards: `_`
                Pattern::Identifier(_) | Pattern::Wildcard => {}
                // Everything else (like nested unsupported patterns) is invalid
                _ => {
                    return Err(SemanticError::InvalidAssignmentTarget {
                        target: format!("{:?}", target),
                    });
                }
            }
        }

        Ok(targets)
    }

    // Figure out the types of the RHS expression and return them as a vector
    // - Functions can return single or multiple values
    // - Tuples spread into multiple values
    // - Simple expressions just return one type
    fn infer_rhs_types(
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

    // Validate a function call:
    // - Check if the function exists
    // - Verify number of arguments
    // - Verify each argument's type
    // - Return function’s declared return type(s)
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

    // Add all identifiers from the LHS into the current scope’s symbol table
    fn bind_targets(&mut self, targets: &[Pattern], rhs_types: &[TypeNode]) {
        for (target, ty) in targets.iter().zip(rhs_types.iter()) {
            if let Pattern::Identifier(name) = target {
                // Ignore wildcard `_`, only insert real names
                if name != "_" {
                    self.symbol_table.insert(name.clone(), (ty.clone(), true));
                }
            }
        }
    }

    /// Recursively flatten a pattern into a list of atomic targets
    /// Example: `(x, (y, _))` → `[x, y, _]`
    pub fn flatten_pattern(pattern: &Pattern, out: &mut Vec<Pattern>) {
        match pattern {
            Pattern::Identifier(_) | Pattern::Wildcard => out.push(pattern.clone()),
            Pattern::Tuple(inner) => {
                for p in inner {
                    Self::flatten_pattern(p, out);
                }
            }
            _ => out.push(pattern.clone()),
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
