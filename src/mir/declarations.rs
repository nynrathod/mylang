use crate::mir::builder::MirBuilder;
use crate::mir::expresssions::build_expression;
use crate::mir::statements::build_statement;
use crate::mir::{MirBlock, MirFunction, MirInstr};
use crate::parser::ast::TypeNode;
use crate::parser::ast::{AstNode, Pattern};

/// Build MIR instructions for a variable declaration (`let` statement).
/// - Handles single variable and tuple destructuring patterns.
/// - Evaluates the right-hand side expression and assigns it to the variable(s).
/// - Inserts reference counting instructions for heap-allocated types (strings, arrays, maps).
pub fn build_let_decl(builder: &mut MirBuilder, node: &AstNode) -> Vec<MirInstr> {
    if let AstNode::LetDecl {
        pattern,
        value,
        mutable,
        type_annotation,
        is_ref_counted,
        ..
    } = node
    {
        let mut instrs = vec![];
        // Create a temporary block to evaluate the right-hand side expression.
        let mut temp_block = MirBlock {
            label: "temp".to_string(),
            instrs: vec![],
            terminator: None,
        };

        // Build MIR for the value expression.
        let value_tmp = build_expression(builder, value, &mut temp_block);

        // Add the expression evaluation instructions to our result.
        instrs.extend(temp_block.instrs);

        // Determine if reference counting is needed for this variable.
        let needs_rc = match type_annotation {
            Some(TypeNode::String) => true,
            Some(TypeNode::Array(_)) => true,
            Some(TypeNode::Map(_, _)) => true,
            _ => false,
        };

        // Check if value_tmp is a simple variable identifier (not a temp or literal).
        // We only need to incref when COPYING from an existing variable.
        // Temps starting with '%' are newly created values (from ConstString, Array, Map, etc.)
        // that already have RC=1, so we shouldn't incref them.
        let is_copying_variable = !value_tmp.starts_with('%')
            && !value_tmp.parse::<i32>().is_ok()
            && value_tmp != "true"
            && value_tmp != "false";

        // Handle different binding patterns for the left-hand side.
        match pattern {
            Pattern::Identifier(name) => {
                instrs.push(MirInstr::Assign {
                    name: name.clone(),
                    value: value_tmp.clone(),
                    mutable: *mutable,
                });

                // Insert IncRef ONLY when copying from an existing variable.
                // Don't incref for newly created temps (they already have RC=1).
                if needs_rc && is_copying_variable {
                    instrs.push(MirInstr::IncRef {
                        value: name.clone(),
                    });
                }

                // Always track RC variables for cleanup at scope end
                if needs_rc {
                    builder.track_rc_var(name.clone());
                }
            }
            Pattern::Tuple(patterns) => {
                for (i, pattern) in patterns.iter().enumerate() {
                    if let Pattern::Identifier(name) = pattern {
                        // Extract each tuple element into a temporary.
                        let extract_tmp = builder.next_tmp();
                        instrs.push(MirInstr::TupleExtract {
                            name: extract_tmp.clone(),
                            source: value_tmp.clone(),
                            index: i,
                        });
                        instrs.push(MirInstr::Assign {
                            name: name.clone(),
                            value: extract_tmp,
                            mutable: *mutable,
                        });

                        // Reference counting for tuple elements if needed.
                        if is_ref_counted.unwrap_or(false) {
                            instrs.push(MirInstr::IncRef {
                                value: name.clone(),
                            });
                        }
                    }
                }
            }
            _ => {
                // Handle other patterns (e.g., struct destructuring) in the future.
            }
        }

        instrs
    } else {
        vec![]
    }
}

