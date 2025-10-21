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
    pub loop_depth: usize,                // Track loop nesting for break/continue error handling
    pub scope_stack: Vec<HashMap<String, SymbolInfo>>, // Scope stack for block scoping
}

impl SemanticAnalyzer {
    /// Create a new semantic analyzer with empty symbol/function tables.
    pub fn new(project_root: Option<PathBuf>) -> Self {
        let project_root = project_root.unwrap_or_else(|| std::env::current_dir().unwrap());
        if cfg!(debug_assertions) {
            println!("[DEBUG] Project root set to: {:?}", project_root);
        }

        Self {
            symbol_table: HashMap::new(),
            function_table: HashMap::new(),
            outer_symbol_table: None,
            project_root,
            imported_modules: HashMap::new(),
            imported_functions: Vec::new(),
            loop_depth: 0,
            scope_stack: Vec::new(),
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
                    if let Err(e) = self.import_module(path, symbol) {
                        return Err(e);
                    }
                }
                // Register local function signatures
                AstNode::FunctionDecl {
                    name,
                    params,
                    return_type,
                    ..
                } => {
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
            AstNode::Break => {
                // Error if not inside a loop
                if self.loop_depth == 0 {
                    return Err(SemanticError::UnexpectedNode {
                        expected: "break inside loop".to_string(),
                    });
                }
                Ok(())
            }
            AstNode::Continue => {
                // Error if not inside a loop
                if self.loop_depth == 0 {
                    return Err(SemanticError::UnexpectedNode {
                        expected: "continue inside loop".to_string(),
                    });
                }
                Ok(())
            }
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
            AstNode::Block(nodes) => {
                // Save the current symbol table (scope)
                let parent_scope = self.symbol_table.clone();
                // Push current scope onto stack and start with a fresh scope for the block
                self.scope_stack.push(parent_scope.clone());
                self.symbol_table = HashMap::new();
                let result = self.analyze_program(nodes);
                // Pop scope from stack and restore
                if let Some(prev_scope) = self.scope_stack.pop() {
                    self.symbol_table = prev_scope;
                }
                result
            }

            // Catch-all for any AST nodes not explicitly handled above.
            // We call `infer_type` to:
            // Validate that all identifiers exist in scope.
            // Ensure expressions (literals, binary/unary ops, function calls) are type-correct.
            // Future-proof: new AST node types will still be semantically validated.
            _ => {
                // Add function call argument count/type checking
                if let AstNode::FunctionCall { func, args } = node {
                    // Try to extract function name from Identifier node
                    let func_name = if let AstNode::Identifier(name) = &**func {
                        name
                    } else {
                        return Err(SemanticError::InvalidFunctionCall {
                            func: format!("{:?}", func),
                        });
                    };

                    let (param_types, _return_type) =
                        self.function_table.get(func_name).ok_or_else(|| {
                            SemanticError::UndeclaredFunction(NamedError {
                                name: func_name.clone(),
                            })
                        })?;

                    // Check argument count
                    if args.len() != param_types.len() {
                        return Err(SemanticError::FunctionArgumentMismatch {
                            name: func_name.clone(),
                            expected: param_types.len(),
                            found: args.len(),
                        });
                    }

                    // Check argument types
                    for (arg, expected_type) in args.iter().zip(param_types.iter()) {
                        let arg_type = self.infer_type(arg)?;
                        if arg_type != *expected_type {
                            return Err(SemanticError::FunctionArgumentTypeMismatch {
                                name: func_name.clone(),
                                expected: expected_type.clone(),
                                found: arg_type,
                            });
                        }
                    }

                    // Return type is not used here, but could be returned if needed
                    Ok(())
                } else {
                    self.infer_type(node)?;
                    Ok(())
                }
            }
        }
    }

    // Helper to check if currently inside a loop (for break/continue validation)

    /// Resolve a module path (e.g., ["http", "Client"]) to a file path
    /// For import http::Client::Fetchuser, we want http/Client.my
    /// The last element before the symbol is the file name
    fn resolve_module_path(&self, path: &[String], symbol: &Option<String>) -> Option<PathBuf> {
        let mut buf = self.project_root.clone();

        // For imports like http::Client::Fetchuser, we want http/Client.my
        // The path will be ["http", "Client"]
        for part in path {
            buf.push(part);
        }

        // Add .my extension
        buf.set_extension("my");

        if buf.exists() {
            Some(buf)
        } else {
            None
        }
    }

