use crate::{
    lexar::token::TokenType,
    mir::{builder::MirBuilder, MirBlock, MirInstr},
    parser::ast::{AstNode, TypeNode},
};

/// Helper function to determine the type of an operand by looking it up in the symbol table
fn get_operand_type(builder: &MirBuilder, operand: &str) -> Option<TypeNode> {
    builder.mir_symbol_table.get(operand).cloned()
}

/// Helper function to determine the operation type for binary operations
/// Returns "float" if either operand is float, "int" if both are int, or None for incompatible types
pub fn determine_op_type(builder: &MirBuilder, lhs: &str, rhs: &str) -> Result<String, String> {
    let lhs_type = get_operand_type(builder, lhs);
    let rhs_type = get_operand_type(builder, rhs);

    match (lhs_type, rhs_type) {
        (Some(TypeNode::Float), Some(TypeNode::Float)) => Ok("float".to_string()),
        (Some(TypeNode::Float), Some(TypeNode::Int)) => Ok("float".to_string()),
        (Some(TypeNode::Int), Some(TypeNode::Float)) => Ok("float".to_string()),
        (Some(TypeNode::Int), Some(TypeNode::Int)) => Ok("int".to_string()),
        (Some(TypeNode::Bool), Some(TypeNode::Bool)) => Ok("bool".to_string()),
        (Some(TypeNode::String), Some(TypeNode::String)) => Ok("string".to_string()),
        (Some(TypeNode::String), _) | (_, Some(TypeNode::String)) => {
            Err(format!("Cannot perform arithmetic on string types"))
        }
        (Some(lhs_t), Some(rhs_t)) => Err(format!(
            "Type mismatch: cannot operate on {:?} and {:?}",
            lhs_t, rhs_t
        )),
        _ => {
            // If we don't know the type, assume int (for backward compatibility with untracked variables)
            Ok("int".to_string())
        }
    }
}

