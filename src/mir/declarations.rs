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
/// - Tracks reference-counted variables in function scope.
/// - Maps function arguments to temporaries and assigns them to parameter names.
/// - Builds MIR for each statement in the function body.
/// - Ensures entry block jumps to loops if they exist (not returns immediately).
/// - Adds DecRef cleanup to the final reachable block only (no duplicates).
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

        // Add arguments: map parameter names to temporaries and assign.
        for (param_name, _) in params {
            let tmp = builder.next_tmp();
            block.instrs.push(MirInstr::Arg { name: tmp.clone() });

            // Assign argument to parameter name (parameters are immutable).
            block.instrs.push(MirInstr::Assign {
                name: param_name.clone(),
                value: tmp,
                mutable: false,
            });
        }

        // Build MIR for each statement in the function body.
        for stmt in body {
            build_statement(builder, stmt, &mut block);
        }

        // Check if there are loop blocks BEFORE we do anything else
        // Loop blocks are added by build_statement to the function
        let has_loop_blocks = if let Some(func) = builder.program.functions.last() {
            !func.blocks.is_empty() // If blocks already exist, they are loop blocks
        } else {
            false
        };

        eprintln!(
            "[DEBUG] Function '{}': After building statements, has_loop_blocks = {}, num_blocks = {}",
            name,
            has_loop_blocks,
            builder.program.functions.last().map(|f| f.blocks.len()).unwrap_or(0)
        );

        // Set entry block terminator BEFORE inserting it
        if has_loop_blocks {
            // There are loop blocks - entry MUST jump to the first loop block
            if let Some(func) = builder.program.functions.last() {
                let first_loop_label = func.blocks.first().map(|b| b.label.clone());

                if let Some(target) = first_loop_label {
                    eprintln!(
                        "[FIX] Function '{}': Setting entry block '{}' to jump to first loop '{}'",
                        name, entry_label, target
                    );
                    block.terminator = Some(MirInstr::Jump {
                        target: target.clone(),
                    });
                } else {
                    eprintln!(
                        "[ERROR] Function '{}': has_loop_blocks=true but no blocks found!",
                        name
                    );
                }
            }
        } else {
            // No loops - entry is the only block
            // Add return only if function has no explicit return type
            if return_type.is_none() && block.terminator.is_none() {
                eprintln!(
                    "[DEBUG] Function '{}': No loops, will add return to entry block after cleanup",
                    name
                );
                // Don't add return yet - we'll add cleanup first, then return
            }
        }

        // Insert the entry block at the BEGINNING of the function's blocks
        if let Some(func) = builder.program.functions.last_mut() {
            func.blocks.insert(0, block);
            eprintln!(
                "[DEBUG] Function '{}': Inserted entry block, total blocks = {}",
                name,
                func.blocks.len()
            );
        }

        // NOW get cleanup instructions from exit_scope
        let mut temp_block = MirBlock {
            label: "temp_cleanup".to_string(),
            instrs: vec![],
            terminator: None,
        };
        builder.exit_scope(&mut temp_block);
        let decref_instrs = temp_block.instrs;

        eprintln!(
            "[DEBUG] Function '{}': Generated {} DecRef instructions for cleanup",
            name,
            decref_instrs.len()
        );

        // Add cleanup to the appropriate block (ONLY ONCE)
        if has_loop_blocks {
            // Add cleanup to the LAST block (after all loops complete)
            if let Some(func) = builder.program.functions.last_mut() {
                let last_block_label = func.blocks.last().map(|b| b.label.clone());

                if let Some(label) = last_block_label {
                    // Find the block with this label and add cleanup
                    if let Some(last_block) = func.blocks.iter_mut().find(|b| b.label == label) {
                        eprintln!(
                            "[DEBUG] Function '{}': Adding RC cleanup to final block '{}'",
                            name, label
                        );

                        // Add decrefs to the last block
                        for decref_instr in decref_instrs {
                            last_block.instrs.push(decref_instr);
                        }

                        // Add return to the last block if it doesn't have a terminator
                        if last_block.terminator.is_none() {
                            last_block.terminator = Some(MirInstr::Return { values: vec![] });
                            eprintln!(
                                "[DEBUG] Function '{}': Added return to final block '{}'",
                                name, label
                            );
                        }
                    }
                }
            }
        } else {
            // No loops - add cleanup to entry block (the only block)
            if let Some(func) = builder.program.functions.last_mut() {
                if let Some(entry_block) = func.blocks.first_mut() {
                    eprintln!(
                        "[DEBUG] Function '{}': Adding RC cleanup to entry block (no loops)",
                        name
                    );

                    // Add decrefs to entry block
                    for decref_instr in decref_instrs {
                        entry_block.instrs.push(decref_instr);
                    }

                    // Add return if needed
                    if return_type.is_none() && entry_block.terminator.is_none() {
                        entry_block.terminator = Some(MirInstr::Return { values: vec![] });
                        eprintln!("[DEBUG] Function '{}': Added return to entry block", name);
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
