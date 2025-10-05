use crate::mir::declarations::{build_function_decl, build_let_decl, build_nested_collection};

use crate::mir::expresssions::build_expression;
use crate::mir::{MirBlock, MirFunction, MirInstr, MirProgram};
use crate::parser::ast::{AstNode, Pattern};

/// Main MIR builder - converts AST to MIR representation
pub struct MirBuilder {
    pub program: MirProgram,
    pub tmp_counter: usize,                // For generating unique temporaries
    pub block_counter: usize,              // For generating unique block labels
    pub loop_stack: Vec<LoopContext>,      // Track nested loops for break/continue
    pub rc_tracked_vars: Vec<Vec<String>>, // Stack of scopes with RC'd vars
}

/// Context for tracking loop break/continue targets
#[derive(Debug, Clone)]
pub struct LoopContext {
    pub break_target: String,    // Where break jumps to
    pub continue_target: String, // Where continue jumps to
}

impl MirBuilder {
    pub fn new() -> Self {
        Self {
            program: MirProgram {
                functions: vec![],
                globals: vec![],
            },
            tmp_counter: 1,
            block_counter: 0,
            loop_stack: vec![],
            rc_tracked_vars: vec![vec![]],
        }
    }

    /// Generate unique temporary variable name
    pub fn next_tmp(&mut self) -> String {
        let tmp = format!("%{}", self.tmp_counter);
        self.tmp_counter += 1;
        tmp
    }

    /// Generate unique basic block label
    pub fn next_block(&mut self) -> String {
        let label = format!("Block{}", self.block_counter);
        self.block_counter += 1;
        label
    }

    pub fn enter_loop(&mut self, break_target: String, continue_target: String) {
        self.loop_stack.push(LoopContext {
            break_target,
            continue_target,
        });
    }

    pub fn exit_loop(&mut self) {
        self.loop_stack.pop();
    }

    pub fn current_loop(&self) -> Option<&LoopContext> {
        self.loop_stack.last()
    }

    pub fn enter_scope(&mut self) {
        self.rc_tracked_vars.push(vec![]);
    }

    pub fn exit_scope(&mut self, block: &mut MirBlock) {
        if let Some(scope_vars) = self.rc_tracked_vars.pop() {
            // Insert DecRef for all RC'd variables in this scope
            for var in scope_vars.iter().rev() {
                block.instrs.push(MirInstr::DecRef { value: var.clone() });
            }
        }
    }

    pub fn track_rc_var(&mut self, var: String) {
        if let Some(current_scope) = self.rc_tracked_vars.last_mut() {
            current_scope.push(var);
        }
    }
    pub fn is_rc_tracked(&self, var: &str) -> bool {
        self.rc_tracked_vars
            .iter()
            .any(|scope| scope.contains(&var.to_string()))
    }

