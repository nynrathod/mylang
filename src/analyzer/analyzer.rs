use crate::analyzer::types::{NamedError, SemanticError};
use crate::parser::ast::{AstNode, Pattern, TypeNode};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct SymbolInfo {
    pub ty: TypeNode,         // The type of the variable
    pub mutable: bool,        // Is the variable mutable?
    pub is_ref_counted: bool, // Should reference counting be used?
}

/// The main semantic analyzer for the language.
/// Responsible for type checking, symbol resolution, and semantic validation.
pub struct SemanticAnalyzer {
    pub(crate) symbol_table: HashMap<String, SymbolInfo>, // Current scope variables
    pub(crate) function_table: HashMap<String, (Vec<TypeNode>, TypeNode)>, // Function signatures

    pub(crate) outer_symbol_table: Option<HashMap<String, SymbolInfo>>, // For nested scopes
}

impl SemanticAnalyzer {
    /// Create a new semantic analyzer with empty symbol/function tables.
    pub fn new() -> Self {
        Self {
            symbol_table: HashMap::new(),
            function_table: HashMap::new(),
            outer_symbol_table: None,
        }
    }

    /// Analyze a list of AST nodes (entire program or a block).
    /// Returns Ok if all nodes are semantically valid, or an error otherwise.
    pub fn analyze_program(&mut self, nodes: &mut Vec<AstNode>) -> Result<(), SemanticError> {
        for node in nodes {
            self.analyze_node(node)?;
        }
        Ok(())
    }

    /// Determines if a type should use reference counting.
    /// Used for arrays, maps, and strings.
    pub fn should_be_rc(ty: &TypeNode) -> bool {
        matches!(
            ty,
            TypeNode::Array(_) | TypeNode::Map(_, _) | TypeNode::String
        )
    }

    /// Dispatch analysis based on AST node type.
    /// Calls the appropriate analysis function for each AST node variant.
    /// Ensures semantic correctness for declarations, assignments, control flow, etc.
    pub fn analyze_node(&mut self, node: &mut AstNode) -> Result<(), SemanticError> {
        match node {
            // Declarations
            AstNode::LetDecl { .. } => self.analyze_let_decl(node),
            AstNode::FunctionDecl {
                name,
                visibility,
                params,
                return_type,
                body,
            } => self.analyze_functional_decl(name, visibility, params, return_type, body),
            AstNode::StructDecl { .. } => self.analyze_struct(node),
            AstNode::EnumDecl { .. } => self.analyze_enum(node),

            // Statements
            AstNode::Assignment { pattern, value } => self.analyze_assignment(pattern, value),
            AstNode::Return { values } => {
                // Check return value types
                for v in values {
                    self.infer_type(v)?;
                }
                Ok(())
            }
            AstNode::Print { .. } => self.analyze_print(node),
            AstNode::Break | AstNode::Continue => Ok(()),
            AstNode::ConditionalStmt {
                condition,
                then_block,
                else_branch,
            } => self.analyze_conditional_stmt(condition, then_block, else_branch),
            AstNode::ForLoopStmt {
                pattern,
                iterable,
                body,
            } => self.analyze_for_stmt(pattern, iterable.as_deref_mut(), body),
            AstNode::Block(nodes) => self.analyze_program(nodes),

            // Catch-all for any AST nodes not explicitly handled above.
            // We call `infer_type` to:
            // Validate that all identifiers exist in scope.
            // Ensure expressions (literals, binary/unary ops, function calls) are type-correct.
            // Future-proof: new AST node types will still be semantically validated.
            _ => {
                self.infer_type(node)?;
                Ok(())
            }
        }
    }
}
