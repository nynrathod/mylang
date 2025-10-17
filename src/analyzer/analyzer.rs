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
    pub imported_functions: Vec<AstNode>, // Store imported function AST nodes for MIR generation
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
            imported_functions: Vec::new(),
        }
    }

    /// Analyze a list of AST nodes (entire program or a block).
    /// Returns Ok if all nodes are semantically valid, or an error otherwise.
    /// Uses a two-pass approach:
    /// 1. First pass: Process imports and register all function signatures (for forward references)
    /// 2. Second pass: Analyze function bodies and other statements
    pub fn analyze_program(&mut self, nodes: &mut Vec<AstNode>) -> Result<(), SemanticError> {
        // FIRST PASS: Process imports and register all function signatures
        for node in nodes.iter_mut() {
            match node {
                // Process imports first to load external functions
                AstNode::Import { path, symbol } => {
                    self.import_module(path, symbol)?;
                }
                // Register local function signatures
                AstNode::FunctionDecl { name, params, return_type, .. } => {
                    // Check if function already defined
                    if self.function_table.contains_key(name) {
                        return Err(SemanticError::FunctionRedeclaration(NamedError {
                            name: name.to_string(),
                        }));
                    }
                    
                    // Collect parameter types
                    let param_types: Vec<TypeNode> = params
                        .iter()
                        .map(|(_, t)| t.clone().unwrap_or(TypeNode::Int))
                        .collect();
                    
                    // Register function signature (all functions, not just public ones)
                    self.function_table.insert(
                        name.to_string(),
                        (param_types, return_type.clone().unwrap_or(TypeNode::Void)),
                    );
                }
                _ => {} // Skip other nodes in first pass
            }
        }
        
        // SECOND PASS: Analyze all nodes (including function bodies)
        // Skip imports as they're already processed
        for node in nodes {
            if !matches!(node, AstNode::Import { .. }) {
                self.analyze_node(node)?;
            }
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

            // Import statement - already processed in first pass of analyze_program
            AstNode::Import { .. } => Ok(()),

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
            // Use analyze_program for proper two-pass analysis
            imported_analyzer.analyze_program(&mut nodes)?;
            
            // Merge public functions from imported module into current function table
            // AND store the function AST nodes for MIR generation
            for node in nodes {
                if let AstNode::FunctionDecl { name, .. } = &node {
                    // Only import functions that start with uppercase (public convention)
                    if name.chars().next().unwrap_or('a').is_uppercase() {
                        // If a specific symbol was requested, only import that symbol
                        if let Some(sym) = symbol {
                            if name == sym {
                                // Store AST node for MIR generation
                                self.imported_functions.push(node.clone());
                                // Copy function signature to current function table
                                if let Some((params, ret)) = imported_analyzer.function_table.get(name) {
                                    self.function_table.insert(name.clone(), (params.clone(), ret.clone()));
                                }
                            }
                        } else {
                            // No specific symbol - import all public functions
                            self.imported_functions.push(node.clone());
                            if let Some((params, ret)) = imported_analyzer.function_table.get(name) {
                                self.function_table.insert(name.clone(), (params.clone(), ret.clone()));
                            }
                        }
                    }
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
