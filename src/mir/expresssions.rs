use crate::{
    lexar::token::TokenType,
    mir::{builder::MirBuilder, MirBlock, MirInstr},
    parser::ast::AstNode,
};

/// Convert AST expressions to MIR temporaries
/// Returns the temporary variable holding the expression result
pub fn build_expression(builder: &mut MirBuilder, expr: &AstNode, block: &mut MirBlock) -> String {
    match expr {
        // Literals create constant instructions
        AstNode::NumberLiteral(n) => {
            let tmp = builder.next_tmp();
            block.instrs.push(MirInstr::ConstInt {
                name: tmp.clone(),
                value: *n,
            });
            tmp
        }
        AstNode::BoolLiteral(b) => {
            let tmp = builder.next_tmp();
            block.instrs.push(MirInstr::ConstBool {
                name: tmp.clone(),
                value: *b,
            });
            tmp
        }
        AstNode::StringLiteral(s) => {
            let tmp = builder.next_tmp();
            block.instrs.push(MirInstr::ConstString {
                name: tmp.clone(),
                value: s.clone(),
            });
            tmp
        }

        // Variables reference existing names directly
        AstNode::Identifier(name) => name.clone(),

        // Binary operations become MIR binary ops
        AstNode::BinaryExpr { left, op, right } => {
            // Handle range expressions differently - they shouldn't be evaluated as regular binary ops
            match op {
                TokenType::RangeExc | TokenType::RangeInc => {
                    // Range expressions should only be handled in specific contexts (for loops)
                    // If we encounter them here, create a range object
                    let start_tmp = build_expression(builder, left, block);
                    let end_tmp = build_expression(builder, right, block);
                    let range_tmp = builder.next_tmp();

                    // Add a range creation instruction
                    block.instrs.push(MirInstr::RangeCreate {
                        name: range_tmp.clone(),
                        start: start_tmp,
                        end: end_tmp,
                        inclusive: matches!(op, TokenType::RangeInc),
                    });
                    range_tmp
                }
                _ => {
                    // Regular binary operations
                    let lhs_tmp = build_expression(builder, left, block);
                    let rhs_tmp = build_expression(builder, right, block);
                    let dest_tmp = builder.next_tmp();

                    let op_str = match op {
                        TokenType::Plus => "add",
                        TokenType::Minus => "sub",
                        TokenType::Star => "mul",
                        TokenType::Slash => "div",
                        TokenType::Gt => "gt",
                        TokenType::Lt => "lt",
                        TokenType::GtEq => "ge",
                        TokenType::LtEq => "le",
                        TokenType::EqEq => "eq",
                        TokenType::NotEq => "ne",
                        TokenType::Percent => "rem",
                        _ => "unknown",
                    }
                    .to_string();

                    block.instrs.push(MirInstr::BinaryOp(
                        op_str,
                        dest_tmp.clone(),
                        lhs_tmp,
                        rhs_tmp,
                    ));
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
                    // If func is an expression, evaluate it
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
            tmp
        }

        _ => {
            // For unhandled expressions, create a placeholder temporary
            builder.next_tmp()
        }
    }
}