    pub fn build_program(&mut self, nodes: &[AstNode]) {
        for node in nodes {
            match node {
                AstNode::FunctionDecl { .. } => {
                    let func = build_function_decl(self, node);
                    self.program.functions.push(func);
                }

                AstNode::LetDecl { .. } => {
                    let instrs = build_let_decl(self, node);
                    self.program.globals.extend(instrs);
                }

                // Handle struct declarations (definitions, not instances)
                AstNode::StructDecl { name, fields } => {
                    // For struct declarations, we might want to store type information
                    // For now, we'll create a placeholder instance to show structure
                    let tmp = self.next_tmp();
                    let field_vals: Vec<(String, String)> = fields
                        .iter()
                        .map(|(fname, _typ)| {
                            let val_tmp = self.next_tmp();
                            (fname.clone(), val_tmp)
                        })
                        .collect();

                    self.program.globals.push(MirInstr::StructInit {
                        name: tmp,
                        struct_name: name.clone(),
                        fields: field_vals,
                    });
                }

                // Handle enum declarations (definitions, not instances)
                AstNode::EnumDecl { name, variants } => {
                    // Create instances of each variant for demonstration
                    for (variant_name, opt_type) in variants {
                        let tmp = self.next_tmp();
                        let value_tmp = if opt_type.is_some() {
                            Some(self.next_tmp())
                        } else {
                            None
                        };

                        self.program.globals.push(MirInstr::EnumInit {
                            name: tmp,
                            enum_name: name.clone(),
                            variant: variant_name.clone(),
                            value: value_tmp,
                        });
                    }
                }

                // Handle struct literals (instances)
                AstNode::StringLiteral(value) => {
                    let tmp = self.next_tmp();

                    // Add a MIR instruction for string allocation / constant
                    self.program.globals.push(MirInstr::ConstString {
                        name: tmp.clone(),
                        value: value.clone(),
                    });

                    // Assign it to a global variable (optional, if you want globals to hold it)
                    self.program.globals.push(MirInstr::Assign {
                        name: format!("global_str_{}", self.tmp_counter - 1),
                        value: tmp,
                        mutable: false,
                    });
                }

                // Handle enum variant literals (instances)
                AstNode::EnumDecl { name, variants } => {
                    for (variant_name, opt_type) in variants {
                        let tmp = self.next_tmp();

                        // For now, just create a placeholder MIR value if the variant has a type
                        let value_tmp = if opt_type.is_some() {
                            Some(self.next_tmp())
                        } else {
                            None
                        };

                        // Push a MIR instruction for this enum variant
                        self.program.globals.push(MirInstr::EnumInit {
                            name: tmp.clone(),
                            enum_name: name.clone(),
                            variant: variant_name.clone(),
                            value: value_tmp,
                        });

                        // Optional: assign the enum variant to a global variable
                        self.program.globals.push(MirInstr::Assign {
                            name: format!("global_enum_{}_{}", name, variant_name),
                            value: tmp,
                            mutable: false,
                        });
                    }
                }

                // Handle global assignments
                AstNode::Assignment { pattern, value } => {
                    let mut temp_block = MirBlock {
                        label: "temp".to_string(),
                        instrs: vec![],
                        terminator: None,
                    };

                    // Generate MIR for the value expression
                    let value_tmp = build_expression(self, value, &mut temp_block);
                    self.program.globals.extend(temp_block.instrs);

                    // Only handle simple identifier patterns for globals
                    if let Pattern::Identifier(name) = pattern {
                        self.program.globals.push(MirInstr::Assign {
                            name: name.clone(),
                            value: value_tmp,
                            mutable: true,
                        });
                    }
                }

                // Handle global print statements
                AstNode::Print { exprs } => {
                    let mut temp_block = MirBlock {
                        label: "temp".to_string(),
                        instrs: vec![],
                        terminator: None,
                    };

                    let mut print_vals = vec![];
                    for expr in exprs {
                        let val_tmp = build_expression(self, expr, &mut temp_block);
                        print_vals.push(val_tmp);
                    }

                    self.program.globals.extend(temp_block.instrs);
                    self.program
                        .globals
                        .push(MirInstr::Print { values: print_vals });
                }

                // Handle for loops at global level (rare but possible)
                AstNode::ForLoopStmt { .. } => {
                    // Create a temporary function to contain the loop
                    let loop_func_name = self.create_temp_function("loop");
                    let mut temp_func = MirFunction {
                        name: loop_func_name.clone(),
                        params: vec![],
                        return_type: None,
                        blocks: vec![],
                    };

                    let block_label = self.next_block();
                    let mut block = MirBlock {
                        label: block_label,
                        instrs: vec![],
                        terminator: None,
                    };

                    // Build the for loop in the temporary function
                    crate::mir::statements::build_statement(self, node, &mut block);
                    temp_func.blocks.push(block);
                    self.program.functions.push(temp_func);

                    // Call the loop function from globals
                    let call_tmp = self.next_tmp();
                    self.program.globals.push(MirInstr::Call {
                        dest: vec![call_tmp],
                        func: loop_func_name,
                        args: vec![],
                    });
                }

                // Handle if statements at global level
                AstNode::ConditionalStmt { .. } => {
                    // Similar to for loops, wrap in a temporary function
                    let if_func_name = self.create_temp_function("if");
                    let mut temp_func = MirFunction {
                        name: if_func_name.clone(),
                        params: vec![],
                        return_type: None,
                        blocks: vec![],
                    };

                    let block_label = self.next_block();
                    let mut block = MirBlock {
                        label: block_label,
                        instrs: vec![],
                        terminator: None,
                    };

                    crate::mir::statements::build_statement(self, node, &mut block);
                    temp_func.blocks.push(block);
                    self.program.functions.push(temp_func);

                    let call_tmp = self.next_tmp();
                    self.program.globals.push(MirInstr::Call {
                        dest: vec![call_tmp],
                        func: if_func_name,
                        args: vec![],
                    });
                }

                // Handle complex expressions at global level
                AstNode::BinaryExpr { .. } | AstNode::FunctionCall { .. } => {
                    let mut temp_block = MirBlock {
                        label: "temp".to_string(),
                        instrs: vec![],
                        terminator: None,
                    };

                    let expr_tmp = build_expression(self, node, &mut temp_block);
                    self.program.globals.extend(temp_block.instrs);

                    // Assign the result to a global variable for reference
                    let global_name = format!("__global_expr_{}", self.tmp_counter - 1);
                    self.program.globals.push(MirInstr::Assign {
                        name: global_name,
                        value: expr_tmp,
                        mutable: false,
                    });
                }

                // Handle array and map literals at global level
                AstNode::ArrayLiteral { .. } | AstNode::MapLiteral { .. } => {
                    let mut temp_block = MirBlock {
                        label: "temp".to_string(),
                        instrs: vec![],
                        terminator: None,
                    };

                    let collection_tmp = build_nested_collection(self, node, &mut temp_block);
                    self.program.globals.extend(temp_block.instrs);

                    let global_name = format!("__global_collection_{}", self.tmp_counter - 1);
                    self.program.globals.push(MirInstr::Assign {
                        name: global_name,
                        value: collection_tmp,
                        mutable: false,
                    });
                }

                _ => {
                    // For unhandled node types, create a placeholder
                    println!(
                        "Warning: Unhandled AST node type in global scope: {:?}",
                        std::mem::discriminant(node)
                    );
                }
            }
        }

        let global_scope_vars = self.rc_tracked_vars.last().cloned().unwrap_or_default();
        for var in global_scope_vars.iter().rev() {
            self.program
                .globals
                .push(MirInstr::DecRef { value: var.clone() });
        }
    }

