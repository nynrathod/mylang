use crate::codegen::{CodeGen, Symbol};
use crate::mir::mir::MirInstr;
use inkwell::types::BasicTypeEnum;
use inkwell::types::{AsTypeRef, BasicType};
use inkwell::values::{AnyValue, AsValueRef, BasicValue, BasicValueEnum};
use inkwell::AddressSpace;

/// This module provides functions for generating LLVM IR for global variables, constants, arrays, maps, and string operations.
/// It handles the translation of MIR instructions into LLVM global definitions, including constant folding and compile-time string concatenation.
/// The code here is essential for setting up the global state of a program before function-level code generation begins.
/// External function from the LLVM C API needed to create constant arrays
/// of complex types (like arrays of structs, or nested arrays).
use llvm_sys::core::LLVMConstArray;
use llvm_sys::prelude::LLVMValueRef;

/// Implements global code generation logic for the CodeGen struct.
/// This includes helpers for building constant arrays, generating global variables/constants,
/// resolving constant values, and handling compile-time string/map/array operations.
impl<'ctx> CodeGen<'ctx> {
    /// Helper function using FFI (Foreign Function Interface) to manually create an
    /// LLVM constant array from a vector of values.
    /// This is essential for arrays containing complex types (structs, other arrays)
    /// where Inkwell's direct API is insufficient or requires advanced usage.
    ///
    /// # Safety
    /// This function uses raw LLVM pointers and should be used with care.
    unsafe fn build_const_array(
        elem_type: BasicTypeEnum<'ctx>,
        values: Vec<BasicValueEnum<'ctx>>,
    ) -> BasicValueEnum<'ctx> {
        // Convert Inkwell values to raw LLVM value references.
        let mut raw: Vec<LLVMValueRef> = values.iter().map(|v| v.as_value_ref()).collect();
        // Call the raw LLVM function to build the constant array.
        let arr_ref = LLVMConstArray(elem_type.as_type_ref(), raw.as_mut_ptr(), raw.len() as u32);
        // Convert the raw reference back into an Inkwell ArrayValue.
        unsafe { inkwell::values::ArrayValue::new(arr_ref) }.as_basic_value_enum()
    }

    /// Generates the LLVM IR definition for a global constant or mutable variable.
    /// This function matches on MIR instructions and creates the corresponding LLVM global objects.
    /// It supports integers, booleans, strings, arrays, maps, binary operations, assignments, and string concatenation.
    ///
    /// - For constants, it inserts them into the temp_values map.
    /// - For assignments, it registers the symbol in the symbol table.
    /// - For arrays and maps, it builds constant initializers using FFI if needed.
    /// - For string concatenation, it creates a new global string.
    pub fn generate_global(&mut self, instr: &MirInstr) {
        match instr {
            // Integer constant global (only i32 supported)
            MirInstr::ConstInt { name, value } => {
                let val = self.context.i32_type().const_int(*value as u64, true);
                self.temp_values.insert(name.clone(), val.into());
            }
            MirInstr::ConstString { name, value } => {
                // For global strings, just create static constant (no RC)
                // RC will be handled in functions, not globals
                if self.strings_to_concat.contains(name) {
                    self.temp_strings.insert(name.clone(), value.clone());
                } else {
                    let s = self.module.add_global(
                        self.context.i8_type().array_type(value.len() as u32 + 1),
                        None,
                        name,
                    );
                    s.set_initializer(&self.context.const_string(value.as_bytes(), true));
                    self.temp_values
                        .insert(name.clone(), s.as_pointer_value().into());
                }
            }
            // Handles constant-time binary operations (e.g., global `a = 5 + 2`).
            MirInstr::BinaryOp(op, dst, lhs, rhs) => {
                // Resolve the constant values of the operands.
                let lhs_val = self.resolve_global_value(lhs).into_int_value();
                let rhs_val = self.resolve_global_value(rhs).into_int_value();

                // Perform the constant fold (calculation) directly.
                let res = match op.as_str() {
                    "add" => lhs_val.const_add(rhs_val),
                    "sub" => lhs_val.const_sub(rhs_val),
                    "mul" => lhs_val.const_mul(rhs_val),
                    "lt" => lhs_val.const_int_compare(inkwell::IntPredicate::SLT, rhs_val),
                    _ => panic!("Unsupported binary op: {}", op),
                };
                // Store the result as a new constant value.
                self.temp_values.insert(dst.clone(), res.into());
            }
            // Handles the final assignment of a constant/variable to its named global location (only i32 supported).
            MirInstr::Assign {
                name,
                value,
                mutable,
            } => {
                // Only integer assignments are supported.
                let val = self.resolve_global_value(value);
                let g = self.module.add_global(self.context.i32_type(), None, name);
                g.set_initializer(&val); // Set the initial constant value.
                g.set_constant(!*mutable); // Set constant flag.

                // Register the final symbol in the symbol table.
                self.symbols.insert(
                    name.clone(),
                    Symbol {
                        ptr: g.as_pointer_value(),
                        ty: self.context.i32_type().into(),
                    },
                );
            }
            // String concatenation is not supported (only i32 type allowed).
            MirInstr::StringConcat { .. } => {}
            // Array initialization is not supported (only i32 type allowed).
            MirInstr::Array { .. } => {}
            // Map initialization is not supported (only i32 type allowed).
            MirInstr::Map { .. } => {}
            // Ignore other MIR instructions
            _ => {}
        }
    }

    /// Resolves a name (which can be a temp variable or a literal) to its LLVM constant value.
    /// This is used recursively for building nested constants.
    ///
    /// - Checks temp_values for previously computed constants.
    /// - Checks symbols for previously defined global variables.
    /// - Handles immediate literals (integers, booleans, strings).
    /// - Panics if the value cannot be resolved.
    pub fn resolve_global_value(&self, name: &str) -> BasicValueEnum<'ctx> {
        // Look up results of previous global instructions
        if let Some(val) = self.temp_values.get(name) {
            return *val;
        }

        // Check if it's a previously defined global variable in symbols
        if let Some(symbol) = self.symbols.get(name) {
            let global = unsafe { inkwell::values::GlobalValue::new(symbol.ptr.as_value_ref()) };
            if let Some(initializer) = global.get_initializer() {
                return initializer;
            }
        }

        // Handle immediate literal values (integers, booleans).
        if let Ok(val) = name.parse::<i32>() {
            return self.context.i32_type().const_int(val as u64, true).into();
        }
        if name == "true" {
            return self.context.bool_type().const_int(1, false).into();
        }
        if name == "false" {
            return self.context.bool_type().const_int(0, false).into();
        }

        // Handle immediate string literals (e.g., `"hello"`).
        if name.starts_with('\"') && name.ends_with('\"') {
            let str_content = &name[1..name.len() - 1];
            // Define an anonymous global string for the literal.
            let s = self.module.add_global(
                self.context
                    .i8_type()
                    .array_type(str_content.len() as u32 + 1),
                None,
                &format!("str_literal_{}", self.temp_values.len()),
            );
            s.set_initializer(&self.context.const_string(str_content.as_bytes(), true));
            return s.as_pointer_value().into();
        }

        // If none of the above, panic with an error.
        panic!("Unknown global variable or literal: {}", name);
    }

    /// Finds a specific MIR instruction (e.g., a ConstString) by its destination name.
    /// This might be used to retrieve details about a globally defined value.
    ///
    /// Returns Some(&MirInstr) if found, otherwise None.
    pub fn find_instr_by_name(&self, name: &str) -> Option<&MirInstr> {
        self.globals.iter().find(|instr| match instr {
            MirInstr::ConstString { name: n, .. } => n == name,
            _ => false,
        })
    }
}
