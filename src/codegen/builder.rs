use crate::codegen::{CodeGen, Symbol};
use crate::mir::MirInstr;
use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicValueEnum;

impl<'ctx> CodeGen<'ctx> {
    /// Generates LLVM IR for a single Intermediate Representation (MIR) instruction.
    /// Returns the resulting LLVM value if the instruction produces one (like an expression),
    /// or None if it's purely a control instruction (like a basic block jump).
    pub fn generate_instr(&mut self, instr: &MirInstr) -> Option<BasicValueEnum<'ctx>> {
        match instr {
            MirInstr::ConstInt { name, value } => {
                let val = self.context.i32_type().const_int(*value as u64, true);
                // Returns the constant value; storage (if needed) is handled by MirInstr::Assign.
                Some(val.into())
            }

            MirInstr::ConstBool { value, .. } => Some(
                self.context
                    .bool_type()
                    .const_int(*value as u64, false)
                    .into(),
            ),

            MirInstr::ConstString { value, .. } => {
                let s = self
                    .builder
                    .build_global_string_ptr(value, "str_tmp")
                    .expect("Failed to create global string");
                Some(s.as_pointer_value().into())
            }

            // Handles binary operations (add, sub, mul, div, etc.) on integer values.
            MirInstr::BinaryOp(op, _dst, lhs, rhs) => {
                // Resolve the values of the left-hand side (lhs) and right-hand side (rhs).
                let lhs_val = self.resolve_value(lhs).into_int_value();
                let rhs_val = self.resolve_value(rhs).into_int_value();

                let res = match op.as_str() {
                    "add" => self
                        .builder
                        .build_int_add(lhs_val, rhs_val, "add_tmp")
                        .unwrap(),
                    "sub" => self
                        .builder
                        .build_int_sub(lhs_val, rhs_val, "sub_tmp")
                        .unwrap(),
                    "mul" => self
                        .builder
                        .build_int_mul(lhs_val, rhs_val, "mul_tmp")
                        .unwrap(),
                    "div" => self
                        .builder
                        .build_int_signed_div(lhs_val, rhs_val, "div_tmp")
                        .unwrap(),
                    _ => panic!("Unsupported binary op: {}", op),
                };

                Some(res.into())
            }

            // Handles variable assignment or re-assignment.
            MirInstr::Assign {
                name,
                value,
                mutable: _,
            } => {
                // Resolve the value (could be a literal, a temp, or result of an operation)
                let val = self.resolve_value(value);

                // Check if the variable already exists in the symbol table (re-assignment)
                if let Some(sym) = self.symbols.get(name) {
                    // Re-assignment: use build_store to write the new value to the existing memory location
                    self.builder.build_store(sym.ptr, val).unwrap();
                } else {
                    // Declaration: allocate new memory on the stack (build_alloca)
                    let alloca = self
                        .builder
                        .build_alloca(val.get_type(), name)
                        .expect("Failed to allocate memory for assignment");

                    // Store the resolved value into the newly allocated memory
                    self.builder.build_store(alloca, val);

                    // Register the new symbol (name, pointer, type) in the symbol table
                    self.symbols.insert(
                        name.clone(),
                        Symbol {
                            ptr: alloca,
                            ty: val.get_type(),
                        },
                    );
                }
                Some(val)
            }
            _ => None, // Unhandled instruction
        }
    }

    /// Resolves a variable name or literal string into its corresponding LLVM value.
    /// This is crucial for evaluating expressions.
    pub fn resolve_value(&self, name: &str) -> BasicValueEnum<'ctx> {
        //  Check for temporary constant values (e.g., results of global constant expression building)
        if let Some(val) = self.temp_values.get(name) {
            return *val;
        }

        // Check the symbol table for local variables (build_load the value from its pointer)
        if let Some(sym) = self.symbols.get(name) {
            // Load the value stored at the memory address
            self.builder
                .build_load(sym.ty, sym.ptr, name)
                .expect("Failed to load value")
        // Check for immediate literals (integers, booleans)
        } else if let Ok(val) = name.parse::<i32>() {
            // Constant integer literal
            self.context.i32_type().const_int(val as u64, true).into()
        } else if name == "true" {
            // Constant boolean literal (1)
            self.context.bool_type().const_int(1, false).into()
        } else if name == "false" {
            // Constant boolean literal (0)
            self.context.bool_type().const_int(0, false).into()
        } else {
            // If the name is not found anywhere, it's an error.
            panic!("Unknown variable or literal: {}", name);
        }
    }

    /// Converts a compiler-specific type name (e.g., "Int", "Str") to its corresponding LLVM type.
    pub fn get_llvm_type(&self, ty: &str) -> BasicTypeEnum<'ctx> {
        match ty {
            "Int" => self.context.i32_type().into(), // 32-bit signed integer
            "Bool" => self.context.bool_type().into(), // 1-bit integer
            "Str" => self
                .context
                .i8_type()
                .ptr_type(inkwell::AddressSpace::default())
                .into(), // Pointer to 8-bit integer (char* in C)
            _ => self.context.i32_type().into(),     // Default to i32 for unknown/placeholder types
        }
    }
}
