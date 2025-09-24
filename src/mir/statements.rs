use crate::lexar::token::TokenType;
use crate::mir::builder::MirBuilder;
use crate::mir::expresssions::build_expression;
use crate::mir::{MirBlock, MirInstr};
use crate::parser::ast::{AstNode, Pattern};

/// Convert AST statements to MIR instructions
/// Handles statements, delegates complex control flow
pub fn build_statement(builder: &mut MirBuilder, stmt: &AstNode, block: &mut MirBlock) {
    match stmt {
        // Handle variable assignments
        AstNode::Assignment { pattern, value } => {
            let value_tmp = build_expression(builder, value, block);

            match pattern {
                Pattern::Identifier(name) => {
                    block.instrs.push(MirInstr::Assign {
                        name: name.clone(),
                        value: value_tmp,
                        mutable: true,
                    });
                }
                Pattern::Tuple(patterns) => {
                    for (i, pattern) in patterns.iter().enumerate() {
                        if let Pattern::Identifier(name) = pattern {
                            let extract_tmp = builder.next_tmp();
                            block.instrs.push(MirInstr::TupleExtract {
                                name: extract_tmp.clone(),
                                source: value_tmp.clone(),
                                index: i,
                            });
                            block.instrs.push(MirInstr::Assign {
                                name: name.clone(),
                                value: extract_tmp,
                                mutable: true,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        // Handle if/else statements
        AstNode::ConditionalStmt {
            condition,
            then_block,
            else_branch,
        } => {
            let cond_tmp = build_expression(builder, condition, block);

            let then_label = builder.next_block();
            let else_label = builder.next_block();
            let end_label = builder.next_block();

            block.terminator = Some(MirInstr::CondJump {
                cond: cond_tmp,
                then_block: then_label.clone(),
                else_block: if else_branch.is_some() {
                    else_label.clone()
                } else {
                    end_label.clone()
                },
            });

            let mut then_mir_block = MirBlock {
                label: then_label,
                instrs: vec![],
                terminator: Some(MirInstr::Jump {
                    target: end_label.clone(),
                }),
            };

            for stmt in then_block {
                build_statement(builder, stmt, &mut then_mir_block);
            }

            if let Some(else_stmt) = else_branch {
                let mut else_mir_block = MirBlock {
                    label: else_label,
                    instrs: vec![],
                    terminator: Some(MirInstr::Jump {
                        target: end_label.clone(),
                    }),
                };
                build_statement(builder, else_stmt, &mut else_mir_block);

                if let Some(current_func) = builder.program.functions.last_mut() {
                    current_func.blocks.push(then_mir_block);
                    current_func.blocks.push(else_mir_block);
                    current_func.blocks.push(MirBlock {
                        label: end_label,
                        instrs: vec![],
                        terminator: None,
                    });
                }
            } else {
                if let Some(current_func) = builder.program.functions.last_mut() {
                    current_func.blocks.push(then_mir_block);
                    current_func.blocks.push(MirBlock {
                        label: end_label,
                        instrs: vec![],
                        terminator: None,
                    });
                }
            }
        }

        AstNode::LetDecl {
            pattern,
            value,
            mutable,
            ..
        } => {
            let value_tmp = build_expression(builder, value, block);

            match pattern {
                Pattern::Identifier(name) => {
                    block.instrs.push(MirInstr::Assign {
                        name: name.clone(),
                        value: value_tmp,
                        mutable: *mutable,
                    });
                }
                Pattern::Tuple(patterns) => {
                    // Handle tuple destructuring in let statements
                    for (i, pattern) in patterns.iter().enumerate() {
                        if let Pattern::Identifier(name) = pattern {
                            let extract_tmp = builder.next_tmp();
                            block.instrs.push(MirInstr::TupleExtract {
                                name: extract_tmp.clone(),
                                source: value_tmp.clone(),
                                index: i,
                            });
                            block.instrs.push(MirInstr::Assign {
                                name: name.clone(),
                                value: extract_tmp,
                                mutable: *mutable,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        AstNode::Return { values } => {
            let mut ret_vals = vec![];
            for val in values {
                let ret_tmp = build_expression(builder, val, block);
                ret_vals.push(ret_tmp);
            }
            block.terminator = Some(MirInstr::Return { values: ret_vals });
        }

        // Handle standalone expressions (like function calls for their side effects)
        AstNode::BinaryExpr { .. } | AstNode::FunctionCall { .. } => {
            // Evaluate the expression but don't necessarily store the result
            build_expression(builder, stmt, block);
        }

        AstNode::Print { exprs } => {
            let mut vals = vec![];
            for expr in exprs {
                let val_tmp = build_expression(builder, expr, block);
                vals.push(val_tmp);
            }
            block.instrs.push(MirInstr::Print { values: vals });
        }

        // Enhanced for loop handling
        AstNode::Break => {
            if let Some(loop_ctx) = builder.current_loop() {
                block.terminator = Some(MirInstr::Jump {
                    target: loop_ctx.break_target.clone(),
                });
            } else {
                panic!("Break statement outside of loop");
            }
        }

        // Handle continue statement
        AstNode::Continue => {
            if let Some(loop_ctx) = builder.current_loop() {
                block.terminator = Some(MirInstr::Jump {
                    target: loop_ctx.continue_target.clone(),
                });
            } else {
                panic!("Continue statement outside of loop");
            }
        }

        // Enhanced for loop handling with break/continue support
        AstNode::ForLoopStmt {
            pattern,
            iterable,
            body,
        } => {
            if iterable.is_none() {
                let loop_header = builder.next_block();
                let loop_body = builder.next_block();
                let loop_end = builder.next_block();

                builder.enter_loop(loop_end.clone(), loop_header.clone());

                block.terminator = Some(MirInstr::Jump {
                    target: loop_header.clone(),
                });

                // Header block jumps directly to body
                let mut header_block = MirBlock {
                    label: loop_header.clone(),
                    instrs: vec![],
                    terminator: Some(MirInstr::Jump {
                        target: loop_body.clone(),
                    }),
                };

                // Body block executes statements, then jumps back to header
                let mut body_block = MirBlock {
                    label: loop_body.clone(),
                    instrs: vec![],
                    terminator: None,
                };
                for stmt in body {
                    build_statement(builder, stmt, &mut body_block);
                }
                if body_block.terminator.is_none() {
                    body_block.terminator = Some(MirInstr::Jump {
                        target: loop_header,
                    });
                }

                if let Some(func) = builder.program.functions.last_mut() {
                    func.blocks.push(header_block);
                    func.blocks.push(body_block);
                    func.blocks.push(MirBlock {
                        label: loop_end,
                        instrs: vec![],
                        terminator: None,
                    });
                }

                builder.exit_loop();
                return; // stop further processing
            }

            let loop_var = match pattern {
                Pattern::Identifier(name) => Some(name.clone()),
                Pattern::Wildcard => Some("_".to_string()), // <- handle wildcard here
                _ => Some(builder.next_tmp()),
            };

            let loop_header = builder.next_block();
            let loop_body = builder.next_block();
            let loop_increment = builder.next_block();
            let loop_end = builder.next_block();

            // Enter loop context (continue goes to increment, break goes to end)
            builder.enter_loop(loop_end.clone(), loop_increment.clone());

            let mut blocks_to_add = Vec::new();

            if let Some(iter_expr) = iterable {
                match iter_expr.as_ref() {
                    // Range-based loops: for i in 0..10
                    AstNode::BinaryExpr { left, op, right }
                        if matches!(op, TokenType::RangeExc | TokenType::RangeInc) =>
                    {
                        let loop_var = loop_var.expect("Loop variable required");

                        // Initialize loop variable
                        let start_tmp = build_expression(builder, left, block);
                        block.instrs.push(MirInstr::Assign {
                            name: loop_var.clone(),
                            value: start_tmp,
                            mutable: true,
                        });

                        let end_tmp = build_expression(builder, right, block);
                        block.terminator = Some(MirInstr::Jump {
                            target: loop_header.clone(),
                        });

                        // Header block: condition check
                        let mut header_block = MirBlock {
                            label: loop_header.clone(),
                            instrs: vec![],
                            terminator: None,
                        };

                        let cmp_tmp = builder.next_tmp();
                        let op_str = match op {
                            TokenType::RangeInc => "le",
                            TokenType::RangeExc => "lt",
                            _ => unreachable!(),
                        };

                        header_block.instrs.push(MirInstr::BinaryOp(
                            op_str.to_string(),
                            cmp_tmp.clone(),
                            loop_var.clone(),
                            end_tmp,
                        ));

                        header_block.terminator = Some(MirInstr::CondJump {
                            cond: cmp_tmp,
                            then_block: loop_body.clone(),
                            else_block: loop_end.clone(),
                        });

                        blocks_to_add.push(header_block);

                        // Body block: execute loop statements
                        let mut body_block = MirBlock {
                            label: loop_body.clone(),
                            instrs: vec![],
                            terminator: None,
                        };

                        // Build body statements (may contain break/continue)
                        for stmt in body {
                            // Check if this statement terminates the block
                            if body_block.terminator.is_some() {
                                // Statement after break/continue - create unreachable block
                                break;
                            }
                            build_statement(builder, stmt, &mut body_block);
                        }

                        // If no break/continue, jump to increment
                        if body_block.terminator.is_none() {
                            body_block.terminator = Some(MirInstr::Jump {
                                target: loop_increment.clone(),
                            });
                        }

                        blocks_to_add.push(body_block);

                        // Increment block: i = i + 1, then jump to header
                        let mut increment_block = MirBlock {
                            label: loop_increment,
                            instrs: vec![],
                            terminator: None,
                        };

                        let one_tmp = builder.next_tmp();
                        increment_block.instrs.push(MirInstr::ConstInt {
                            name: one_tmp.clone(),
                            value: 1,
                        });

                        let new_val_tmp = builder.next_tmp();
                        increment_block.instrs.push(MirInstr::BinaryOp(
                            "add".to_string(),
                            new_val_tmp.clone(),
                            loop_var.clone(),
                            one_tmp,
                        ));

                        increment_block.instrs.push(MirInstr::Assign {
                            name: loop_var,
                            value: new_val_tmp,
                            mutable: true,
                        });

                        increment_block.terminator = Some(MirInstr::Jump {
                            target: loop_header,
                        });

                        blocks_to_add.push(increment_block);

                        // End block
                        let end_block = MirBlock {
                            label: loop_end,
                            instrs: vec![],
                            terminator: None,
                        };

                        blocks_to_add.push(end_block);
                    }

                    // Array iteration with break/continue support
                    AstNode::Identifier(_) | AstNode::MapLiteral(_) => {
                        if let Some(loop_var) = &loop_var {
                            let iter_tmp = build_expression(builder, iter_expr, block);
                            let index_var = format!("{}__index", loop_var);

                            // Initialize index
                            let zero_tmp = builder.next_tmp();
                            block.instrs.push(MirInstr::ConstInt {
                                name: zero_tmp.clone(),
                                value: 0,
                            });
                            block.instrs.push(MirInstr::Assign {
                                name: index_var.clone(),
                                value: zero_tmp,
                                mutable: true,
                            });

                            block.terminator = Some(MirInstr::Jump {
                                target: loop_header.clone(),
                            });

                            // Header: bounds check
                            let mut header_block = MirBlock {
                                label: loop_header.clone(),
                                instrs: vec![],
                                terminator: None,
                            };

                            let len_tmp = builder.next_tmp();
                            header_block.instrs.push(MirInstr::ArrayLen {
                                name: len_tmp.clone(),
                                array: iter_tmp.clone(),
                            });

                            let cmp_tmp = builder.next_tmp();
                            header_block.instrs.push(MirInstr::BinaryOp(
                                "lt".to_string(),
                                cmp_tmp.clone(),
                                index_var.clone(),
                                len_tmp,
                            ));

                            header_block.terminator = Some(MirInstr::CondJump {
                                cond: cmp_tmp,
                                then_block: loop_body.clone(),
                                else_block: loop_end.clone(),
                            });

                            blocks_to_add.push(header_block);

                            // Body: extract element and execute statements
                            let mut body_block = MirBlock {
                                label: loop_body.clone(),
                                instrs: vec![],
                                terminator: None,
                            };

                            let elem_tmp = builder.next_tmp();
                            body_block.instrs.push(MirInstr::ArrayGet {
                                name: elem_tmp.clone(),
                                array: iter_tmp,
                                index: index_var.clone(),
                            });

                            body_block.instrs.push(MirInstr::Assign {
                                name: loop_var.clone(),
                                value: elem_tmp,
                                mutable: false,
                            });

                            // Build body statements
                            for stmt in body {
                                if body_block.terminator.is_some() {
                                    break;
                                }
                                build_statement(builder, stmt, &mut body_block);
                            }

                            if body_block.terminator.is_none() {
                                body_block.terminator = Some(MirInstr::Jump {
                                    target: loop_increment.clone(),
                                });
                            }

                            blocks_to_add.push(body_block);

                            // Increment: index++
                            let mut increment_block = MirBlock {
                                label: loop_increment,
                                instrs: vec![],
                                terminator: None,
                            };

                            let one_tmp = builder.next_tmp();
                            increment_block.instrs.push(MirInstr::ConstInt {
                                name: one_tmp.clone(),
                                value: 1,
                            });

                            let new_index_tmp = builder.next_tmp();
                            increment_block.instrs.push(MirInstr::BinaryOp(
                                "add".to_string(),
                                new_index_tmp.clone(),
                                index_var.clone(),
                                one_tmp,
                            ));

                            increment_block.instrs.push(MirInstr::Assign {
                                name: index_var,
                                value: new_index_tmp,
                                mutable: true,
                            });

                            increment_block.terminator = Some(MirInstr::Jump {
                                target: loop_header,
                            });

                            blocks_to_add.push(increment_block);

                            // End block
                            let end_block = MirBlock {
                                label: loop_end,
                                instrs: vec![],
                                terminator: None,
                            };

                            blocks_to_add.push(end_block);
                        }
                    }

                    _ => {
                        // Handle other cases
                    }
                }
            }

            // Add all blocks and exit loop context
            if let Some(current_func) = builder.program.functions.last_mut() {
                current_func.blocks.extend(blocks_to_add);
            }

            builder.exit_loop(); // Important: exit loop context
        }

        AstNode::StructDecl { name, fields } => {
            let tmp = builder.next_tmp();
            let field_vals: Vec<(String, String)> = fields
                .iter()
                .map(|(fname, _typ)| {
                    let val_tmp = builder.next_tmp();
                    (fname.clone(), val_tmp)
                })
                .collect();

            block.instrs.push(MirInstr::StructInit {
                name: tmp,
                struct_name: name.clone(),
                fields: field_vals,
            });
        }

        AstNode::EnumDecl { name, variants } => {
            for (variant_name, opt_type) in variants {
                let tmp = builder.next_tmp();
                let value_tmp = opt_type.as_ref().map(|_| builder.next_tmp());
                block.instrs.push(MirInstr::EnumInit {
                    name: tmp,
                    enum_name: name.clone(),
                    variant: variant_name.clone(),
                    value: value_tmp,
                });
            }
        }

        _ => {}
    }
}