    // Helper method to create a temporary function for complex global constructs
    fn create_temp_function(&mut self, name_prefix: &str) -> String {
        let func_name = format!(
            "__{}_{}_{}",
            name_prefix, self.block_counter, self.tmp_counter
        );
        func_name
    }

    /// Finalize the MIR program: clean up and lightweight optimizations
    pub fn finalize(&mut self) {
        use std::collections::HashSet;

        // 1. Remove empty blocks (blocks without instructions and no terminator)
        for func in &mut self.program.functions {
            func.blocks
                .retain(|block| !block.instrs.is_empty() || block.terminator.is_some());
        }

        // 2. Deduplicate global constants / assignments by name
        //    Ensures multiple identical constants are not emitted repeatedly
        let mut seen_assigns: HashSet<String> = HashSet::new();
        self.program.globals.retain(|instr| {
            match instr {
                MirInstr::Assign { name, .. }
                | MirInstr::ConstString { name, .. }
                | MirInstr::StructInit { name, .. }
                | MirInstr::EnumInit { name, .. } => {
                    if seen_assigns.contains(name) {
                        false // already seen, remove duplicate
                    } else {
                        seen_assigns.insert(name.clone());
                        true
                    }
                }
                _ => true,
            }
        });

        // 3. Optional: merge consecutive constant assignments to same target
        //    (if MIR has %tmp1 = const, %tmp1 = const again, keep last)
        let mut last_assigns: HashSet<String> = HashSet::new();
        self.program.globals.retain(|instr| {
            if let MirInstr::Assign { name, .. } = instr {
                if last_assigns.contains(name) {
                    false
                } else {
                    last_assigns.insert(name.clone());
                    true
                }
            } else {
                true
            }
        });
    }
}
