use crate::codegen::{CodeGen, Symbol};
use crate::mir::MirInstr;
use inkwell::types::BasicType;
use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicValueEnum;
use inkwell::AddressSpace;

impl<'ctx> CodeGen<'ctx> {
    /// Generates LLVM IR for a single Intermediate Representation (MIR) instruction.
    /// Returns the resulting LLVM value if the instruction produces one (like an expression),
    /// or None if it's purely a control instruction (like a basic block jump).
    pub fn generate_instr(&mut self, instr: &MirInstr) -> Option<BasicValueEnum<'ctx>> {
        match instr {
            MirInstr::ConstInt { name, value } => {
                let val = self.context.i32_type().const_int(*value as u64, true);
                // Returns the constant value; storage (if needed) is handled by MirInstr::Assign.
                self.temp_values.insert(name.clone(), val.into());
                Some(val.into())
            }

            MirInstr::ConstBool { name, value } => {
                let val = self.context.bool_type().const_int(*value as u64, false);
                // STORE IT
                self.temp_values.insert(name.clone(), val.into());
                Some(val.into())
            }

            MirInstr::ConstString { name, value } => {
                let malloc_fn = self.get_or_declare_malloc();
                let total_size = 8 + value.len() + 1;
                let size_val = self.context.i64_type().const_int(total_size as u64, false);

                let heap_ptr = self
                    .builder
                    .build_call(malloc_fn, &[size_val.into()], "heap_str")
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_pointer_value();

                let i64_ptr = self
                    .builder
                    .build_pointer_cast(
                        heap_ptr,
                        self.context.i64_type().ptr_type(AddressSpace::default()),
                        "rc_ptr",
                    )
                    .unwrap();
                self.builder
                    .build_store(i64_ptr, self.context.i64_type().const_int(1, false))
                    .unwrap();

                let data_ptr = unsafe {
                    self.builder
                        .build_gep(
                            self.context.i8_type(),
                            heap_ptr,
                            &[self.context.i64_type().const_int(8, false)],
                            "data_ptr",
                        )
                        .unwrap()
                };

                let str_global = self
                    .builder
                    .build_global_string_ptr(value, "str_const")
                    .expect("Failed to create string");

                let memcpy = self.get_or_declare_memcpy();
                let len = self
                    .context
                    .i64_type()
                    .const_int((value.len() + 1) as u64, false);
                self.builder
                    .build_call(
                        memcpy,
                        &[
                            data_ptr.into(),
                            str_global.as_pointer_value().into(),
                            len.into(),
                            self.context.bool_type().const_zero().into(),
                        ],
                        "",
                    )
                    .unwrap();

                // CRITICAL: Store BOTH in temp_values
                self.temp_values.insert(name.clone(), data_ptr.into());
                self.heap_strings.insert(name.clone()); // Mark as heap string

                Some(data_ptr.into())
            }

            MirInstr::Array { name, elements } => {
                let element_values: Vec<BasicValueEnum<'ctx>> =
                    elements.iter().map(|el| self.resolve_value(el)).collect();

                if element_values.is_empty() {
                    panic!("Empty arrays not supported");
                }

                // Track string pointers
                let str_ptrs: Vec<BasicValueEnum<'ctx>> = element_values
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| self.heap_strings.contains(&elements[*i]))
                    .map(|(_, val)| *val)
                    .collect();

                if !str_ptrs.is_empty() {
                    self.composite_string_ptrs.insert(name.clone(), str_ptrs);
                }

                let elem_type = element_values[0].get_type();
                let array_type = elem_type.array_type(elements.len() as u32);

                // HEAP ALLOCATE with RC header
                let malloc_fn = self.get_or_declare_malloc();
                let array_size = array_type.size_of().unwrap();
                let total_size = self.context.i64_type().const_int(8, false);
                let total_size = self
                    .builder
                    .build_int_add(total_size, array_size, "total_size")
                    .unwrap();

                let heap_ptr = self
                    .builder
                    .build_call(malloc_fn, &[total_size.into()], "heap_array")
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_pointer_value();

                // Store RC = 1
                let i64_ptr = self
                    .builder
                    .build_pointer_cast(
                        heap_ptr,
                        self.context.i64_type().ptr_type(AddressSpace::default()),
                        "rc_ptr",
                    )
                    .unwrap();
                self.builder
                    .build_store(i64_ptr, self.context.i64_type().const_int(1, false))
                    .unwrap();

                // Get data pointer (after RC header)
                let data_ptr = unsafe {
                    self.builder
                        .build_gep(
                            self.context.i8_type(),
                            heap_ptr,
                            &[self.context.i64_type().const_int(8, false)],
                            "data_ptr",
                        )
                        .unwrap()
                };

                // Cast to array type pointer
                let array_ptr = self
                    .builder
                    .build_pointer_cast(
                        data_ptr,
                        array_type.ptr_type(AddressSpace::default()),
                        "array_ptr",
                    )
                    .unwrap();

                // Store elements
                for (i, val) in element_values.iter().enumerate() {
                    let idx = self.context.i32_type().const_int(i as u64, false);
                    let elem_ptr = unsafe {
                        self.builder
                            .build_gep(
                                array_type,
                                array_ptr,
                                &[self.context.i32_type().const_zero(), idx],
                                &format!("elem_{}", i),
                            )
                            .unwrap()
                    };
                    self.builder.build_store(elem_ptr, *val).unwrap();
                }

                self.temp_values.insert(name.clone(), data_ptr.into());
                self.heap_arrays.insert(name.clone());
                Some(data_ptr.into())
            }