    fn import_module(
        &mut self,
        path: &[String],
        symbol: &Option<String>,
    ) -> Result<(), SemanticError> {
        // Create module key for circular import detection

        let module_key = path.join("::");

        // If importing a specific symbol, check if that symbol is already imported
        if let Some(sym) = symbol {
            if self.function_table.contains_key(sym) {
                return Ok(()); // Symbol already imported, skip
            }
        }

        // Check if we've already fully analyzed this module (for wildcard imports)
        let already_analyzed = self.imported_modules.contains_key(&module_key);

        let file_path = self.resolve_module_path(path, symbol).ok_or_else(|| {
            let full_path = if let Some(sym) = symbol {
                format!("{}::{}", path.join("::"), sym)
            } else {
                path.join("::")
            };
            SemanticError::ModuleNotFound(full_path)
        })?;

        // If this module was already analyzed, we can reuse the cached analysis

        // We only need to parse and analyze once per module file

        let (nodes, imported_analyzer) = if already_analyzed {
            // Module already analyzed, just parse to get the AST nodes

            let code = fs::read_to_string(&file_path)
                .map_err(|_| SemanticError::ModuleNotFound(file_path.display().to_string()))?;
            let tokens = crate::lexar::lexer::lex(&code);

            let mut parser = crate::parser::Parser::new(&tokens);

            let ast = parser
                .parse_program()
                .map_err(|e| SemanticError::ParseErrorInModule {
                    file: file_path.display().to_string(),
                    error: e.to_string(),
                })?;

            if let crate::parser::ast::AstNode::Program(nodes) = ast {
                // Create a temporary analyzer and analyze (will be fast since already done)
                let mut imported_analyzer = SemanticAnalyzer::new(Some(self.project_root.clone()));
                let mut nodes_mut = nodes.clone();
                imported_analyzer.analyze_program(&mut nodes_mut)?;
                (nodes, imported_analyzer)
            } else {
                return Ok(());
            }
        } else {
            // First time analyzing this module

            let code = fs::read_to_string(&file_path)
                .map_err(|_| SemanticError::ModuleNotFound(file_path.display().to_string()))?;

            // Mark this module as being imported

            self.imported_modules.insert(module_key, true);

            let tokens = crate::lexar::lexer::lex(&code);

            let mut parser = crate::parser::Parser::new(&tokens);

            let ast = parser
                .parse_program()
                .map_err(|e| SemanticError::ParseErrorInModule {
                    file: file_path.display().to_string(),
                    error: e.to_string(),
                })?;

            // Recursively analyze the imported AST
            if let crate::parser::ast::AstNode::Program(mut nodes) = ast {
                // Create a temporary analyzer to collect public functions from the imported module

                let mut imported_analyzer = SemanticAnalyzer::new(Some(self.project_root.clone()));

                // Use analyze_program for proper two-pass analysis

                imported_analyzer.analyze_program(&mut nodes)?;

                (nodes, imported_analyzer)
            } else {
                println!(
                    "[WARNING] [import_module] Parsed AST from {:?} is not a Program variant",
                    file_path
                );
                return Ok(());
            }
        };

        // Merge public functions from imported module into current function table
        // AND store the function AST nodes for MIR generation
        for node in nodes {
            if let AstNode::FunctionDecl { name, .. } = &node {
                // Only import functions that start with uppercase (public convention)
                if name.chars().next().unwrap_or('a').is_uppercase() {
                    // If a specific symbol was requested, only import that symbol
                    if let Some(sym) = symbol {
                        if name == sym {
                            // Store AST node for MIR generation (only if not already stored)
                            if !self.imported_functions.iter().any(|n| {
                                if let AstNode::FunctionDecl { name: fn_name, .. } = n {
                                    fn_name == name
                                } else {
                                    false
                                }
                            }) {
                                self.imported_functions.push(node.clone());
                            }
                            // Copy function signature to current function table
                            if let Some((params, ret)) = imported_analyzer.function_table.get(name)
                            {
                                self.function_table
                                    .insert(name.clone(), (params.clone(), ret.clone()));
                            } else {
                                println!("[WARNING] [import_module] Function '{}' not found in imported_analyzer.function_table", name);
                            }
                        }
                    } else {
                        // No specific symbol - import all public functions
                        if !self.imported_functions.iter().any(|n| {
                            if let AstNode::FunctionDecl { name: fn_name, .. } = n {
                                fn_name == name
                            } else {
                                false
                            }
                        }) {
                            self.imported_functions.push(node.clone());
                        }
                        if let Some((params, ret)) = imported_analyzer.function_table.get(name) {
                            self.function_table
                                .insert(name.clone(), (params.clone(), ret.clone()));
                        } else {
                            println!("[WARNING] [import_module] Function '{}' not found in imported_analyzer.function_table", name);
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
        Ok(())
    }
}
