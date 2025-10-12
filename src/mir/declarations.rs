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

        // Handle different binding patterns for the left-hand side.
        match pattern {
            Pattern::Identifier(name) => {
                instrs.push(MirInstr::Assign {
                    name: name.clone(),
                    value: value_tmp,
                    mutable: *mutable,
                });

                // Insert IncRef if this value needs reference counting.
                if needs_rc {
                    instrs.push(MirInstr::IncRef {
                        value: name.clone(),
                    });
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
/// - Ensures DecRef instructions are inserted before return for proper memory management.
/// - Adds an implicit return if none is present and the function has no return type.
pub fn build_function_decl(builder: &mut MirBuilder, node: &AstNode) -> MirFunction {
    if let AstNode::FunctionDecl {
        name,
        params,
        return_type,
        body,
        ..
    } = node
    {
        let mut func = MirFunction {
            name: name.clone(),
            params: params.iter().map(|(n, _)| n.clone()).collect(),
            return_type: return_type.as_ref().map(|t| format!("{:?}", t)),
            blocks: vec![],
        };

        let block_label = builder.next_block();
        let mut block = MirBlock {
            label: block_label,
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

        // Exit scope: insert DecRef for all tracked variables before return.
        builder.exit_scope(&mut block);

        // If no explicit return and function has no return type, add implicit return.
        if block.terminator.is_none() && return_type.is_none() {
            block.terminator = Some(MirInstr::Return { values: vec![] });
        }

        func.blocks.push(block);
        func
    } else {
        panic!("Expected FunctionDecl node");
    }
}

/// Helper function to build MIR instructions for complex nested collections (arrays/maps).
/// - Recursively builds MIR for nested arrays and maps.
/// - Handles arrays containing maps, maps containing arrays, and deeply nested structures.
/// - Returns the temporary variable name holding the constructed collection.
/// Note: Nested is not supported in codegen yet
// TODO: Implement nested collection support in codegen
pub fn build_nested_collection(
    builder: &mut MirBuilder,
    expr: &AstNode,
    block: &mut MirBlock,
) -> String {
    match expr {
        AstNode::ArrayLiteral(elements) => {
            let mut element_tmps = vec![];
            for elem in elements {
                let elem_tmp = match elem {
                    AstNode::ArrayLiteral(_) => {
                        // Recursively build nested array.
                        build_nested_collection(builder, elem, block)
                    }
                    AstNode::MapLiteral(_) => {
                        // Recursively build array containing maps.
                        build_nested_collection(builder, elem, block)
                    }
                    _ => {
                        // Build MIR for regular expression element.
                        build_expression(builder, elem, block)
                    }
                };
                element_tmps.push(elem_tmp);
            }

            // Create MIR instruction for array construction.
            let array_tmp = builder.next_tmp();
            block.instrs.push(MirInstr::Array {
                name: array_tmp.clone(),
                elements: element_tmps,
            });
            array_tmp
        }

        AstNode::MapLiteral(entries) => {
            let mut map_entries = vec![];
            for (key_expr, val_expr) in entries {
                let key_tmp = build_expression(builder, key_expr, block);
                let val_tmp = match val_expr {
                    AstNode::ArrayLiteral(_) | AstNode::MapLiteral(_) => {
                        // Recursively build nested collection as map value.
                        build_nested_collection(builder, val_expr, block)
                    }
                    _ => build_expression(builder, val_expr, block),
                };
                map_entries.push((key_tmp, val_tmp));
            }

            // Create MIR instruction for map construction.
            let map_tmp = builder.next_tmp();
            block.instrs.push(MirInstr::Map {
                name: map_tmp.clone(),
                entries: map_entries,
            });
            map_tmp
        }

        _ => {
            // Fallback: build MIR for regular expression.
            build_expression(builder, expr, block)
        }
    }
}
