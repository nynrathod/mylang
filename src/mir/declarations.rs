use crate::mir::builder::MirBuilder;
use crate::mir::expresssions::build_expression;
use crate::mir::statements::build_statement;
use crate::mir::{MirBlock, MirFunction, MirInstr};
use crate::parser::ast::TypeNode;
use crate::parser::ast::{AstNode, Pattern};

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

        // ENTER FUNCTION SCOPE - track RC variables
        builder.enter_scope();

        // Add arguments - map parameter names to temporaries
        for (param_name, _) in params {
            let tmp = builder.next_tmp();
            block.instrs.push(MirInstr::Arg { name: tmp.clone() });

            // Assign argument to parameter name
            block.instrs.push(MirInstr::Assign {
                name: param_name.clone(),
                value: tmp,
                mutable: false, // Parameters are immutable by default
            });
        }

        // Build body statements
        for stmt in body {
            build_statement(builder, stmt, &mut block);
        }

        // EXIT SCOPE - insert DecRef for all tracked variables BEFORE return
        builder.exit_scope(&mut block);

        // If no explicit return and function has no return type, add implicit return
        if block.terminator.is_none() && return_type.is_none() {
            block.terminator = Some(MirInstr::Return { values: vec![] });
        }

        func.blocks.push(block);
        func
    } else {
        panic!("Expected FunctionDecl node");
    }
}

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

        // Create a temporary block to evaluate the expression
        let mut temp_block = MirBlock {
            label: "temp".to_string(),
            instrs: vec![],
            terminator: None,
        };

        // Use the enhanced expression builder
        let value_tmp = build_expression(builder, value, &mut temp_block);

        // Add the expression evaluation instructions to our result
        instrs.extend(temp_block.instrs);

        let needs_rc = match type_annotation {
            Some(TypeNode::String) => true,
            Some(TypeNode::Array(_)) => true,
            Some(TypeNode::Map(_, _)) => true,
            _ => false,
        };

        // Handle different binding patterns
        match pattern {
            Pattern::Identifier(name) => {
                instrs.push(MirInstr::Assign {
                    name: name.clone(),
                    value: value_tmp,
                    mutable: *mutable,
                });

                let is_copy = matches!(&**value, AstNode::Identifier(_));
                // Insert IncRef if this value needs RC
                if needs_rc {
                    instrs.push(MirInstr::IncRef {
                        value: name.clone(),
                    });
                    builder.track_rc_var(name.clone());
                }
            }
            Pattern::Tuple(patterns) => {
                // Handle tuple destructuring: let (x, y, z) = func();
                for (i, pattern) in patterns.iter().enumerate() {
                    if let Pattern::Identifier(name) = pattern {
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

                        // RC for tuple elements (check if needed)
                        if is_ref_counted.unwrap_or(false) {
                            instrs.push(MirInstr::IncRef {
                                value: name.clone(),
                            });
                        }
                    }
                }
            }
            _ => {
                // Handle other patterns (e.g., struct destructuring) in the future
            }
        }

        instrs
    } else {
        vec![]
    }
}

// Helper function to handle complex nested collections
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
                        // Nested array
                        build_nested_collection(builder, elem, block)
                    }
                    AstNode::MapLiteral(_) => {
                        // Array containing maps
                        build_nested_collection(builder, elem, block)
                    }
                    _ => {
                        // Regular expression
                        build_expression(builder, elem, block)
                    }
                };
                element_tmps.push(elem_tmp);
            }

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
                        // Nested collection as map value
                        build_nested_collection(builder, val_expr, block)
                    }
                    _ => build_expression(builder, val_expr, block),
                };
                map_entries.push((key_tmp, val_tmp));
            }

            let map_tmp = builder.next_tmp();
            block.instrs.push(MirInstr::Map {
                name: map_tmp.clone(),
                entries: map_entries,
            });
            map_tmp
        }

        _ => {
            // Fallback to regular expression building
            build_expression(builder, expr, block)
        }
    }
}