            MirInstr::Map { name, entries } => {
                if entries.is_empty() {
                    panic!("Empty maps not supported");
                }

                // Track string keys and values
                let mut str_temps = Vec::new();
                for (k, v) in entries {
                    if self.heap_strings.contains(k) {
                        str_temps.push(k.clone());
                    }
                    if self.heap_strings.contains(v) {
                        str_temps.push(v.clone());
                    }
                }

                if !str_temps.is_empty() {
                    self.composite_strings.insert(name.clone(), str_temps);
                }

                let first_key = self.resolve_value(&entries[0].0);
                let first_val = self.resolve_value(&entries[0].1);
                let key_type = first_key.get_type();
                let val_type = first_val.get_type();

                let pair_type = self.context.struct_type(&[key_type, val_type], false);
                let map_type = pair_type.array_type(entries.len() as u32);

                // HEAP ALLOCATE with RC header
                let malloc_fn = self.get_or_declare_malloc();
                let map_size = map_type.size_of().unwrap();
                let total_size = self.context.i64_type().const_int(8, false);
                let total_size = self
                    .builder
                    .build_int_add(total_size, map_size, "total_size")
                    .unwrap();

                let heap_ptr = self
                    .builder
                    .build_call(malloc_fn, &[total_size.into()], "heap_map")
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_pointer_value();

                // Store RC = 1
                let i64_ptr = self
                    .builder
                    .build_pointer_cast(
                        heap_ptr,
                        self.context.i64_type().ptr_type(AddressSpace::default()),
                        "rc_ptr",
                    )
                    .unwrap();
                self.builder
                    .build_store(i64_ptr, self.context.i64_type().const_int(1, false))
                    .unwrap();

                // Get data pointer
                let data_ptr = unsafe {
                    self.builder
                        .build_gep(
                            self.context.i8_type(),
                            heap_ptr,
                            &[self.context.i64_type().const_int(8, false)],
                            "data_ptr",
                        )
                        .unwrap()
                };

                let map_ptr = self
                    .builder
                    .build_pointer_cast(
                        data_ptr,
                        map_type.ptr_type(AddressSpace::default()),
                        "map_ptr",
                    )
                    .unwrap();

                for (i, (k, v)) in entries.iter().enumerate() {
                    let key_val = self.resolve_value(k);
                    let val_val = self.resolve_value(v);

                    let idx = self.context.i32_type().const_int(i as u64, false);
                    let pair_ptr = unsafe {
                        self.builder
                            .build_gep(
                                map_type,
                                map_ptr,
                                &[self.context.i32_type().const_zero(), idx],
                                &format!("pair_{}", i),
                            )
                            .unwrap()
                    };

                    let key_ptr = self
                        .builder
                        .build_struct_gep(pair_type, pair_ptr, 0, "key_ptr")
                        .unwrap();
                    self.builder.build_store(key_ptr, key_val).unwrap();

                    let val_ptr = self
                        .builder
                        .build_struct_gep(pair_type, pair_ptr, 1, "val_ptr")
                        .unwrap();
                    self.builder.build_store(val_ptr, val_val).unwrap();
                }

                self.temp_values.insert(name.clone(), data_ptr.into());
                self.heap_maps.insert(name.clone());
                Some(data_ptr.into())
            }

            // Handles binary operations (add, sub, mul, div, etc.) on integer values.
            MirInstr::BinaryOp(op, dst, lhs, rhs) => {
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

                self.temp_values.insert(dst.clone(), res.into());
                Some(res.into())
            }

