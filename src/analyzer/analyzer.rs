use crate::parser::ast::{AstNode, TypeNode};
use std::collections::HashMap;

#[derive(Debug)]
pub enum SemanticError {
    TypeMismatch { expected: TypeNode, found: TypeNode },
    Redeclaration { name: String },
    UndeclaredVariable { name: String },
    InvalidConditionType { found: TypeNode },
}

pub struct SemanticAnalyzer {
    symbol_table: HashMap<String, (TypeNode, bool)>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            symbol_table: HashMap::new(),
        }
    }

    pub fn analyze_program(&mut self, nodes: &mut Vec<AstNode>) -> Result<(), SemanticError> {
        for node in nodes {
            self.analyze_node(node)?;
        }
        Ok(())
    }

    fn analyze_node(&mut self, node: &mut AstNode) -> Result<(), SemanticError> {
        match node {
            AstNode::LetDecl { .. } => self.analyze_let_decl(node),
            AstNode::Block(nodes) => self.analyze_program(nodes),
            AstNode::ConditionalStmt {
                condition,
                then_block,
                else_branch,
            } => self.analyze_conditional_stmt(condition, then_block, else_branch),

            // later: Assignment, FunctionDecl, ForLoopStmt, etc.
            _ => Ok(()),
        }
    }

    fn analyze_conditional_stmt(
        &mut self,
        condition: &mut AstNode,
        then_block: &mut Vec<AstNode>,
        else_branch: &mut Option<Box<AstNode>>,
    ) -> Result<(), SemanticError> {
        let cond_type = self.infer_type(condition)?;
        if cond_type != TypeNode::Bool {
            return Err(SemanticError::TypeMismatch {
                expected: TypeNode::Bool,
                found: cond_type,
            });
        }

        self.analyze_program(then_block)?;

        if let Some(else_node) = else_branch {
            self.analyze_node(else_node)?;
        }

        Ok(())
    }

    fn analyze_let_decl(&mut self, node: &mut AstNode) -> Result<(), SemanticError> {
        if let AstNode::LetDecl {
            mutable,
            type_annotation,
            name,
            value,
        } = node
        {
            // 1️⃣ Check redeclaration
            if self.symbol_table.contains_key(name) {
                return Err(SemanticError::Redeclaration { name: name.clone() });
            }

            // 2️⃣ Determine type
            let inferred_type = if let Some(annotated_type) = type_annotation.as_ref() {
                // println!("User defined type for '{}': {:?}", name, annotated_type);
                // Validate RHS matches annotation
                let rhs_type = self.infer_type(value)?;
                if rhs_type != *annotated_type {
                    return Err(SemanticError::TypeMismatch {
                        expected: annotated_type.clone(),
                        found: rhs_type,
                    });
                }
                annotated_type.clone()
            } else {
                // Infer type from value
                self.infer_type(value)?
            };

            // 3️⃣ Append type if missing
            *type_annotation = Some(inferred_type.clone()); // ✅ assign directly

            // 4️⃣ Insert into symbol table
            self.symbol_table
                .insert(name.clone(), (inferred_type, *mutable));

            println!("Symbol table: {:#?}", self.symbol_table);
        }
        Ok(())
    }

    fn infer_type(&self, node: &AstNode) -> Result<TypeNode, SemanticError> {
        match node {
            AstNode::NumberLiteral(_) => Ok(TypeNode::Int),
            AstNode::StringLiteral(_) => Ok(TypeNode::String),
            AstNode::BoolLiteral(_) => Ok(TypeNode::Bool),
            AstNode::ArrayLiteral(elements) => {
                if elements.is_empty() {
                    return Err(SemanticError::TypeMismatch {
                        expected: TypeNode::Array(Box::new(TypeNode::Int)),
                        found: TypeNode::Array(Box::new(TypeNode::Int)),
                    }); // placeholder
                }
                let first_type = self.infer_type(&elements[0])?;
                // TODO: check rest elements match first_type
                Ok(TypeNode::Array(Box::new(first_type)))
            }
            AstNode::MapLiteral(pairs) => {
                if pairs.is_empty() {
                    return Err(SemanticError::TypeMismatch {
                        expected: TypeNode::Map(
                            Box::new(TypeNode::String),
                            Box::new(TypeNode::Int),
                        ),
                        found: TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::Int)),
                    }); // placeholder
                }
                let key_type = self.infer_type(&pairs[0].0)?;
                let value_type = self.infer_type(&pairs[0].1)?;
                // TODO: check rest pairs match key/value type
                Ok(TypeNode::Map(Box::new(key_type), Box::new(value_type)))
            }
            AstNode::Identifier(name) => {
                if let Some((t, _)) = self.symbol_table.get(name) {
                    Ok(t.clone())
                } else {
                    // Variable used before declaration
                    return Err(SemanticError::UndeclaredVariable { name: name.clone() });
                }
            }

            AstNode::BinaryExpr { left, op: _, right } => {
                let left_type = self.infer_type(left)?;
                let right_type = self.infer_type(right)?;
                if left_type != right_type {
                    return Err(SemanticError::TypeMismatch {
                        expected: left_type,
                        found: right_type,
                    });
                }
                Ok(left_type) // result type same as operands
            }
            AstNode::UnaryExpr { op: _, expr } => self.infer_type(expr),
            _ => unimplemented!(),
        }
    }
}
