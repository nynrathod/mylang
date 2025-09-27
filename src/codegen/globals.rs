use crate::codegen::{CodeGen, Symbol};
use crate::mir::mir::MirInstr;
use inkwell::types::BasicTypeEnum;
use inkwell::types::{AsTypeRef, BasicType};
use inkwell::values::{AnyValue, AsValueRef, BasicValue, BasicValueEnum};
use inkwell::AddressSpace;

// External function from the LLVM C API needed to create constant arrays
// of complex types (like arrays of structs, or nested arrays).
use llvm_sys::core::LLVMConstArray;
use llvm_sys::prelude::LLVMValueRef;

impl<'ctx> CodeGen<'ctx> {
    /// Helper function using FFI (Foreign Function Interface) to manually create an
    /// LLVM constant array from a vector of values. This is essential for arrays
    /// containing complex types (structs, other arrays) where Inkwell's direct API
    /// is insufficient or requires advanced usage.
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
    pub fn generate_global(&mut self, instr: &MirInstr) {
        match instr {
            MirInstr::ConstInt { name, value } => {
                let val = self.context.i32_type().const_int(*value as u64, true);
                self.temp_values.insert(name.clone(), val.into());
            }
            MirInstr::ConstBool { name, value } => {
                let val = self.context.bool_type().const_int(*value as u64, false);
                self.temp_values.insert(name.clone(), val.into());
            }
            MirInstr::ConstString { name, value } => {
                if self.strings_to_concat.contains(name) {
                    // If the string is part of a concatenation, save its raw text temporarily.
                    self.temp_strings.insert(name.clone(), value.clone());
                } else {
                    // Otherwise, define it as a global constant array of i8 (char).
                    let s = self.module.add_global(
                        self.context.i8_type().array_type(value.len() as u32 + 1), // +1 for null terminator
                        None,
                        name,
                    );
                    // Initialize the global with the string data.
                    s.set_initializer(&self.context.const_string(value.as_bytes(), true));
                    // Store its pointer in temp_values for lookups.
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

            // Handles the final assignment of a constant/variable to its named global location.
            MirInstr::Assign {
                name,
                value,
                mutable,
            } => {
                // Special handling for constant strings to rename the temporary global
                // created by ConstString, avoiding redundant memory allocation.
                if let Some(string_ptr) = self.temp_values.get(value) {
                    if string_ptr.is_pointer_value() {
                        // ... (Complex logic to find and rename the temporary string global)
                        for global in self.module.get_globals() {
                            if global.as_pointer_value() == string_ptr.into_pointer_value() {
                                if let Some(initializer) = global.get_initializer() {
                                    global.set_name(name);
                                    global.set_constant(!*mutable); // Set constant based on mutability

                                    // Register the final symbol in the symbol table.
                                    self.symbols.insert(
                                        name.clone(),
                                        Symbol {
                                            ptr: global.as_pointer_value(),
                                            ty: initializer.get_type(),
                                        },
                                    );
                                    self.temp_values.remove(value); // Clean up temp value
                                    return;
                                }
                            }
                        }
                    }
                }

                // Standard global variable/constant definition for non-string values.
                let val = self.resolve_global_value(value);
                let g = self.module.add_global(val.get_type(), None, name);
                g.set_initializer(&val); // Set the initial constant value.
                g.set_constant(!*mutable); // Set constant flag.

                // Register the final symbol in the symbol table.
                self.symbols.insert(
                    name.clone(),
                    Symbol {
                        ptr: g.as_pointer_value(),
                        ty: val.get_type(),
                    },
                );
            }

            // Handles string concatenation at compile time (global scope).
            MirInstr::StringConcat { name, left, right } => {
                // Resolve raw string contents from the temp_strings map.
                let left_val = self.temp_strings.get(left).expect("Left string not found");
                let right_val = self
                    .temp_strings
                    .get(right)
                    .expect("Right string not found");
                let result = format!("{}{}", left_val, right_val);

                // Store the result string for potential further concatenation.
                self.temp_strings.insert(name.clone(), result.clone());

                // Define the new concatenated global string variable.
                let str_bytes = result.as_bytes();
                let str_len = str_bytes.len() + 1;
                let array_type = self.context.i8_type().array_type(str_len as u32);

                let g = self.module.add_global(array_type, None, name);
                g.set_initializer(&self.context.const_string(str_bytes, true));

                // Store its pointer for assignment/lookup.
                self.temp_values
                    .insert(name.clone(), g.as_pointer_value().into());
            }

            // Handles constant array initialization, including nested aggregates.
            MirInstr::Array { name, elements } => {
                // Resolve the LLVM constant value for ALL elements.
                let element_values: Vec<BasicValueEnum<'ctx>> = elements
                    .iter()
                    .map(|el| self.resolve_global_value(el))
                    .collect();

                // Determine the uniform type of the elements (using the first element).
                let first_val = &element_values[0];
                let elem_type = first_val.get_type();
                let _array_type = elem_type.array_type(elements.len() as u32);

                // Create the constant array initializer based on element type.
                let const_array = if elem_type.is_int_type() {
                    // Use Inkwell's simple const_array for arrays of primitive integers.
                    let int_values: Vec<_> = element_values
                        .into_iter()
                        .map(|v| v.into_int_value())
                        .collect();
                    elem_type
                        .into_int_type()
                        .const_array(&int_values)
                        .as_basic_value_enum()
                } else {
                    // Use the FFI helper for complex types (structs, pointers, nested arrays).
                    unsafe { Self::build_const_array(elem_type, element_values) }
                };

                // Store the final constant array value.
                self.temp_values.insert(name.clone(), const_array);
            }

            // Handles constant map initialization, represented as an array of structs.
            MirInstr::Map { name, entries } => {
                // Determine the types of the key and value from the first entry.
                let first_key = self.resolve_global_value(&entries[0].0);
                let first_val = self.resolve_global_value(&entries[0].1);
                let key_type = first_key.get_type();
                let val_type = first_val.get_type();
                // Define the structure type {KeyType, ValueType}.
                let pair_type = self.context.struct_type(&[key_type, val_type], false);

                // Build ALL struct entries using the defined pair type.
                let struct_values: Vec<BasicValueEnum<'ctx>> = entries
                    .iter()
                    .map(|(k, v)| {
                        let key_val = self.resolve_global_value(k);
                        let val_val = self.resolve_global_value(v);
                        // Create a constant struct for each key-value pair.
                        pair_type
                            .const_named_struct(&[key_val, val_val])
                            .as_basic_value_enum()
                    })
                    .collect();

                // Create the final constant array of ALL structs using the FFI helper.
                let const_array =
                    unsafe { Self::build_const_array(pair_type.into(), struct_values) };

                // Store the final constant map value.
                self.temp_values.insert(name.clone(), const_array);
            }

            _ => {}
        }
    }

    /// Resolves a name (which can be a temp variable or a literal) to its LLVM constant value.
    /// This is used recursively for building nested constants.
    pub fn resolve_global_value(&self, name: &str) -> BasicValueEnum<'ctx> {
        // Look up results of previous global instructions
        if let Some(val) = self.temp_values.get(name) {
            return *val;
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
        if name.starts_with('"') && name.ends_with('"') {
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

        panic!("Unknown global variable or literal: {}", name);
    }

    /// Finds a specific MIR instruction (e.g., a ConstString) by its destination name.
    /// This might be used to retrieve details about a globally defined value.
    pub fn find_instr_by_name(&self, name: &str) -> Option<&MirInstr> {
        self.globals.iter().find(|instr| match instr {
            MirInstr::ConstString { name: n, .. } => n == name,
            _ => false,
        })
    }
}
