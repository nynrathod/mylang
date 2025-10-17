use crate::analyzer::types::{NamedError, SemanticError};
use crate::parser::ast::{AstNode, Pattern, TypeNode};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    pub(crate) project_root: PathBuf, // Root directory for module resolution
    pub(crate) imported_modules: HashMap<String, bool>, // Track imported modules to prevent circular imports
}

impl SemanticAnalyzer {
    /// Create a new semantic analyzer with empty symbol/function tables.
    pub fn new() -> Self {
        // Set project root to X:\Projects\mylang\myproject
        // Use environment variable or current working directory
        let project_root = std::env::current_dir().unwrap().join("myproject");

        println!("[DEBUG] Project root set to: {:?}", project_root);

        Self {
            symbol_table: HashMap::new(),
            function_table: HashMap::new(),
            outer_symbol_table: None,
            project_root,
            imported_modules: HashMap::new(),
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

            // Import statement
            AstNode::Import { path, symbol } => {
                self.import_module(path, symbol)?;
                Ok(())
            }

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

    /// Resolve a module path (e.g., ["http", "Client"]) to a file path
    /// For import http::Client::Fetchuser, we want http/Client.my
    /// The last element before the symbol is the file name
    fn resolve_module_path(&self, path: &[String], symbol: &Option<String>) -> Option<PathBuf> {
        let mut buf = self.project_root.clone();

        // If we have a symbol, the path is module path + file name
        // e.g., ["http", "Client"] with symbol "Fetchuser" -> http/Client.my
        // If no symbol, treat last element as file name
        // e.g., ["http", "Client"] with no symbol -> http/Client.my

        println!("[DEBUG] Resolving path: {:?}, symbol: {:?}", path, symbol);
        println!("[DEBUG] Starting from project root: {:?}", buf);

        for part in path {
            buf.push(part);
        }
        buf.set_extension("my");

        println!("[DEBUG] Final resolved path: {:?}", buf);
        println!("[DEBUG] File exists: {}", buf.exists());

        if buf.exists() {
            Some(buf)
        } else {
            // Try to list directory contents for debugging
            if let Some(parent) = buf.parent() {
                if parent.exists() {
                    println!("[DEBUG] Parent directory exists: {:?}", parent);
                    if let Ok(entries) = std::fs::read_dir(parent) {
                        println!("[DEBUG] Directory contents:");
                        for entry in entries {
                            if let Ok(entry) = entry {
                                println!("[DEBUG]   - {:?}", entry.file_name());
                            }
                        }
                    }
                } else {
                    println!("[DEBUG] Parent directory does not exist: {:?}", parent);
                }
            }
            None
        }
    }

    /// Import a module and merge its symbols/types/functions into the current scope
    fn import_module(
        &mut self,
        path: &[String],
        symbol: &Option<String>,
    ) -> Result<(), SemanticError> {
        // Create module key for circular import detection
        let module_key = path.join("::");

        // Check for circular imports
        if self.imported_modules.contains_key(&module_key) {
            return Ok(()); // Already imported, skip
        }

        let file_path = self.resolve_module_path(path, symbol).ok_or_else(|| {
            let full_path = if let Some(sym) = symbol {
                format!("{}::{}", path.join("::"), sym)
            } else {
                path.join("::")
            };
            SemanticError::ModuleNotFound(full_path)
        })?;

        let code = fs::read_to_string(&file_path)
            .map_err(|_| SemanticError::ModuleNotFound(file_path.display().to_string()))?;

        // Mark this module as being imported
        self.imported_modules.insert(module_key, true);

        let tokens = crate::lexar::lexer::lex(&code);
        let mut parser = crate::parser::Parser::new(&tokens);
        let ast = parser
            .parse_program()
            .map_err(|_| SemanticError::ParseError)?;

        // Recursively analyze the imported AST
        if let crate::parser::ast::AstNode::Program(mut nodes) = ast {
            // Create a temporary analyzer to collect public functions from the imported module
            let mut imported_analyzer = SemanticAnalyzer::new();
            for node in &mut nodes {
                imported_analyzer.analyze_node(node)?;
            }
            // Merge public functions from imported module into global function table
            for (name, (params, ret)) in imported_analyzer.function_table.iter() {
                if name.chars().next().unwrap_or('a').is_uppercase() {
                    self.function_table
                        .insert(name.clone(), (params.clone(), ret.clone()));
                }
            }
            // If a specific symbol was requested, verify it exists
            if let Some(sym) = symbol {
                // Check if the symbol exists in function_table or symbol_table
                if !self.function_table.contains_key(sym) && !self.symbol_table.contains_key(sym) {
                    return Err(SemanticError::UndeclaredFunction(NamedError {
                        name: sym.clone(),
                    }));
                }
            }
        }
        Ok(())
    }
}