/// Build MIR instructions for a function declaration.
/// - Sets up a new MIR function with parameters and return type.
/// - Tracks reference-counted variables in function scope (but NOT parameters).
/// - Maps function arguments to temporaries and assigns them to parameter names.
/// - Builds MIR for each statement in the function body.
/// - Ensures entry block jumps to loops if they exist (not returns immediately).
/// - Adds DecRef cleanup to the final reachable block only (no duplicates).
/// - Parameters are NOT tracked for RC cleanup since caller owns them.
/// - Adds an implicit return if none is present and the function has no return type.
pub fn build_function_decl(builder: &mut MirBuilder, node: &AstNode) {
    if let AstNode::FunctionDecl {
        name,
        params,
        return_type,
        body,
        ..
    } = node
    {
        let func = MirFunction {
            name: name.clone(),
            params: params.iter().map(|(n, _)| n.clone()).collect(),
            param_types: params
                .iter()
                .map(|(_, t)| t.as_ref().map(|ty| format!("{:?}", ty)))
                .collect(),
            return_type: return_type.as_ref().map(|t| format!("{:?}", t)),
            blocks: vec![],
        };

        // Add function to program BEFORE processing body
        // This ensures that when build_statement adds blocks for loops,
        // it adds them to THIS function (via last_mut())
        builder.program.functions.push(func);

        let entry_label = builder.next_block();
        let mut block = MirBlock {
            label: entry_label.clone(),
            instrs: vec![],
            terminator: None,
        };

        // Enter function scope for reference counting.
        builder.enter_scope();

        // Track parameter names and types to check if they need RC
        let mut param_rc_types: Vec<(String, bool)> = Vec::new();

        // Parameters are handled directly by codegen (allocated and stored from function args)
        // No need for Arg instructions or intermediate temps
        // Just track which parameters need RC for potential future use
        for (param_name, param_type) in params {
            // Check if parameter is RC type (String, Array, Map)
            let is_rc = match param_type {
                Some(TypeNode::String) => true,
                Some(TypeNode::Array(_)) => true,
                Some(TypeNode::Map(_, _)) => true,
                _ => false,
            };

            param_rc_types.push((param_name.clone(), is_rc));

            // DO NOT track parameters as RC variables for cleanup
            // Parameters are owned by the caller, not by this function
            // The function borrows them, and caller handles cleanup
        }

        // Track if we've already pushed the final block (to avoid duplicate insertion)
        let mut final_block_pushed = false;

        // Build MIR for each statement in the function body.
        for stmt in body {
            build_statement(builder, stmt, &mut block);

            // If the statement set a terminator (like a for-loop), subsequent statements
            // need a new block to avoid adding instructions after the terminator
            if block.terminator.is_some() {
                // Save the current block label before adding it
                let current_block_label = block.label.clone();

                // Add the current block to the function
                if let Some(current_func) = builder.program.functions.last_mut() {
                    current_func.blocks.push(block.clone());
                    final_block_pushed = true;
                }

                // Create a new block for the next statement
                let next_label = builder.next_block();
                let next_block_label = next_label.clone();
                block = MirBlock {
                    label: next_label,
                    instrs: vec![],
                    terminator: None,
                };
                final_block_pushed = false;

                // Don't connect loop exit blocks to continuation blocks
                // Let them remain without terminators so they can get return statements added
                // Only connect if there are actually more statements coming
            }
        }

        // Check if there are loop blocks BEFORE we do anything else
        // Loop blocks are added by build_statement to the function
        let has_loop_blocks = if let Some(func) = builder.program.functions.last() {
            !func.blocks.is_empty() // If blocks already exist, they are loop blocks
        } else {
            false
        };

        // Don't set entry terminator yet for loops - we'll do it after insertion
        if !has_loop_blocks {
            // No loops - entry is the only block
            // Add return only if function has no explicit return type
            if return_type.is_none() && block.terminator.is_none() {
                // Don't add return yet - we'll add cleanup first, then return
            }
        }

        // Insert the entry block at the BEGINNING of the function's blocks
        // Only if it wasn't already pushed (to avoid duplicates)
        if !final_block_pushed {
            if let Some(func) = builder.program.functions.last_mut() {
                func.blocks.insert(0, block);

                // NOW find the init block and make entry jump to it
                // Init block should have stores to loop variables (i, i_end, etc.)
                if has_loop_blocks && func.blocks.len() > 1 {
                    // Find the first block that has Store instructions (the init block)
                    let mut init_label = None;
                    for b in func.blocks.iter().skip(1) {
                        // Init block has assigns/stores but no loads or comparisons
                        let has_stores = b
                            .instrs
                            .iter()
                            .any(|instr| matches!(instr, MirInstr::Assign { .. }));
                        let has_ops = b
                            .instrs
                            .iter()
                            .any(|instr| matches!(instr, MirInstr::BinaryOp(..)));
                        if has_stores && !has_ops {
                            init_label = Some(b.label.clone());
                            break;
                        }
                    }

                    if let Some(target) = init_label {
                        func.blocks[0].terminator = Some(MirInstr::Jump { target });
                    } else {
                        // Fallback: jump to first block after entry
                        let first_block = func.blocks[1].label.clone();
                        func.blocks[0].terminator = Some(MirInstr::Jump {
                            target: first_block,
                        });
                    }
                }
            }
        } else {
            // Still need to set entry block jump for loops even if it was already pushed
            if has_loop_blocks {
                if let Some(func) = builder.program.functions.last_mut() {
                    // Find init block (has stores but no binary ops)
                    let mut init_label = None;
                    for b in func.blocks.iter() {
                        let has_stores = b
                            .instrs
                            .iter()
                            .any(|instr| matches!(instr, MirInstr::Assign { .. }));
                        let has_ops = b
                            .instrs
                            .iter()
                            .any(|instr| matches!(instr, MirInstr::BinaryOp(..)));
                        if has_stores && !has_ops {
                            init_label = Some(b.label.clone());
                            break;
                        }
                    }

                    if let Some(target) = init_label {
                        eprintln!(
                            "[FIX] Function '{}': Setting already-pushed entry '{}' to jump to init '{}'",
                            name, entry_label, target
                        );
                        // Find the entry block and set its terminator
                        for b in func.blocks.iter_mut() {
                            if b.label == entry_label {
                                b.terminator = Some(MirInstr::Jump { target });
                                break;
                            }
                        }
                    }
                }
            }
        }

        // NOW get cleanup instructions from exit_scope
        let mut temp_block = MirBlock {
            label: "temp_cleanup".to_string(),
            instrs: vec![],
            terminator: None,
        };
        builder.exit_scope(&mut temp_block);
        let decref_instrs = temp_block.instrs;

        // Add cleanup to the appropriate block (ONLY ONCE)
        if has_loop_blocks {
            // Add cleanup and return to ALL blocks without terminators
            // (these are the blocks where execution could end)
            if let Some(func) = builder.program.functions.last_mut() {
                // Collect blocks that need cleanup (no terminator, not entry)
                let blocks_needing_cleanup: Vec<String> = func
                    .blocks
                    .iter()
                    .filter(|b| b.terminator.is_none() && b.label != entry_label)
                    .map(|b| b.label.clone())
                    .collect();

                for block_label in blocks_needing_cleanup {
                    if let Some(final_block) =
                        func.blocks.iter_mut().find(|b| b.label == block_label)
                    {
                        // Add decrefs to this block
                        for decref_instr in &decref_instrs {
                            final_block.instrs.push(decref_instr.clone());
                        }

                        // Only add return if function is void
                        if return_type.is_none() {
                            final_block.terminator = Some(MirInstr::Return { values: vec![] });
                        } else {
                        }
                    }
                }
            }
        } else {
            // No loops - add cleanup to entry block (the only block)
            if let Some(func) = builder.program.functions.last_mut() {
                if let Some(entry_block) = func.blocks.first_mut() {
                    // Add decrefs to entry block
                    for decref_instr in decref_instrs {
                        entry_block.instrs.push(decref_instr);
                    }

                    // Add return if needed
                    if return_type.is_none() && entry_block.terminator.is_none() {
                        entry_block.terminator = Some(MirInstr::Return { values: vec![] });
                    }
                }
            }
        }
    } else {
        panic!("Expected FunctionDecl node");
    }
}

/// Helper function to build MIR instructions for nested collections.
/// NOTE: Nested collections are NOT supported for production.
/// This function exists for future extension but should not be used.
/// Regular arrays and maps work fine, but nested structures (array of arrays, etc.) are not implemented.
#[allow(dead_code)]
pub fn build_nested_collection(
    builder: &mut MirBuilder,
    expr: &AstNode,
    block: &mut MirBlock,
) -> String {
    // For now, just fall back to regular expression building
    // Nested collections are not supported
    build_expression(builder, expr, block)
}