            // Handles variable assignment or re-assignment.
            MirInstr::Assign {
                name,
                value,
                mutable: _,
            } => {
                let val = self.resolve_value(value);

                // Check what type of heap value this is
                let value_is_heap_str = self.heap_strings.contains(value);
                let value_is_heap_array = self.heap_arrays.contains(value);
                let value_is_heap_map = self.heap_maps.contains(value);

                if let Some(ptrs) = self.composite_string_ptrs.remove(value) {
                    self.composite_string_ptrs.insert(name.clone(), ptrs);
                }

                if let Some(sym) = self.symbols.get(name) {
                    // Re-assignment: decref old value
                    let name_was_heap_str = self.heap_strings.contains(name);
                    let name_was_heap_array = self.heap_arrays.contains(name);
                    let name_was_heap_map = self.heap_maps.contains(name);

                    if name_was_heap_array || name_was_heap_map {
                        if let Some(old_str_ptrs) = self.composite_string_ptrs.get(name) {
                            for str_ptr in old_str_ptrs {
                                let data_ptr = str_ptr.into_pointer_value();
                                let rc_header = unsafe {
                                    self.builder.build_in_bounds_gep(
                                        self.context.i8_type(),
                                        data_ptr,
                                        &[self.context.i64_type().const_int((-8_i64) as u64, true)],
                                        "rc_header",
                                    )
                                }
                                .unwrap();

                                let decref = self.decref_fn.unwrap();
                                self.builder
                                    .build_call(decref, &[rc_header.into()], "")
                                    .unwrap();
                            }
                        }
                    }

                    if name_was_heap_str || name_was_heap_array || name_was_heap_map {
                        self.emit_decref(name);
                    }

                    self.builder.build_store(sym.ptr, val).unwrap();

                    // Update tracking
                    self.heap_strings.remove(name);
                    self.heap_arrays.remove(name);
                    self.heap_maps.remove(name);

                    if value_is_heap_str {
                        self.heap_strings.insert(name.clone());
                        self.emit_incref(name);
                    } else if value_is_heap_array {
                        self.heap_arrays.insert(name.clone());
                        self.emit_incref(name);
                    } else if value_is_heap_map {
                        self.heap_maps.insert(name.clone());
                        self.emit_incref(name);
                    }
                } else {
                    // Initial assignment
                    let alloca = self.builder.build_alloca(val.get_type(), name).unwrap();
                    self.builder.build_store(alloca, val).unwrap();

                    self.symbols.insert(
                        name.clone(),
                        Symbol {
                            ptr: alloca,
                            ty: val.get_type(),
                        },
                    );

                    if value_is_heap_str {
                        self.heap_strings.insert(name.clone());
                        if self.symbols.contains_key(value) {
                            self.emit_incref(name);
                        }
                    } else if value_is_heap_array {
                        self.heap_arrays.insert(name.clone());
                        if self.symbols.contains_key(value) {
                            self.emit_incref(name);
                        }
                    } else if value_is_heap_map {
                        self.heap_maps.insert(name.clone());
                        if self.symbols.contains_key(value) {
                            self.emit_incref(name);
                        }
                    }
                }
                Some(val)
            }

            MirInstr::IncRef { value } => {
                self.emit_incref(value);
                None
            }

            MirInstr::DecRef { value } => {
                self.emit_decref(value);
                None
            }

            _ => None, // Unhandled instruction
        }
    }

    /// Resolves a variable name or literal string into its corresponding LLVM value.
    /// This is crucial for evaluating expressions.
    pub fn resolve_value(&self, name: &str) -> BasicValueEnum<'ctx> {
        // Check temp values first
        if let Some(val) = self.temp_values.get(name) {
            return *val;
        }

        // Check symbols
        if let Some(sym) = self.symbols.get(name) {
            return self
                .builder
                .build_load(sym.ty, sym.ptr, name)
                .expect("Failed to load value");
        }

        // Handle literals
        if let Ok(val) = name.parse::<i32>() {
            return self.context.i32_type().const_int(val as u64, true).into();
        }
        if name == "true" {
            return self.context.bool_type().const_int(1, false).into();
        }
        if name == "false" {
            return self.context.bool_type().const_int(0, false).into();
        }

        // Better error message
        eprintln!("Available temps: {:?}", self.temp_values.keys());
        eprintln!("Available symbols: {:?}", self.symbols.keys());
        panic!(
            "Unknown variable or literal: {} - check your MIR generation",
            name
        );
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