pub fn build_expression(builder: &mut MirBuilder, expr: &AstNode, block: &mut MirBlock) -> String {
    match expr {
        AstNode::NumberLiteral(n) => {
            let tmp = builder.next_tmp();
            block.instrs.push(MirInstr::ConstInt {
                name: tmp.clone(),
                value: *n,
            });
            // Track type in symbol table
            builder.mir_symbol_table.insert(tmp.clone(), TypeNode::Int);
            tmp
        }
        AstNode::FloatLiteral(f) => {
            let tmp = builder.next_tmp();
            block.instrs.push(MirInstr::ConstFloat {
                name: tmp.clone(),
                value: *f,
            });
            // Track type in symbol table
            builder
                .mir_symbol_table
                .insert(tmp.clone(), TypeNode::Float);
            tmp
        }

        AstNode::BoolLiteral(b) => {
            let tmp = builder.next_tmp();
            block.instrs.push(MirInstr::ConstBool {
                name: tmp.clone(),
                value: *b,
            });
            // Track type in symbol table
            builder.mir_symbol_table.insert(tmp.clone(), TypeNode::Bool);
            tmp
        }

        AstNode::StringLiteral(s) => {
            let tmp = builder.next_tmp();
            block.instrs.push(MirInstr::ConstString {
                name: tmp.clone(),
                value: s.clone(),
            });
            // Track type in symbol table
            builder
                .mir_symbol_table
                .insert(tmp.clone(), TypeNode::String);
            tmp
        }

        AstNode::Identifier(name) => name.clone(),

        AstNode::BinaryExpr { left, op, right } => {
            // Special handling for range expressions (.., ..=) used in for loops.
            match op {
                TokenType::RangeExc | TokenType::RangeInc => {
                    let start_tmp = build_expression(builder, left, block);
                    let end_tmp = build_expression(builder, right, block);
                    let range_tmp = builder.next_tmp();

                    block.instrs.push(MirInstr::RangeCreate {
                        name: range_tmp.clone(),
                        start: start_tmp,
                        end: end_tmp,
                        inclusive: matches!(op, TokenType::RangeInc),
                    });

                    range_tmp
                }

                _ => {
                    // Regular binary operations (add, sub, mul, div, etc.).
                    let lhs_tmp = build_expression(builder, left, block);
                    let rhs_tmp = build_expression(builder, right, block);
                    let dest_tmp = builder.next_tmp();

                    if *op == TokenType::Plus {
                        // Check if this is string concatenation
                        let lhs_type = get_operand_type(builder, &lhs_tmp);
                        let rhs_type = get_operand_type(builder, &rhs_tmp);

                        if matches!(lhs_type, Some(TypeNode::String))
                            || matches!(rhs_type, Some(TypeNode::String))
                        {
                            block.instrs.push(MirInstr::StringConcat {
                                name: dest_tmp.clone(),
                                left: lhs_tmp,
                                right: rhs_tmp,
                            });
                            builder
                                .mir_symbol_table
                                .insert(dest_tmp.clone(), TypeNode::String);
                        } else {
                            // Numeric addition - determine operation type
                            match determine_op_type(builder, &lhs_tmp, &rhs_tmp) {
                                Ok(op_type) if op_type == "string" => {
                                    block.instrs.push(MirInstr::StringConcat {
                                        name: dest_tmp.clone(),
                                        left: lhs_tmp,
                                        right: rhs_tmp,
                                    });
                                    builder
                                        .mir_symbol_table
                                        .insert(dest_tmp.clone(), TypeNode::String);
                                }
                                Ok(op_type) => {
                                    block.instrs.push(MirInstr::BinaryOp(
                                        format!("add:{}", op_type),
                                        dest_tmp.clone(),
                                        lhs_tmp,
                                        rhs_tmp,
                                    ));
                                    // Track result type
                                    if op_type == "float" {
                                        builder
                                            .mir_symbol_table
                                            .insert(dest_tmp.clone(), TypeNode::Float);
                                    } else {
                                        builder
                                            .mir_symbol_table
                                            .insert(dest_tmp.clone(), TypeNode::Int);
                                    }
                                }
                                Err(err) => {
                                    panic!("Type error in addition: {}", err);
                                }
                            }
                        }
                    } else {
                        // Other binary operators (sub, mul, div, comparisons, logical, etc.).
                        let op_str = match op {
                            TokenType::Minus => "sub",
                            TokenType::Star => "mul",
                            TokenType::Slash => "div",
                            TokenType::Gt => "gt",
                            TokenType::Lt => "lt",
                            TokenType::GtEq => "ge",
                            TokenType::LtEq => "le",
                            TokenType::EqEq => "eq",
                            TokenType::NotEq => "ne",
                            TokenType::Percent => "mod",
                            TokenType::AndAnd => "and",
                            TokenType::OrOr => "or",
                            _ => "unknown",
                        }
                        .to_string();

                        // Determine operation type based on operands
                        match determine_op_type(builder, &lhs_tmp, &rhs_tmp) {
                            Ok(op_type) if op_type == "string" => {
                                panic!("Cannot perform '{}' operation on string types", op_str);
                            }
                            Ok(op_type) => {
                                block.instrs.push(MirInstr::BinaryOp(
                                    format!("{}:{}", op_str, op_type),
                                    dest_tmp.clone(),
                                    lhs_tmp,
                                    rhs_tmp,
                                ));
                                // Track result type - comparisons and logical ops return bool, others return the operand type
                                if matches!(
                                    op_str.as_str(),
                                    "eq" | "ne" | "lt" | "le" | "gt" | "ge" | "and" | "or"
                                ) {
                                    builder
                                        .mir_symbol_table
                                        .insert(dest_tmp.clone(), TypeNode::Bool);
                                } else if op_type == "float" {
                                    builder
                                        .mir_symbol_table
                                        .insert(dest_tmp.clone(), TypeNode::Float);
                                } else {
                                    builder
                                        .mir_symbol_table
                                        .insert(dest_tmp.clone(), TypeNode::Int);
                                }
                            }
                            Err(err) => {
                                panic!("Type error in '{}' operation: {}", op_str, err);
                            }
                        }
                    }

                    dest_tmp
                }
            }
        }

        AstNode::FunctionCall { func, args } => {
            let mut arg_tmps = vec![];
            for arg in args {
                let arg_tmp = build_expression(builder, arg, block);
                arg_tmps.push(arg_tmp);
            }

            let dest_tmp = builder.next_tmp();
            let func_name = match &**func {
                AstNode::Identifier(name) => name.clone(),
                _ => {
                    // If func is an expression, evaluate it and use its result as the function name.
                    build_expression(builder, func, block)
                }
            };

            block.instrs.push(MirInstr::Call {
                dest: vec![dest_tmp.clone()],
                func: func_name,
                args: arg_tmps,
            });

            dest_tmp
        }

        AstNode::ArrayLiteral(elements) => {
            let mut tmp_elements = vec![];
            for elem in elements {
                let elem_tmp = build_expression(builder, elem, block);
                tmp_elements.push(elem_tmp);
            }

            let tmp = builder.next_tmp();
            block.instrs.push(MirInstr::Array {
                name: tmp.clone(),
                elements: tmp_elements,
            });
            // Track type in symbol table (simplified - could track element type)
            builder
                .mir_symbol_table
                .insert(tmp.clone(), TypeNode::Array(Box::new(TypeNode::Void)));
            tmp
        }

        AstNode::MapLiteral(entries) => {
            let mut map_entries = vec![];
            for (key_expr, val_expr) in entries {
                let key_tmp = build_expression(builder, key_expr, block);
                let val_tmp = build_expression(builder, val_expr, block);
                map_entries.push((key_tmp, val_tmp));
            }

            let tmp = builder.next_tmp();
            block.instrs.push(MirInstr::Map {
                name: tmp.clone(),
                entries: map_entries,
            });
            // Track type in symbol table: for empty maps, use default String, Int
            let map_type = if entries.is_empty() {
                TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::Int))
            } else {
                TypeNode::Map(Box::new(TypeNode::Void), Box::new(TypeNode::Void))
            };
            builder.mir_symbol_table.insert(tmp.clone(), map_type);
            tmp
        }

        // Element access: arr[index] or map[key]
        AstNode::ElementAccess { array, index } => {
            let array_tmp = build_expression(builder, array, block);
            let index_tmp = build_expression(builder, index, block);

            // Check if it's an array or map access by looking up the type
            let array_type = get_operand_type(builder, &array_tmp);

            match array_type {
                // Array element access
                Some(TypeNode::Array(_)) => {
                    let result_tmp = builder.next_tmp();
                    block.instrs.push(MirInstr::ArrayGet {
                        name: result_tmp.clone(),
                        array: array_tmp,
                        index: index_tmp,
                    });
                    result_tmp
                }
                // Map element access
                Some(TypeNode::Map(_, value_type)) => {
                    let result_tmp = builder.next_tmp();
                    block.instrs.push(MirInstr::MapGet {
                        name: result_tmp.clone(),
                        map: array_tmp,
                        key: index_tmp,
                    });
                    // Track the value type
                    builder
                        .mir_symbol_table
                        .insert(result_tmp.clone(), *value_type);
                    result_tmp
                }
                // Fallback: treat as array access
                _ => {
                    let result_tmp = builder.next_tmp();
                    block.instrs.push(MirInstr::ArrayGet {
                        name: result_tmp.clone(),
                        array: array_tmp,
                        index: index_tmp,
                    });
                    result_tmp
                }
            }
        }

        _ => {
            // For unhandled expressions, create a placeholder temporary.
            // This is a safeguard for future AST node types.
            builder.next_tmp()
        }
    }
}
