use super::analyzer::SemanticAnalyzer;
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
}
