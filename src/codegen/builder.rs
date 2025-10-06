use crate::codegen::{CodeGen, Symbol};
use crate::mir::MirInstr;
use inkwell::types::BasicType;
use inkwell::types::BasicTypeEnum;
use inkwell::types::StructType;
use inkwell::values::BasicValueEnum;
use inkwell::AddressSpace;
use inkwell::IntPredicate;

use std::collections::HashMap;

impl<'ctx> CodeGen<'ctx> {
    /// Generates LLVM IR for a single Intermediate Representation (MIR) instruction.
    /// Returns the resulting LLVM value if the instruction produces one (like an expression),
    /// or None if it's purely a control instruction (like a basic block jump).
    pub fn generate_instr(&mut self, instr: &MirInstr) -> Option<BasicValueEnum<'ctx>> {
        match instr {
            MirInstr::ConstInt { name, value } => {
                let val = self.context.i32_type().const_int(*value as u64, true);
                self.temp_values.insert(name.clone(), val.into());
                Some(val.into())
            }

            MirInstr::ConstBool { name, value } => {
                let val = self.context.bool_type().const_int(*value as u64, false);
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

                self.temp_values.insert(name.clone(), data_ptr.into());
                self.heap_strings.insert(name.clone());

                Some(data_ptr.into())
            }

            MirInstr::Array { name, elements } => self.generate_array_with_metadata(name, elements),
            MirInstr::Map { name, entries } => self.generate_map_with_metadata(name, entries),

            // ===== LOOP INSTRUCTIONS =====
            MirInstr::ForRange { .. }
            | MirInstr::ForArray { .. }
            | MirInstr::ForMap { .. }
            | MirInstr::ForInfinite { .. }
            | MirInstr::Break { .. }
            | MirInstr::Continue { .. } => {
                // These need bb_map, so they should be handled in generate_block
                // This is just a placeholder - actual handling in generate_block_with_loops
                None
            }

            MirInstr::LoopBodyMarker { .. } => {
                // Marker instruction - no code generation needed
                // The marker is used by generate_block_with_loops to know how to handle the block
                None
            }

            MirInstr::LoadArrayElement { dest, array, index } => {
                let array_ptr = self.resolve_value(array).into_pointer_value();
                let index_val = self.resolve_value(index).into_int_value();

                let is_string = self.array_contains_strings(array);
                let elem_type = self.get_array_element_type(array);

                // Get array metadata to know the array type
                let array_len = if let Some(metadata) = self.array_metadata.get(array) {
                    metadata.length as u32
                } else {
                    0
                };

                let array_type = elem_type.array_type(array_len);

                // Cast data pointer to array pointer
                let typed_array_ptr = self
                    .builder
                    .build_pointer_cast(
                        array_ptr,
                        array_type.ptr_type(AddressSpace::default()),
                        "array_ptr_typed",
                    )
                    .unwrap();

                // GEP to get element pointer
                let elem_ptr = unsafe {
                    self.builder.build_gep(
                        array_type,
                        typed_array_ptr,
                        &[self.context.i32_type().const_zero(), index_val],
                        "elem_ptr",
                    )
                }
                .unwrap();

                // Load element
                let elem_val = self
                    .builder
                    .build_load(elem_type, elem_ptr, "elem")
                    .unwrap();

                // If it's a heap-allocated string, increment RC
                if is_string && elem_val.is_pointer_value() {
                    let str_ptr = elem_val.into_pointer_value();
                    let rc_header = unsafe {
                        self.builder.build_in_bounds_gep(
                            self.context.i8_type(),
                            str_ptr,
                            &[self.context.i64_type().const_int((-8_i64) as u64, true)],
                            "rc_header",
                        )
                    }
                    .unwrap();

                    let incref = self.incref_fn.unwrap();
                    self.builder
                        .build_call(incref, &[rc_header.into()], "")
                        .unwrap();

                    // Mark this variable as heap string for cleanup
                    self.heap_strings.insert(dest.clone());
                }

                // Store in destination variable
                if let Some(symbol) = self.symbols.get(dest) {
                    self.builder.build_store(symbol.ptr, elem_val).unwrap();
                } else {
                    // Create new variable
                    let alloca = self.builder.build_alloca(elem_type, dest).unwrap();
                    self.builder.build_store(alloca, elem_val).unwrap();

                    self.symbols.insert(
                        dest.clone(),
                        Symbol {
                            ptr: alloca,
                            ty: elem_type,
                        },
                    );
                }

                Some(elem_val)
            }

            MirInstr::LoadMapPair {
                key_dest,
                val_dest,
                map,
                index,
            } => {
                let map_ptr = self.resolve_value(map).into_pointer_value();
                let index_val = self.resolve_value(index).into_int_value();

                let (key_is_string, val_is_string) = self.map_contains_strings(map);
                let (key_type, val_type) = self.get_map_types(map);
                let pair_type = self.context.struct_type(&[key_type, val_type], false);

                // Get map length for array type
                let map_len = if let Some(metadata) = self.map_metadata.get(map) {
                    metadata.length as u32
                } else {
                    0
                };

                let map_array_type = pair_type.array_type(map_len);

                // Cast to typed map pointer
                let typed_map_ptr = self
                    .builder
                    .build_pointer_cast(
                        map_ptr,
                        map_array_type.ptr_type(AddressSpace::default()),
                        "map_ptr_typed",
                    )
                    .unwrap();

                // GEP to get pair pointer
                let pair_ptr = unsafe {
                    self.builder.build_gep(
                        map_array_type,
                        typed_map_ptr,
                        &[self.context.i32_type().const_zero(), index_val],
                        "pair_ptr",
                    )
                }
                .unwrap();

                // Extract key (field 0)
                let key_ptr = self
                    .builder
                    .build_struct_gep(pair_type, pair_ptr, 0, "key_ptr")
                    .unwrap();
                let key_val = self.builder.build_load(key_type, key_ptr, "key").unwrap();

                // Extract value (field 1)
                let val_ptr = self
                    .builder
                    .build_struct_gep(pair_type, pair_ptr, 1, "val_ptr")
                    .unwrap();
                let val_val = self.builder.build_load(val_type, val_ptr, "val").unwrap();

                // Handle RC for key if string
                if key_is_string && key_val.is_pointer_value() {
                    let str_ptr = key_val.into_pointer_value();
                    let rc_header = unsafe {
                        self.builder.build_in_bounds_gep(
                            self.context.i8_type(),
                            str_ptr,
                            &[self.context.i64_type().const_int((-8_i64) as u64, true)],
                            "rc_header",
                        )
                    }
                    .unwrap();
                    let incref = self.incref_fn.unwrap();
                    self.builder
                        .build_call(incref, &[rc_header.into()], "")
                        .unwrap();
                    self.heap_strings.insert(key_dest.clone());
                }

                // Handle RC for value if string
                if val_is_string && val_val.is_pointer_value() {
                    let str_ptr = val_val.into_pointer_value();
                    let rc_header = unsafe {
                        self.builder.build_in_bounds_gep(
                            self.context.i8_type(),
                            str_ptr,
                            &[self.context.i64_type().const_int((-8_i64) as u64, true)],
                            "rc_header",
                        )
                    }
                    .unwrap();
                    let incref = self.incref_fn.unwrap();
                    self.builder
                        .build_call(incref, &[rc_header.into()], "")
                        .unwrap();
                    self.heap_strings.insert(val_dest.clone());
                }

                // Store key
                if let Some(symbol) = self.symbols.get(key_dest) {
                    self.builder.build_store(symbol.ptr, key_val).unwrap();
                } else {
                    let alloca = self.builder.build_alloca(key_type, key_dest).unwrap();
                    self.builder.build_store(alloca, key_val).unwrap();
                    self.symbols.insert(
                        key_dest.clone(),
                        Symbol {
                            ptr: alloca,
                            ty: key_type,
                        },
                    );
                }

                // Store value
                if let Some(symbol) = self.symbols.get(val_dest) {
                    self.builder.build_store(symbol.ptr, val_val).unwrap();
                } else {
                    let alloca = self.builder.build_alloca(val_type, val_dest).unwrap();
                    self.builder.build_store(alloca, val_val).unwrap();
                    self.symbols.insert(
                        val_dest.clone(),
                        Symbol {
                            ptr: alloca,
                            ty: val_type,
                        },
                    );
                }

                None
            }

            // ===== EXISTING INSTRUCTIONS =====
            MirInstr::BinaryOp(op, dst, lhs, rhs) => {
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
                    "mod" => self
                        .builder
                        .build_int_signed_rem(lhs_val, rhs_val, "mod_tmp")
                        .unwrap(),
                    "eq" => self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::EQ, lhs_val, rhs_val, "eq_tmp")
                        .unwrap(),
                    "ne" => self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::NE, lhs_val, rhs_val, "ne_tmp")
                        .unwrap(),
                    "lt" => self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::SLT, lhs_val, rhs_val, "lt_tmp")
                        .unwrap(),
                    "le" => self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::SLE, lhs_val, rhs_val, "le_tmp")
                        .unwrap(),
                    "gt" => self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::SGT, lhs_val, rhs_val, "gt_tmp")
                        .unwrap(),
                    "ge" => self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::SGE, lhs_val, rhs_val, "ge_tmp")
                        .unwrap(),
                    _ => panic!("Unsupported binary op: {}", op),
                };

                self.temp_values.insert(dst.clone(), res.into());
                Some(res.into())
            }

            MirInstr::Assign {
                name,
                value,
                mutable: _,
            } => {
                let val = self.resolve_value(value);

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

                        // Copy array metadata
                        if let Some(metadata) = self.array_metadata.get(value).cloned() {
                            self.array_metadata.insert(name.clone(), metadata);
                        }
                    } else if value_is_heap_map {
                        self.heap_maps.insert(name.clone());
                        if self.symbols.contains_key(value) {
                            self.emit_incref(name);
                        }

                        // Copy map metadata
                        if let Some(metadata) = self.map_metadata.get(value).cloned() {
                            self.map_metadata.insert(name.clone(), metadata);
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

            _ => None,
        }
    }

    /// Resolve value (unchanged)
    pub fn resolve_value(&self, name: &str) -> BasicValueEnum<'ctx> {
        if let Some(val) = self.temp_values.get(name) {
            return *val;
        }

        if let Some(sym) = self.symbols.get(name) {
            return self
                .builder
                .build_load(sym.ty, sym.ptr, name)
                .expect("Failed to load value");
        }

        if let Ok(val) = name.parse::<i32>() {
            return self.context.i32_type().const_int(val as u64, true).into();
        }
        if name == "true" {
            return self.context.bool_type().const_int(1, false).into();
        }
        if name == "false" {
            return self.context.bool_type().const_int(0, false).into();
        }

        eprintln!("Available temps: {:?}", self.temp_values.keys());
        eprintln!("Available symbols: {:?}", self.symbols.keys());
        panic!(
            "Unknown variable or literal: {} - check your MIR generation",
            name
        );
    }

    pub fn get_llvm_type(&self, ty: &str) -> BasicTypeEnum<'ctx> {
        match ty {
            "Int" => self.context.i32_type().into(),
            "Bool" => self.context.bool_type().into(),
            "Str" => self
                .context
                .i8_type()
                .ptr_type(inkwell::AddressSpace::default())
                .into(),
            _ => self.context.i32_type().into(),
        }
    }

    pub fn generate_array_with_metadata(
        &mut self,
        name: &str,
        elements: &[String],
    ) -> Option<BasicValueEnum<'ctx>> {
        let element_values: Vec<BasicValueEnum<'ctx>> =
            elements.iter().map(|el| self.resolve_value(el)).collect();

        if element_values.is_empty() {
            panic!("Empty arrays not supported");
        }

        let elem_type = element_values[0].get_type();
        let array_type = elem_type.array_type(elements.len() as u32);

        // Track string pointers
        let str_ptrs: Vec<BasicValueEnum<'ctx>> = element_values
            .iter()
            .enumerate()
            .filter(|(i, _)| self.heap_strings.contains(&elements[*i]))
            .map(|(_, val)| *val)
            .collect();

        let contains_strings = !str_ptrs.is_empty();

        if contains_strings {
            self.composite_string_ptrs
                .insert(name.to_string(), str_ptrs);
        }

        // Store metadata
        let element_type_name = if elem_type.is_int_type() {
            "Int"
        } else if elem_type.is_pointer_type() {
            "Str"
        } else {
            "Unknown"
        };

        self.array_metadata.insert(
            name.to_string(),
            crate::codegen::ArrayMetadata {
                length: elements.len(),
                element_type: element_type_name.to_string(),
                contains_strings,
            },
        );

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

        self.temp_values.insert(name.to_string(), data_ptr.into());
        self.heap_arrays.insert(name.to_string());
        Some(data_ptr.into())
    }

    pub fn generate_map_with_metadata(
        &mut self,
        name: &str,
        entries: &[(String, String)],
    ) -> Option<BasicValueEnum<'ctx>> {
        if entries.is_empty() {
            panic!("Empty maps not supported");
        }

        // Track string keys and values
        let mut str_temps = Vec::new();
        let mut key_is_string = false;
        let mut value_is_string = false;

        for (k, v) in entries {
            if self.heap_strings.contains(k) {
                str_temps.push(k.clone());
                key_is_string = true;
            }
            if self.heap_strings.contains(v) {
                str_temps.push(v.clone());
                value_is_string = true;
            }
        }

        if !str_temps.is_empty() {
            self.composite_strings.insert(name.to_string(), str_temps);
        }

        let first_key = self.resolve_value(&entries[0].0);
        let first_val = self.resolve_value(&entries[0].1);
        let key_type = first_key.get_type();
        let val_type = first_val.get_type();

        let key_type_name = if key_type.is_int_type() {
            "Int"
        } else if key_type.is_pointer_type() {
            "Str"
        } else {
            "Unknown"
        };

        let val_type_name = if val_type.is_int_type() {
            "Int"
        } else if val_type.is_pointer_type() {
            "Str"
        } else {
            "Unknown"
        };

        self.map_metadata.insert(
            name.to_string(),
            crate::codegen::MapMetadata {
                length: entries.len(),
                key_type: key_type_name.to_string(),
                value_type: val_type_name.to_string(),
                key_is_string,
                value_is_string,
            },
        );

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

        self.temp_values.insert(name.to_string(), data_ptr.into());
        self.heap_maps.insert(name.to_string());
        Some(data_ptr.into())
    }

    pub fn generate_for_loop(
        &mut self,
        instr: &MirInstr,
        bb_map: &HashMap<String, inkwell::basic_block::BasicBlock<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        match instr {
            MirInstr::ForRange {
                var,
                start,
                end,
                inclusive,
                body_block,
                exit_block,
            } => {
                self.generate_for_range(var, start, end, *inclusive, body_block, exit_block, bb_map)
            }

            MirInstr::ForArray {
                var,
                array,
                index_var,
                body_block,
                exit_block,
            } => self.generate_for_array(var, array, index_var, body_block, exit_block, bb_map),

            MirInstr::ForMap {
                key_var,
                value_var,
                map,
                index_var,
                body_block,
                exit_block,
            } => self.generate_for_map(
                key_var, value_var, map, index_var, body_block, exit_block, bb_map,
            ),

            MirInstr::ForInfinite { body_block } => self.generate_for_infinite(body_block, bb_map),

            MirInstr::Break { target } => {
                let target_bb = bb_map.get(target).expect("Break target not found");
                self.builder.build_unconditional_branch(*target_bb).unwrap();
                None
            }

            MirInstr::Continue { target } => {
                let target_bb = bb_map.get(target).expect("Continue target not found");
                self.builder.build_unconditional_branch(*target_bb).unwrap();
                None
            }

            _ => None,
        }
    }

    /// Generate for i in start..end or for i in start..=end
    fn generate_for_range(
        &mut self,
        var: &str,
        start: &str,
        end: &str,
        inclusive: bool,
        body_block: &str,
        exit_block: &str,
        bb_map: &HashMap<String, inkwell::basic_block::BasicBlock<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let start_val = self.resolve_value(start).into_int_value();
        let end_val = self.resolve_value(end).into_int_value();

        // Allocate loop variable
        let loop_var_alloca = self
            .builder
            .build_alloca(self.context.i32_type(), var)
            .unwrap();
        self.builder
            .build_store(loop_var_alloca, start_val)
            .unwrap();

        self.symbols.insert(
            var.to_string(),
            Symbol {
                ptr: loop_var_alloca,
                ty: self.context.i32_type().into(),
            },
        );

        // Get blocks
        let body_bb = bb_map.get(body_block).expect("Body block not found");
        let exit_bb = bb_map.get(exit_block).expect("Exit block not found");

        // Create condition check block
        let current_func = self
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();
        let cond_bb = self.context.append_basic_block(current_func, "for.cond");

        // Enter loop context for break/continue
        self.enter_loop(
            exit_block.to_string(),
            cond_bb.get_name().to_str().unwrap().to_string(),
        );

        // Jump to condition
        self.builder.build_unconditional_branch(cond_bb).unwrap();

        // Build condition block
        self.builder.position_at_end(cond_bb);
        let current_val = self
            .builder
            .build_load(self.context.i32_type(), loop_var_alloca, "loop_var")
            .unwrap()
            .into_int_value();

        let predicate = if inclusive {
            IntPredicate::SLE
        } else {
            IntPredicate::SLT
        };

        let cond = self
            .builder
            .build_int_compare(predicate, current_val, end_val, "loop_cond")
            .unwrap();

        self.builder
            .build_conditional_branch(cond, *body_bb, *exit_bb)
            .unwrap();

        // Body block will be filled by generate_block
        // After body, need to increment and jump back to cond

        None
    }

    /// Generate for item in array
    fn generate_for_array(
        &mut self,
        var: &str,
        array: &str,
        index_var: &str,
        body_block: &str,
        exit_block: &str,
        bb_map: &HashMap<String, inkwell::basic_block::BasicBlock<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let array_val = self.resolve_value(array);
        let array_ptr = array_val.into_pointer_value();

        // Allocate index counter
        let index_alloca = self
            .builder
            .build_alloca(self.context.i32_type(), index_var)
            .unwrap();
        let zero = self.context.i32_type().const_int(0, false);
        self.builder.build_store(index_alloca, zero).unwrap();

        self.symbols.insert(
            index_var.to_string(),
            Symbol {
                ptr: index_alloca,
                ty: self.context.i32_type().into(),
            },
        );

        // Get array length
        let array_len = self.get_array_length(array);

        // Get blocks
        let body_bb = bb_map.get(body_block).expect("Body block not found");
        let exit_bb = bb_map.get(exit_block).expect("Exit block not found");

        // Create condition block
        let current_func = self
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();
        let cond_bb = self
            .context
            .append_basic_block(current_func, "for.arr.cond");

        // Enter loop context
        self.enter_loop(
            exit_block.to_string(),
            cond_bb.get_name().to_str().unwrap().to_string(),
        );
        self.add_loop_var(var.to_string());

        // Jump to condition
        self.builder.build_unconditional_branch(cond_bb).unwrap();

        // Build condition: index < array_len
        self.builder.position_at_end(cond_bb);
        let current_index = self
            .builder
            .build_load(self.context.i32_type(), index_alloca, "current_idx")
            .unwrap()
            .into_int_value();

        let cond = self
            .builder
            .build_int_compare(IntPredicate::SLT, current_index, array_len, "arr_cond")
            .unwrap();

        self.builder
            .build_conditional_branch(cond, *body_bb, *exit_bb)
            .unwrap();

        // Allocate item variable
        let elem_type = self.get_array_element_type(array);
        let item_alloca = self.builder.build_alloca(elem_type, var).unwrap();

        self.symbols.insert(
            var.to_string(),
            Symbol {
                ptr: item_alloca,
                ty: elem_type,
            },
        );

        None
    }

    /// Generate for (key, value) in map
    fn generate_for_map(
        &mut self,
        key_var: &str,
        value_var: &str,
        map: &str,
        index_var: &str,
        body_block: &str,
        exit_block: &str,
        bb_map: &HashMap<String, inkwell::basic_block::BasicBlock<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let map_val = self.resolve_value(map);
        let map_ptr = map_val.into_pointer_value();

        // Allocate index
        let index_alloca = self
            .builder
            .build_alloca(self.context.i32_type(), index_var)
            .unwrap();
        let zero = self.context.i32_type().const_int(0, false);
        self.builder.build_store(index_alloca, zero).unwrap();

        self.symbols.insert(
            index_var.to_string(),
            Symbol {
                ptr: index_alloca,
                ty: self.context.i32_type().into(),
            },
        );

        // Get map length
        let map_len = self.get_map_length(map);

        // Get blocks
        let body_bb = bb_map.get(body_block).expect("Body block not found");
        let exit_bb = bb_map.get(exit_block).expect("Exit block not found");

        // Create condition block
        let current_func = self
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();
        let cond_bb = self
            .context
            .append_basic_block(current_func, "for.map.cond");

        // Enter loop context
        self.enter_loop(
            exit_block.to_string(),
            cond_bb.get_name().to_str().unwrap().to_string(),
        );
        self.add_loop_var(key_var.to_string());
        self.add_loop_var(value_var.to_string());

        // Jump to condition
        self.builder.build_unconditional_branch(cond_bb).unwrap();

        // Condition: index < map_len
        self.builder.position_at_end(cond_bb);
        let current_index = self
            .builder
            .build_load(self.context.i32_type(), index_alloca, "current_idx")
            .unwrap()
            .into_int_value();

        let cond = self
            .builder
            .build_int_compare(IntPredicate::SLT, current_index, map_len, "map_cond")
            .unwrap();

        self.builder
            .build_conditional_branch(cond, *body_bb, *exit_bb)
            .unwrap();

        // Allocate key and value variables
        let (key_type, val_type) = self.get_map_types(map);

        let key_alloca = self.builder.build_alloca(key_type, key_var).unwrap();
        let val_alloca = self.builder.build_alloca(val_type, value_var).unwrap();

        self.symbols.insert(
            key_var.to_string(),
            Symbol {
                ptr: key_alloca,
                ty: key_type,
            },
        );
        self.symbols.insert(
            value_var.to_string(),
            Symbol {
                ptr: val_alloca,
                ty: val_type,
            },
        );

        None
    }

    /// Generate infinite loop: for { }
    fn generate_for_infinite(
        &mut self,
        body_block: &str,
        bb_map: &HashMap<String, inkwell::basic_block::BasicBlock<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let body_bb = bb_map.get(body_block).expect("Body block not found");

        // For infinite loop, create a dummy exit block (unreachable unless break)
        let current_func = self
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();
        let exit_bb = self
            .context
            .append_basic_block(current_func, "for.inf.exit");

        // Enter loop context
        self.enter_loop(
            exit_bb.get_name().to_str().unwrap().to_string(),
            body_bb.get_name().to_str().unwrap().to_string(),
        );

        // Jump to body
        self.builder.build_unconditional_branch(*body_bb).unwrap();

        None
    }

    /// Generate loop increment (called after body execution for range loops)
    pub fn generate_loop_increment_and_branch(
        &mut self,
        var: &str,
        cond_bb: inkwell::basic_block::BasicBlock<'ctx>,
    ) {
        if let Some(symbol) = self.symbols.get(var) {
            let current = self
                .builder
                .build_load(self.context.i32_type(), symbol.ptr, "current")
                .unwrap()
                .into_int_value();

            let one = self.context.i32_type().const_int(1, false);
            let incremented = self
                .builder
                .build_int_add(current, one, "incremented")
                .unwrap();

            self.builder.build_store(symbol.ptr, incremented).unwrap();
        }

        // Jump back to condition check
        self.builder.build_unconditional_branch(cond_bb).unwrap();
    }

    /// Generate cleanup when exiting a loop
    pub fn generate_loop_exit_cleanup(&mut self) {
        // Get current loop context
        if let Some(loop_ctx) = self.exit_loop() {
            // Clean up any heap-allocated loop variables
            for var in &loop_ctx.loop_vars {
                if self.heap_strings.contains(var) {
                    self.emit_decref(var);
                    self.heap_strings.remove(var);
                }
                if self.heap_arrays.contains(var) {
                    // Clean up strings in array elements if needed
                    if let Some(str_ptrs) = self.composite_string_ptrs.get(var) {
                        for str_ptr in str_ptrs.clone() {
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
                    self.emit_decref(var);
                    self.heap_arrays.remove(var);
                }
                if self.heap_maps.contains(var) {
                    // Clean up strings in map if needed
                    if let Some(str_names) = self.composite_strings.get(var) {
                        for str_name in str_names.clone() {
                            if let Some(val) = self.temp_values.get(&str_name) {
                                if val.is_pointer_value() {
                                    let data_ptr = val.into_pointer_value();
                                    let rc_header = unsafe {
                                        self.builder.build_in_bounds_gep(
                                            self.context.i8_type(),
                                            data_ptr,
                                            &[self
                                                .context
                                                .i64_type()
                                                .const_int((-8_i64) as u64, true)],
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
                    }
                    self.emit_decref(var);
                    self.heap_maps.remove(var);
                }
            }
        }
    }

    /// Helper implementations for loop code
    pub fn get_array_length(&self, array_name: &str) -> inkwell::values::IntValue<'ctx> {
        if let Some(metadata) = self.array_metadata.get(array_name) {
            self.context
                .i32_type()
                .const_int(metadata.length as u64, false)
        } else {
            // Fallback: return 0
            self.context.i32_type().const_int(0, false)
        }
    }

    pub fn get_array_element_type(&self, array_name: &str) -> inkwell::types::BasicTypeEnum<'ctx> {
        if let Some(metadata) = self.array_metadata.get(array_name) {
            match metadata.element_type.as_str() {
                "Int" => self.context.i32_type().into(),
                "Bool" => self.context.bool_type().into(),
                "Str" => self
                    .context
                    .i8_type()
                    .ptr_type(AddressSpace::default())
                    .into(),
                _ => self.context.i32_type().into(),
            }
        } else {
            self.context.i32_type().into()
        }
    }

    pub fn get_map_length(&self, map_name: &str) -> inkwell::values::IntValue<'ctx> {
        if let Some(metadata) = self.map_metadata.get(map_name) {
            self.context
                .i32_type()
                .const_int(metadata.length as u64, false)
        } else {
            self.context.i32_type().const_int(0, false)
        }
    }

    pub fn get_map_types(
        &self,
        map_name: &str,
    ) -> (
        inkwell::types::BasicTypeEnum<'ctx>,
        inkwell::types::BasicTypeEnum<'ctx>,
    ) {
        if let Some(metadata) = self.map_metadata.get(map_name) {
            let key_type = match metadata.key_type.as_str() {
                "Int" => self.context.i32_type().into(),
                "Bool" => self.context.bool_type().into(),
                "Str" => self
                    .context
                    .i8_type()
                    .ptr_type(AddressSpace::default())
                    .into(),
                _ => self.context.i32_type().into(),
            };

            let val_type = match metadata.value_type.as_str() {
                "Int" => self.context.i32_type().into(),
                "Bool" => self.context.bool_type().into(),
                "Str" => self
                    .context
                    .i8_type()
                    .ptr_type(AddressSpace::default())
                    .into(),
                _ => self.context.i32_type().into(),
            };

            (key_type, val_type)
        } else {
            (
                self.context.i32_type().into(),
                self.context.i32_type().into(),
            )
        }
    }

    pub fn get_map_pair_type(&self, map_name: &str) -> StructType<'ctx> {
        let (key_type, val_type) = self.get_map_types(map_name);
        self.context.struct_type(&[key_type, val_type], false)
    }

    pub fn map_contains_strings(&self, map_name: &str) -> (bool, bool) {
        if let Some(metadata) = self.map_metadata.get(map_name) {
            (metadata.key_is_string, metadata.value_is_string)
        } else {
            (false, false)
        }
    }

    pub fn array_contains_strings(&self, array_name: &str) -> bool {
        if let Some(metadata) = self.array_metadata.get(array_name) {
            metadata.contains_strings
        } else {
            false
        }
    }

    pub fn load_array_element_with_rc(
        &mut self,
        array_ptr: inkwell::values::PointerValue<'ctx>,
        index: inkwell::values::IntValue<'ctx>,
        elem_type: inkwell::types::BasicTypeEnum<'ctx>,
        is_string: bool,
    ) -> BasicValueEnum<'ctx> {
        // GEP to get element pointer
        let elem_ptr = unsafe {
            self.builder.build_gep(
                elem_type.array_type(0), // We need actual array type here
                array_ptr,
                &[self.context.i32_type().const_zero(), index],
                "elem_ptr",
            )
        }
        .unwrap();

        // Load element
        let elem_val = self
            .builder
            .build_load(elem_type, elem_ptr, "elem")
            .unwrap();

        // If it's a heap-allocated string, increment RC
        if is_string {
            let str_ptr = elem_val.into_pointer_value();
            let rc_header = unsafe {
                self.builder.build_in_bounds_gep(
                    self.context.i8_type(),
                    str_ptr,
                    &[self.context.i64_type().const_int((-8_i64) as u64, true)],
                    "rc_header",
                )
            }
            .unwrap();

            let incref = self.incref_fn.unwrap();
            self.builder
                .build_call(incref, &[rc_header.into()], "")
                .unwrap();
        }

        elem_val
    }

    /// Extract map key-value pair with RC handling
    pub fn load_map_pair_with_rc(
        &mut self,
        map_ptr: inkwell::values::PointerValue<'ctx>,
        index: inkwell::values::IntValue<'ctx>,
        pair_type: inkwell::types::StructType<'ctx>,
        key_is_string: bool,
        val_is_string: bool,
    ) -> (BasicValueEnum<'ctx>, BasicValueEnum<'ctx>) {
        // GEP to get pair pointer
        let pair_ptr = unsafe {
            self.builder.build_gep(
                pair_type.array_type(0),
                map_ptr,
                &[self.context.i32_type().const_zero(), index],
                "pair_ptr",
            )
        }
        .unwrap();

        // Extract key (field 0)
        let key_ptr = self
            .builder
            .build_struct_gep(pair_type, pair_ptr, 0, "key_ptr")
            .unwrap();
        let key_val = self
            .builder
            .build_load(
                pair_type.get_field_type_at_index(0).unwrap(),
                key_ptr,
                "key",
            )
            .unwrap();

        // Extract value (field 1)
        let val_ptr = self
            .builder
            .build_struct_gep(pair_type, pair_ptr, 1, "val_ptr")
            .unwrap();
        let val_val = self
            .builder
            .build_load(
                pair_type.get_field_type_at_index(1).unwrap(),
                val_ptr,
                "val",
            )
            .unwrap();

        // Handle RC for strings
        if key_is_string {
            let str_ptr = key_val.into_pointer_value();
            let rc_header = unsafe {
                self.builder.build_in_bounds_gep(
                    self.context.i8_type(),
                    str_ptr,
                    &[self.context.i64_type().const_int((-8_i64) as u64, true)],
                    "rc_header",
                )
            }
            .unwrap();
            let incref = self.incref_fn.unwrap();
            self.builder
                .build_call(incref, &[rc_header.into()], "")
                .unwrap();
        }

        if val_is_string {
            let str_ptr = val_val.into_pointer_value();
            let rc_header = unsafe {
                self.builder.build_in_bounds_gep(
                    self.context.i8_type(),
                    str_ptr,
                    &[self.context.i64_type().const_int((-8_i64) as u64, true)],
                    "rc_header",
                )
            }
            .unwrap();
            let incref = self.incref_fn.unwrap();
            self.builder
                .build_call(incref, &[rc_header.into()], "")
                .unwrap();
        }

        (key_val, val_val)
    }
}
