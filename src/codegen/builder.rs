use crate::codegen::{CodeGen, Symbol};
use crate::mir::MirInstr;
use inkwell::types::BasicType;
use inkwell::types::BasicTypeEnum;
use inkwell::types::StructType;
use inkwell::values::BasicValue;
use inkwell::values::BasicValueEnum;
use inkwell::values::FunctionValue;
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
                // If this temp was pre-allocated as a symbol (cross-block usage), store it there
                if let Some(sym) = self.symbols.get(name) {
                    self.builder.build_store(sym.ptr, val).unwrap();
                }
                self.temp_values.insert(name.clone(), val.into());
                Some(val.into())
            }

            MirInstr::ConstBool { name, value } => {
                // Use i32 instead of i1 for consistency with rest of codegen
                let val = self.context.i32_type().const_int(*value as u64, false);
                // If this temp was pre-allocated as a symbol (cross-block usage), store it there
                if let Some(sym) = self.symbols.get(name) {
                    self.builder.build_store(sym.ptr, val).unwrap();
                }
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

                let rc_ptr = self
                    .builder
                    .build_pointer_cast(
                        heap_ptr,
                        self.context.ptr_type(AddressSpace::default()),
                        "rc_ptr",
                    )
                    .unwrap();
                self.builder
                    .build_store(rc_ptr, self.context.i32_type().const_int(1, false))
                    .unwrap();

                let data_ptr = unsafe {
                    self.builder
                        .build_gep(
                            self.context.i8_type(),
                            heap_ptr,
                            &[self.context.i32_type().const_int(8, false)],
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

            // String concatenation with proper RC handling
            MirInstr::StringConcat { name, left, right } => {
                let left_ptr = self.resolve_value(left).into_pointer_value();
                let right_ptr = self.resolve_value(right).into_pointer_value();

                // Get strlen function or implement inline length calculation
                let strlen_fn = self.get_or_declare_strlen();

                // Get lengths of both strings
                let left_len = self
                    .builder
                    .build_call(strlen_fn, &[left_ptr.into()], "left_len")
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_int_value();

                let right_len = self
                    .builder
                    .build_call(strlen_fn, &[right_ptr.into()], "right_len")
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_int_value();

                // Calculate total length: left_len + right_len + 1 (null terminator) + 8 (RC header)
                let total_len = self
                    .builder
                    .build_int_add(left_len, right_len, "partial_len")
                    .unwrap();
                let total_len_plus_null = self
                    .builder
                    .build_int_add(
                        total_len,
                        self.context.i32_type().const_int(1, false),
                        "len_with_null",
                    )
                    .unwrap();
                let total_size = self
                    .builder
                    .build_int_add(
                        total_len_plus_null,
                        self.context.i32_type().const_int(8, false),
                        "total_size",
                    )
                    .unwrap();

                // Allocate memory for RC header + concatenated string
                let malloc_fn = self.get_or_declare_malloc();
                let heap_ptr = self
                    .builder
                    .build_call(malloc_fn, &[total_size.into()], "concat_heap")
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_pointer_value();

                // Set RC to 1
                let rc_ptr = self
                    .builder
                    .build_pointer_cast(
                        heap_ptr,
                        self.context.ptr_type(AddressSpace::default()),
                        "rc_ptr",
                    )
                    .unwrap();
                self.builder
                    .build_store(rc_ptr, self.context.i32_type().const_int(1, false))
                    .unwrap();

                // Get data pointer (8 bytes after RC header)
                let data_ptr = unsafe {
                    self.builder.build_gep(
                        self.context.i8_type(),
                        heap_ptr,
                        &[self.context.i32_type().const_int(8, false)],
                        "data_ptr",
                    )
                }
                .unwrap();

                // Copy left string to data_ptr
                let memcpy_fn = self.get_or_declare_memcpy();
                let left_len_i64 = self
                    .builder
                    .build_int_cast(left_len, self.context.i64_type(), "left_len_i64")
                    .unwrap();
                self.builder
                    .build_call(
                        memcpy_fn,
                        &[
                            data_ptr.into(),
                            left_ptr.into(),
                            left_len_i64.into(),
                            self.context.bool_type().const_zero().into(),
                        ],
                        "",
                    )
                    .unwrap();

                // Copy right string to data_ptr + left_len
                let right_dest = unsafe {
                    self.builder.build_gep(
                        self.context.i8_type(),
                        data_ptr,
                        &[left_len],
                        "right_dest",
                    )
                }
                .unwrap();

                let right_len_i64 = self
                    .builder
                    .build_int_cast(right_len, self.context.i64_type(), "right_len_i64")
                    .unwrap();
                self.builder
                    .build_call(
                        memcpy_fn,
                        &[
                            right_dest.into(),
                            right_ptr.into(),
                            right_len_i64.into(),
                            self.context.bool_type().const_zero().into(),
                        ],
                        "",
                    )
                    .unwrap();

                // Add null terminator
                let null_pos = unsafe {
                    self.builder.build_gep(
                        self.context.i8_type(),
                        data_ptr,
                        &[total_len],
                        "null_pos",
                    )
                }
                .unwrap();

                self.builder
                    .build_store(null_pos, self.context.i8_type().const_zero())
                    .unwrap();

                // Store result and mark as heap string
                self.temp_values.insert(name.clone(), data_ptr.into());
                self.heap_strings.insert(name.clone());

                Some(data_ptr.into())
            }

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
                        self.context.ptr_type(AddressSpace::default()),
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
                            &[self.context.i32_type().const_int((-8_i32) as u64, true)],
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
                        self.context.ptr_type(AddressSpace::default()),
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
                            &[self.context.i32_type().const_int((-8_i32) as u64, true)],
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
                            &[self.context.i32_type().const_int((-8_i32) as u64, true)],
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

                // Store in temp_values for immediate use
                self.temp_values.insert(dst.clone(), res.into());

                // If this temp was pre-allocated as a symbol (cross-block usage), store it there too
                if let Some(sym) = self.symbols.get(dst) {
                    self.builder.build_store(sym.ptr, res).unwrap();
                }

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
                                        &[self.context.i32_type().const_int((-8_i32) as u64, true)],
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
                        // Only incref when copying from an existing variable (not from a temp)
                        if self.symbols.contains_key(value) {
                            self.emit_incref(name);
                        }
                    } else if value_is_heap_array {
                        self.heap_arrays.insert(name.clone());
                        // Only incref when copying from an existing variable (not from a temp)
                        if self.symbols.contains_key(value) {
                            self.emit_incref(name);
                        }

                        // Copy array metadata on re-assignment - ENHANCED
                        // CRITICAL: Try ALL possible ways to find the metadata
                        let mut found_metadata = self.array_metadata.get(value).cloned();

                        // If not found directly, search through ALL array metadata by pointer equality
                        if found_metadata.is_none() {
                            if let Some(val_ptr_value) = self.temp_values.get(value) {
                                if val_ptr_value.is_pointer_value() {
                                    let val_ptr = val_ptr_value.into_pointer_value();
                                    let array_metadata_clone = self.array_metadata.clone();
                                    for (meta_name, metadata) in &array_metadata_clone {
                                        if let Some(meta_val) = self.temp_values.get(meta_name) {
                                            if meta_val.is_pointer_value()
                                                && meta_val.into_pointer_value() == val_ptr
                                            {
                                                found_metadata = Some(metadata.clone());
                                                eprintln!(
                                                    "[DEBUG] Re-assignment: Found metadata via temp_values pointer match: '{}' -> '{}' (length: {})",
                                                    value, meta_name, metadata.length
                                                );
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // LAST RESORT: Try to extract array length from LLVM type
                        if found_metadata.is_none() {
                            if let Some(sym) = self.symbols.get(value) {
                                if let Ok(loaded) =
                                    self.builder
                                        .build_load(sym.ty, sym.ptr, "extract_array_len")
                                {
                                    if loaded.is_pointer_value() {
                                        let array_ptr = loaded.into_pointer_value();
                                        let ptr_type = array_ptr.get_type();

                                        // Try to determine element type and count
                                        // This is a heuristic - we assume string arrays if we can't find metadata
                                        let element_type = if self.heap_strings.contains(value) {
                                            "Str"
                                        } else {
                                            "Int"
                                        };

                                        // For dynamically allocated arrays, try to infer size from usage
                                        // Check if there are any GEP instructions that accessed this array
                                        let mut max_index = 0;
                                        for (check_name, check_val) in &self.temp_values {
                                            if check_name.contains(value)
                                                && check_name.contains("elem")
                                            {
                                                // Found an element access, try to extract index
                                                if let Some(idx_part) =
                                                    check_name.split("elem_").last()
                                                {
                                                    if let Some(idx) =
                                                        idx_part.chars().next().and_then(|c| {
                                                            c.to_string().parse::<usize>().ok()
                                                        })
                                                    {
                                                        max_index = max_index.max(idx);
                                                    }
                                                }
                                            }
                                        }

                                        if max_index > 0 {
                                            found_metadata = Some(crate::codegen::ArrayMetadata {
                                                length: max_index + 1,
                                                element_type: element_type.to_string(),
                                                contains_strings: element_type == "Str",
                                            });
                                            eprintln!(
                                                "[DEBUG] Re-assignment: Extracted array length from usage pattern: '{}' has length {} (inferred)",
                                                value, max_index + 1
                                            );
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(metadata) = found_metadata {
                            // Register under EXTENSIVE variations
                            let base_name = name.trim_start_matches('%').trim_end_matches("_array");
                            let name_variations = vec![
                                name.to_string(),
                                name.trim_end_matches("_array").to_string(),
                                name.trim_start_matches('%').to_string(),
                                format!("{}_array", name),
                                format!("{}_array", base_name),
                                base_name.to_string(),
                                format!("{}item_array", base_name),
                                format!("{}item", base_name),
                            ];

                            eprintln!(
                                "[DEBUG] Re-assignment: Copying array metadata from '{}' to '{}' (length: {})",
                                value, name, metadata.length
                            );

                            for variation in name_variations {
                                self.array_metadata.insert(variation, metadata.clone());
                            }
                        } else {
                            // Try to find metadata by checking if value points to a known array
                            eprintln!(
                                "[DEBUG] Re-assignment: No direct metadata for '{}', trying propagate_metadata",
                                value
                            );
                            self.propagate_metadata(name, value);
                        }
                    } else if value_is_heap_map {
                        self.heap_maps.insert(name.clone());
                        // Only incref when copying from an existing variable (not from a temp)
                        if self.symbols.contains_key(value) {
                            self.emit_incref(name);
                        }

                        // Copy map metadata on re-assignment
                        if let Some(metadata) = self.map_metadata.get(value).cloned() {
                            self.map_metadata.insert(name.clone(), metadata);
                        } else {
                            // Try to find metadata by checking if value points to a known map
                            self.propagate_metadata(name, value);
                        }
                    } else {
                        // Even for non-heap reassignments, try to propagate metadata
                        // This handles cases like: inneritem_array = innerarr (both ptrs)
                        self.propagate_metadata(name, value);
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

                        // Copy array metadata - ENHANCED for dynamic arrays
                        // CRITICAL: Try ALL possible ways to find the metadata
                        let mut found_metadata = self.array_metadata.get(value).cloned();

                        // If not found directly, search through ALL array metadata by pointer equality
                        if found_metadata.is_none() {
                            if let Some(val_ptr_value) = self.temp_values.get(value) {
                                if val_ptr_value.is_pointer_value() {
                                    let val_ptr = val_ptr_value.into_pointer_value();
                                    let array_metadata_clone = self.array_metadata.clone();
                                    for (meta_name, metadata) in &array_metadata_clone {
                                        if let Some(meta_val) = self.temp_values.get(meta_name) {
                                            if meta_val.is_pointer_value()
                                                && meta_val.into_pointer_value() == val_ptr
                                            {
                                                found_metadata = Some(metadata.clone());
                                                eprintln!(
                                                    "[DEBUG] Found metadata via temp_values pointer match: '{}' -> '{}' (length: {})",
                                                    value, meta_name, metadata.length
                                                );
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // LAST RESORT: Try to extract array length from LLVM value directly
                        if found_metadata.is_none() {
                            // Check if the value itself has array information
                            let element_type = if value.contains("str") || value.contains("Str") {
                                "Str"
                            } else {
                                "Int"
                            };

                            // Try to infer from element count in temp_values
                            let mut elem_count = 0;
                            for (temp_name, _) in &self.temp_values {
                                if temp_name.starts_with(&format!("{}_elem_", value))
                                    || temp_name.contains(&format!("{}[", value))
                                {
                                    elem_count += 1;
                                }
                            }

                            if elem_count > 0 {
                                found_metadata = Some(crate::codegen::ArrayMetadata {
                                    length: elem_count,
                                    element_type: element_type.to_string(),
                                    contains_strings: element_type == "Str",
                                });
                                eprintln!(
                                    "[DEBUG] Initial assignment: Inferred array length from temp_values: '{}' has length {} (inferred)",
                                    value, elem_count
                                );
                            }
                        }

                        if let Some(metadata) = found_metadata {
                            // Register under EXTENSIVE variations
                            let base_name = name.trim_start_matches('%').trim_end_matches("_array");
                            let name_variations = vec![
                                name.to_string(),
                                name.trim_end_matches("_array").to_string(),
                                name.trim_start_matches('%').to_string(),
                                format!("{}_array", name),
                                format!("{}_array", base_name),
                                base_name.to_string(),
                                format!("{}item_array", base_name),
                                format!("{}item", base_name),
                            ];

                            eprintln!(
                                "[DEBUG] Propagating array metadata from '{}' to '{}' (length: {})",
                                value, name, metadata.length
                            );

                            for variation in name_variations {
                                self.array_metadata.insert(variation, metadata.clone());
                            }
                        } else {
                            // Try to find metadata by checking if value points to a known array
                            eprintln!(
                                "[DEBUG] No direct metadata found for '{}', trying propagate_metadata",
                                value
                            );
                            self.propagate_metadata(name, value);
                        }
                    } else if value_is_heap_map {
                        self.heap_maps.insert(name.clone());
                        if self.symbols.contains_key(value) {
                            self.emit_incref(name);
                        }

                        // Copy map metadata
                        if let Some(metadata) = self.map_metadata.get(value).cloned() {
                            self.map_metadata.insert(name.clone(), metadata);
                        } else {
                            // Try to find metadata by checking if value points to a known map
                            self.propagate_metadata(name, value);
                        }
                    } else {
                        // Even for initial non-heap assignments, try to propagate metadata
                        // This is critical for variables that store pointers
                        self.propagate_metadata(name, value);
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

            MirInstr::ArrayLen { name, array } => {
                let array_name = array;

                // ALWAYS generate runtime length extraction from heap header
                // This ensures correct behavior for arrays created dynamically in loops
                eprintln!(
                    "[DEBUG] ArrayLen: Generating runtime extraction for '{}'",
                    array_name
                );

                // First, try static metadata for optimization (constant folding)
                if let Some(metadata) = self.array_metadata.get(array_name) {
                    eprintln!(
                        "[DEBUG] ArrayLen: Found static metadata for '{}', length = {}",
                        array_name, metadata.length
                    );
                    let len_val = self
                        .context
                        .i32_type()
                        .const_int(metadata.length as u64, false);
                    self.temp_values.insert(name.clone(), len_val.into());
                    if let Some(sym) = self.symbols.get(name) {
                        self.builder.build_store(sym.ptr, len_val).unwrap();
                    }
                    return Some(len_val.into());
                }

                if let Some(metadata) = self.map_metadata.get(array_name) {
                    eprintln!(
                        "[DEBUG] ArrayLen: Found static map metadata for '{}', length = {}",
                        array_name, metadata.length
                    );
                    let len_val = self
                        .context
                        .i32_type()
                        .const_int(metadata.length as u64, false);
                    self.temp_values.insert(name.clone(), len_val.into());
                    if let Some(sym) = self.symbols.get(name) {
                        self.builder.build_store(sym.ptr, len_val).unwrap();
                    }
                    return Some(len_val.into());
                }

                // No static metadata - generate runtime extraction
                eprintln!(
                    "[DEBUG] ArrayLen: No static metadata for '{}', generating runtime extraction",
                    array_name
                );

                // Try to get the array pointer from temp_values or symbols
                let array_ptr_opt = if let Some(val) = self.temp_values.get(array_name) {
                    if val.is_pointer_value() {
                        Some(val.into_pointer_value())
                    } else {
                        None
                    }
                } else if let Some(sym) = self.symbols.get(array_name) {
                    if let Ok(loaded) =
                        self.builder
                            .build_load(sym.ty, sym.ptr, &format!("{}_ptr", array_name))
                    {
                        if loaded.is_pointer_value() {
                            Some(loaded.into_pointer_value())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(array_ptr) = array_ptr_opt {
                    // Generate runtime length extraction
                    // Array layout: [RC: 4 bytes][Length: 4 bytes][data at offset 8]
                    // array_ptr points to data (offset 8), so length is at offset -4
                    eprintln!(
                        "[DEBUG] ArrayLen: Extracting runtime length from heap header for '{}'",
                        array_name
                    );

                    let len_ptr_result = unsafe {
                        self.builder.build_in_bounds_gep(
                            self.context.i8_type(),
                            array_ptr,
                            &[self.context.i32_type().const_int((-4_i32) as u64, true)],
                            &format!("{}_len_ptr", array_name),
                        )
                    };

                    if let Ok(len_ptr) = len_ptr_result {
                        let len_ptr_cast_result = self.builder.build_pointer_cast(
                            len_ptr,
                            self.context.ptr_type(inkwell::AddressSpace::default()),
                            &format!("{}_len_cast", array_name),
                        );

                        if let Ok(len_ptr_cast) = len_ptr_cast_result {
                            if let Ok(runtime_len) = self.builder.build_load(
                                self.context.i32_type(),
                                len_ptr_cast,
                                &format!("{}_runtime_len", array_name),
                            ) {
                                eprintln!(
                                    "[SUCCESS] ArrayLen: Runtime length extracted for '{}'",
                                    array_name
                                );
                                let len_val = runtime_len.into_int_value();
                                self.temp_values.insert(name.clone(), len_val.into());
                                if let Some(sym) = self.symbols.get(name) {
                                    self.builder.build_store(sym.ptr, len_val).unwrap();
                                }
                                return Some(len_val.into());
                            }
                        }
                    }
                }

                // Last resort: return 0 (loop will skip)
                eprintln!(
                    "[ERROR] ArrayLen: Could not extract length for '{}', returning 0",
                    array_name
                );
                let len_val = self.context.i32_type().const_int(0, false);
                self.temp_values.insert(name.clone(), len_val.into());
                if let Some(sym) = self.symbols.get(name) {
                    self.builder.build_store(sym.ptr, len_val).unwrap();
                }
                Some(len_val.into())
            }

            MirInstr::ArrayGet { name, array, index } => {
                let array_ptr = self.resolve_value(array).into_pointer_value();
                let index_val = self.resolve_value(index).into_int_value();

                // Track that this ArrayGet result came from this source array
                self.arrayget_sources.insert(name.clone(), array.clone());

                // Check if this is actually a map iteration (map metadata exists for this array)
                if let Some(map_metadata) = self.map_metadata.get(array) {
                    // This is a map being iterated as an array - extract the key-value pair
                    let (key_type, val_type) = self.get_map_types(array);
                    let pair_type = self.context.struct_type(&[key_type, val_type], false);
                    let map_len = map_metadata.length as u32;
                    // Use direct pointer arithmetic with single index for runtime maps
                    // This is clearer and more explicit than the two-index array syntax
                    let pair_ptr = unsafe {
                        self.builder.build_in_bounds_gep(
                            pair_type,
                            array_ptr,
                            &[index_val],
                            "pair_ptr",
                        )
                    }
                    .unwrap();

                    // Return the pair pointer so TupleGet can extract key/value
                    // Store the pair pointer in temp_values
                    self.temp_values.insert(name.clone(), pair_ptr.into());

                    // If this temp was pre-allocated as a symbol, store it there too
                    if let Some(sym) = self.symbols.get(name) {
                        self.builder.build_store(sym.ptr, pair_ptr).unwrap();
                    }

                    // Return the pair pointer for subsequent TupleGet operations
                    return Some(pair_ptr.into());
                }

                // Normal array element access
                let elem_type = self.get_array_element_type(array);

                // Use direct pointer arithmetic with single index for runtime arrays
                // This is clearer and more explicit than the two-index array syntax
                let elem_ptr = unsafe {
                    self.builder
                        .build_in_bounds_gep(elem_type, array_ptr, &[index_val], "elem_ptr")
                }
                .unwrap();

                // Load the element
                let elem_val = self
                    .builder
                    .build_load(elem_type, elem_ptr, "elem_val")
                    .unwrap();

                // Store in temp_values for immediate use
                self.temp_values.insert(name.clone(), elem_val);

                // If this temp was pre-allocated as a symbol (cross-block usage), store it there too
                if let Some(sym) = self.symbols.get(name) {
                    self.builder.build_store(sym.ptr, elem_val).unwrap();
                }

                // Track if this is a heap-allocated value and increment RC
                if elem_type.is_pointer_type() && self.array_contains_strings(array) {
                    self.heap_strings.insert(name.clone());

                    // Increment reference count when loading a string from an array
                    // This is critical for loop iterations where the same variable is reused
                    let str_ptr = elem_val.into_pointer_value();
                    let rc_header = unsafe {
                        self.builder.build_in_bounds_gep(
                            self.context.i8_type(),
                            str_ptr,
                            &[self.context.i32_type().const_int((-8_i32) as u64, true)],
                            "rc_header",
                        )
                    }
                    .unwrap();

                    let incref = self.incref_fn.unwrap();
                    self.builder
                        .build_call(incref, &[rc_header.into()], "")
                        .unwrap();
                }

                Some(elem_val)
            }

            MirInstr::TupleGet { name, tuple, index } => {
                // Get the tuple/pair value (should be a pointer to a pair struct from ArrayGet)
                let tuple_val = self.resolve_value(tuple);

                if !tuple_val.is_pointer_value() {
                    // Not a pointer - return a dummy value
                    let dummy = self.context.i32_type().const_int(0, false);
                    self.temp_values.insert(name.clone(), dummy.into());
                    return Some(dummy.into());
                }

                let pair_ptr = tuple_val.into_pointer_value();

                // Find the map metadata by looking up the tuple source variable
                // The tuple variable comes from ArrayGet, which should have map metadata
                let mut found_metadata: Option<&crate::codegen::MapMetadata> = None;
                let mut search_log: Vec<String> = Vec::new();

                // Strategy 1: Look up the source array from ArrayGet tracking
                if let Some(source_array) = self.arrayget_sources.get(tuple) {
                    search_log.push(format!("Strategy 1: ArrayGet source = '{}'", source_array));
                    if let Some(metadata) = self.map_metadata.get(source_array) {
                        found_metadata = Some(metadata);
                        search_log.push(format!(
                            "  ✓ Found metadata for '{}': {}:{}",
                            source_array, metadata.key_type, metadata.value_type
                        ));
                    } else {
                        search_log.push(format!("  ✗ No metadata for '{}'", source_array));
                    }
                }

                // Strategy 2: Try to find metadata directly from the tuple variable name
                if found_metadata.is_none() {
                    search_log.push(format!("Strategy 2: Direct lookup for '{}'", tuple));
                    if let Some(metadata) = self.map_metadata.get(tuple) {
                        found_metadata = Some(metadata);
                        search_log.push(format!(
                            "  ✓ Found metadata: {}:{}",
                            metadata.key_type, metadata.value_type
                        ));
                    } else {
                        search_log.push("  ✗ Not found".to_string());
                    }
                }

                // Strategy 3: Try removing "_array" suffix (e.g., "%45_array" -> "%45")
                if found_metadata.is_none() {
                    let base_name = tuple.trim_end_matches("_array");
                    if base_name != tuple {
                        search_log.push(format!("Strategy 3: Try base name '{}'", base_name));
                        if let Some(metadata) = self.map_metadata.get(base_name) {
                            found_metadata = Some(metadata);
                            search_log.push(format!(
                                "  ✓ Found metadata: {}:{}",
                                metadata.key_type, metadata.value_type
                            ));
                        } else {
                            search_log.push("  ✗ Not found".to_string());
                        }
                    }
                }

                // Strategy 4: Try adding "_array" suffix (e.g., "map1" -> "map1_array")
                if found_metadata.is_none() {
                    let array_name = format!("{}_array", tuple);
                    search_log.push(format!(
                        "Strategy 4: Try with _array suffix '{}'",
                        array_name
                    ));
                    if let Some(metadata) = self.map_metadata.get(&array_name) {
                        found_metadata = Some(metadata);
                        search_log.push(format!(
                            "  ✓ Found metadata: {}:{}",
                            metadata.key_type, metadata.value_type
                        ));
                    } else {
                        search_log.push("  ✗ Not found".to_string());
                    }
                }

                // Strategy 5: Search for any map name that matches or contains this variable
                if found_metadata.is_none() {
                    search_log
                        .push("Strategy 5: Fuzzy search through all map metadata".to_string());
                    for (map_name, metadata) in &self.map_metadata {
                        let tuple_clean = tuple.trim_start_matches('%');
                        let map_clean = map_name.trim_start_matches('%');

                        if map_clean.contains(tuple_clean) || tuple_clean.contains(map_clean) {
                            found_metadata = Some(metadata);
                            search_log.push(format!(
                                "  ✓ Fuzzy match: '{}' contains '{}'",
                                map_name, tuple
                            ));
                            search_log.push(format!(
                                "    Metadata: {}:{}",
                                metadata.key_type, metadata.value_type
                            ));
                            break;
                        }
                    }
                    if found_metadata.is_none() {
                        search_log.push("  ✗ No fuzzy matches found".to_string());
                    }
                }

                let (key_type, val_type, key_is_string, val_is_string) =
                    if let Some(metadata) = found_metadata {
                        let k_type = match metadata.key_type.as_str() {
                            "Str" => self
                                .context
                                .ptr_type(inkwell::AddressSpace::default())
                                .into(),
                            "Int" => self.context.i32_type().into(),
                            "Bool" => self.context.bool_type().into(),
                            _ => self.context.i32_type().into(),
                        };
                        let v_type = match metadata.value_type.as_str() {
                            "Str" => self
                                .context
                                .ptr_type(inkwell::AddressSpace::default())
                                .into(),
                            "Int" => self.context.i32_type().into(),
                            "Bool" => self.context.bool_type().into(),
                            _ => self.context.i32_type().into(),
                        };
                        (
                            k_type,
                            v_type,
                            metadata.key_is_string,
                            metadata.value_is_string,
                        )
                    } else {
                        // No metadata found - this is a critical error
                        eprintln!(
                        "\n╔════════════════════════════════════════════════════════════════════╗"
                    );
                        eprintln!(
                            "║ ERROR: Map metadata lookup failed in TupleGet                     ║"
                        );
                        eprintln!(
                        "╚════════════════════════════════════════════════════════════════════╝"
                    );
                        eprintln!("\n📍 Context:");
                        eprintln!("  • Variable name: '{}'", name);
                        eprintln!("  • Tuple variable: '{}'", tuple);
                        eprintln!("  • Tuple index: {}", index);

                        eprintln!("\n🔍 Search attempts:");
                        for log in &search_log {
                            eprintln!("  {}", log);
                        }

                        eprintln!("\n📊 Available map metadata:");
                        if self.map_metadata.is_empty() {
                            eprintln!("  (none)");
                        } else {
                            for (key, meta) in &self.map_metadata {
                                eprintln!(
                                    "  • '{}' → {{{}:{}}}, length={}",
                                    key, meta.key_type, meta.value_type, meta.length
                                );
                            }
                        }

                        eprintln!("\n🔗 ArrayGet source tracking:");
                        if self.arrayget_sources.is_empty() {
                            eprintln!("  (none)");
                        } else {
                            for (result, source) in &self.arrayget_sources {
                                eprintln!("  • '{}' ← ArrayGet from '{}'", result, source);
                            }
                        }

                        eprintln!("\n💡 Possible causes:");
                        eprintln!("  1. Map metadata not propagated to iteration variable");
                        eprintln!("  2. ArrayGet tracking not capturing the source array");
                        eprintln!("  3. Variable name mismatch between MIR and codegen");
                        eprintln!(
                        "\n═══════════════════════════════════════════════════════════════════\n"
                    );

                        // Return dummy values to avoid crash, but this will produce incorrect IR
                        let dummy = self.context.i32_type().const_int(0, false);
                        self.temp_values.insert(name.clone(), dummy.into());
                        return Some(dummy.into());
                    };

                // Reconstruct the pair struct type
                let pair_type = self.context.struct_type(&[key_type, val_type], false);

                // Extract the field using struct_gep
                let field_ptr = self
                    .builder
                    .build_struct_gep(pair_type, pair_ptr, *index as u32, &format!("{}_ptr", name))
                    .unwrap();

                // Load the field value
                let field_type = if *index == 0 { key_type } else { val_type };
                let is_string_field = if *index == 0 {
                    key_is_string
                } else {
                    val_is_string
                };

                let field_val = self
                    .builder
                    .build_load(field_type, field_ptr, name)
                    .unwrap();

                // Store in temp_values
                self.temp_values.insert(name.clone(), field_val);

                // Store into existing symbol (allocated by generate_for_map)
                // or create a new one if this is not a loop variable
                if let Some(sym) = self.symbols.get(name) {
                    // Symbol already exists (e.g., loop variable) - reuse it

                    // For map iteration variables, decref old value before storing new one
                    // This prevents memory leaks in loop iterations
                    if is_string_field {
                        // Check if we're in a map loop by checking loop stack
                        let in_map_loop = self.loop_stack.last().map_or(false, |ctx| {
                            matches!(ctx.loop_type, Some(crate::codegen::LoopType::Map { .. }))
                        });

                        if in_map_loop {
                            // Load the old value to check if it needs cleanup
                            let old_val = self
                                .builder
                                .build_load(field_type, sym.ptr, &format!("{}_old", name))
                                .unwrap();

                            if old_val.is_pointer_value() {
                                let old_ptr = old_val.into_pointer_value();

                                // Check if pointer is not null before decref
                                let null_ptr = field_type.into_pointer_type().const_null();
                                let old_int = self
                                    .builder
                                    .build_ptr_to_int(old_ptr, self.context.i64_type(), "old_int")
                                    .unwrap();
                                let null_int = self
                                    .builder
                                    .build_ptr_to_int(null_ptr, self.context.i64_type(), "null_int")
                                    .unwrap();
                                let is_not_null = self
                                    .builder
                                    .build_int_compare(
                                        inkwell::IntPredicate::NE,
                                        old_int,
                                        null_int,
                                        "is_not_null",
                                    )
                                    .unwrap();

                                let current_bb = self.builder.get_insert_block().unwrap();
                                let func = current_bb.get_parent().unwrap();
                                let decref_bb = self.context.append_basic_block(func, "decref_old");
                                let store_bb = self.context.append_basic_block(func, "store_new");

                                self.builder
                                    .build_conditional_branch(is_not_null, decref_bb, store_bb)
                                    .unwrap();

                                // Decref old value
                                self.builder.position_at_end(decref_bb);
                                let rc_header = unsafe {
                                    self.builder.build_in_bounds_gep(
                                        self.context.i8_type(),
                                        old_ptr,
                                        &[self.context.i32_type().const_int((-8_i32) as u64, true)],
                                        &format!("{}_old_rc", name),
                                    )
                                }
                                .unwrap();

                                let decref_fn = self.decref_fn.unwrap();
                                self.builder
                                    .build_call(decref_fn, &[rc_header.into()], "")
                                    .unwrap();

                                self.builder.build_unconditional_branch(store_bb).unwrap();

                                // Continue with store
                                self.builder.position_at_end(store_bb);
                            }
                        }
                    }

                    self.builder.build_store(sym.ptr, field_val).unwrap();
                } else {
                    // Symbol doesn't exist - create new alloca in ENTRY BLOCK
                    let current_insert_block = self.builder.get_insert_block().unwrap();
                    let func = current_insert_block.get_parent().unwrap();
                    let entry_block = func.get_first_basic_block().unwrap();

                    // Position at the END of entry block to create alloca
                    if let Some(terminator) = entry_block.get_terminator() {
                        self.builder.position_before(&terminator);
                    } else {
                        self.builder.position_at_end(entry_block);
                    }

                    let alloca = self.builder.build_alloca(field_type, name).unwrap();

                    // Initialize to null/zero if it's a string (pointer type)
                    if is_string_field && field_type.is_pointer_type() {
                        let null_ptr = field_type.into_pointer_type().const_null();
                        self.builder.build_store(alloca, null_ptr).unwrap();
                    }

                    // Restore builder position to where we were
                    self.builder.position_at_end(current_insert_block);

                    self.symbols.insert(
                        name.clone(),
                        crate::codegen::Symbol {
                            ptr: alloca,
                            ty: field_type,
                        },
                    );
                    self.builder.build_store(alloca, field_val).unwrap();
                }

                // Track if this is a string that needs RC and apply RC increment
                if is_string_field && field_val.is_pointer_value() {
                    self.heap_strings.insert(name.clone());

                    // Apply RC increment for string keys/values
                    let str_ptr = field_val.into_pointer_value();
                    let rc_header = unsafe {
                        self.builder.build_in_bounds_gep(
                            self.context.i8_type(),
                            str_ptr,
                            &[self.context.i32_type().const_int((-8_i32) as u64, true)],
                            &format!("{}_rc_header", name),
                        )
                    }
                    .unwrap();

                    let incref_fn = self.incref_fn.unwrap();
                    self.builder
                        .build_call(incref_fn, &[rc_header.into()], "")
                        .unwrap();
                }

                Some(field_val)
            }

            MirInstr::Print { values } => {
                // For now, implement a simple print that outputs integers
                // In a full implementation, you'd handle different types

                // Declare printf if not already declared
                let printf_fn = self.get_or_declare_printf();

                for value in values {
                    let val = self.resolve_value(value);

                    // Create format string based on value type
                    let format_str = if val.is_int_value() {
                        "%d\n"
                    } else if val.is_pointer_value() {
                        "%s\n"
                    } else {
                        "%d\n" // default to int format
                    };

                    let format_global = self
                        .builder
                        .build_global_string_ptr(format_str, "print_fmt")
                        .unwrap();

                    self.builder
                        .build_call(
                            printf_fn,
                            &[format_global.as_pointer_value().into(), val.into()],
                            "print_call",
                        )
                        .unwrap();
                }

                None
            }

            _ => None,
        }
    }

    /// Resolve value (unchanged)
    /// Resolves a variable or constant name to its LLVM value.
    /// Used for looking up values in the symbol table or temporary values.
    pub fn resolve_value(&self, name: &str) -> BasicValueEnum<'ctx> {
        if let Some(val) = self.temp_values.get(name) {
            return *val;
        }

        if let Some(sym) = self.symbols.get(name) {
            // Special handling for array/map variables - they should always be pointers
            let load_type =
                if (name.contains("_array") || name.contains("_map")) && sym.ty.is_int_type() {
                    // Type was incorrectly set as int, use pointer instead
                    self.context
                        .ptr_type(inkwell::AddressSpace::default())
                        .into()
                } else {
                    sym.ty
                };

            return self
                .builder
                .build_load(load_type, sym.ptr, name)
                .expect("Failed to load value");
        }

        if let Ok(val) = name.parse::<i32>() {
            return self.context.i32_type().const_int(val as u64, true).into();
        }
        if name == "true" {
            return self.context.i32_type().const_int(1, false).into();
        }
        if name == "false" {
            return self.context.i32_type().const_int(0, false).into();
        }

        eprintln!("Available temps: {:?}", self.temp_values.keys());
        eprintln!("Available symbols: {:?}", self.symbols.keys());
        panic!(
            "Unknown variable or literal: {} - check your MIR generation",
            name
        );
    }

    /// Returns the LLVM type corresponding to a type name string.
    /// Used for type resolution during codegen.
    pub fn get_llvm_type(&self, type_name: &str) -> BasicTypeEnum<'ctx> {
        match type_name {
            "Int" => self.context.i32_type().into(), // Only i32 for integers
            "Bool" => self.context.bool_type().into(),
            "Str" => self.context.ptr_type(AddressSpace::default()).into(),
            _ => self.context.i32_type().into(),
        }
    }

    /// Get or declare printf function for print statements
    fn get_or_declare_printf(&self) -> FunctionValue<'ctx> {
        if let Some(func) = self.module.get_function("printf") {
            return func;
        }

        let i8_ptr_type = self.context.ptr_type(AddressSpace::default());
        let printf_type = self.context.i32_type().fn_type(&[i8_ptr_type.into()], true);
        self.module.add_function("printf", printf_type, None)
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

        let metadata = crate::codegen::ArrayMetadata {
            length: elements.len(),
            element_type: element_type_name.to_string(),
            contains_strings,
        };

        // Register metadata under EXTENSIVE name variations for better lookup
        // This is CRITICAL for arrays created inside loops
        let base_name = name.trim_start_matches('%').trim_end_matches("_array");
        let name_variations = vec![
            name.to_string(),
            name.trim_end_matches("_array").to_string(),
            name.trim_start_matches('%').to_string(),
            format!("{}_array", name),
            format!("{}_array", name.trim_start_matches('%')),
            format!("{}_array", base_name),
            base_name.to_string(),
            format!("{}item_array", base_name),
            format!("{}item", base_name),
        ];

        eprintln!(
            "[DEBUG] Registering array metadata for '{}' with length {} under variations: {:?}",
            name,
            elements.len(),
            name_variations
        );

        for variation in name_variations {
            self.array_metadata.insert(variation, metadata.clone());
        }

        // HEAP ALLOCATE with RC header and length field
        // Layout: [RC: 4 bytes][Length: 4 bytes][data...]
        let malloc_fn = self.get_or_declare_malloc();
        let array_size = array_type.size_of().unwrap();
        let header_size = self.context.i32_type().const_int(8, false); // RC + Length = 8 bytes
        let total_size = self
            .builder
            .build_int_add(header_size, array_size, "total_size")
            .unwrap();

        let heap_ptr = self
            .builder
            .build_call(malloc_fn, &[total_size.into()], "heap_array")
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_pointer_value();

        // Store RC = 1 at offset 0
        let rc_ptr = self
            .builder
            .build_pointer_cast(
                heap_ptr,
                self.context.ptr_type(AddressSpace::default()),
                "rc_ptr",
            )
            .unwrap();
        self.builder
            .build_store(rc_ptr, self.context.i32_type().const_int(1, false))
            .unwrap();

        // Store array length at offset 4
        let len_ptr = unsafe {
            self.builder
                .build_gep(
                    self.context.i8_type(),
                    heap_ptr,
                    &[self.context.i32_type().const_int(4, false)],
                    "len_ptr",
                )
                .unwrap()
        };
        let len_ptr_cast = self
            .builder
            .build_pointer_cast(
                len_ptr,
                self.context.ptr_type(AddressSpace::default()),
                "len_ptr_cast",
            )
            .unwrap();
        self.builder
            .build_store(
                len_ptr_cast,
                self.context
                    .i32_type()
                    .const_int(elements.len() as u64, false),
            )
            .unwrap();

        // Get data pointer at offset 8
        let data_ptr = unsafe {
            self.builder
                .build_gep(
                    self.context.i8_type(),
                    heap_ptr,
                    &[self.context.i32_type().const_int(8, false)],
                    "data_ptr",
                )
                .unwrap()
        };

        // Cast to array type pointer
        let array_ptr = self
            .builder
            .build_pointer_cast(
                data_ptr,
                self.context.ptr_type(AddressSpace::default()),
                "array_ptr",
            )
            .unwrap();

        // Store the array pointer in temp_values IMMEDIATELY for metadata tracking
        self.temp_values.insert(name.to_string(), data_ptr.into());

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

        // CRITICAL: Also store element count in temp_values for later inference
        for i in 0..elements.len() {
            let elem_marker = format!("{}_elem_{}", name, i);
            self.temp_values.insert(elem_marker, element_values[i]);
        }

        // CRITICAL: Also register the actual pointer value under variations
        // This helps with later pointer equality checks
        let ptr_variations = vec![format!("ptr_{}", name), format!("data_{}", name)];
        for variation in ptr_variations {
            self.temp_values.insert(variation, data_ptr.into());
        }

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
        let total_size = self.context.i32_type().const_int(8, false);
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
        let rc_ptr = self
            .builder
            .build_pointer_cast(
                heap_ptr,
                self.context.ptr_type(AddressSpace::default()),
                "rc_ptr",
            )
            .unwrap();
        self.builder
            .build_store(rc_ptr, self.context.i32_type().const_int(1, false))
            .unwrap();

        // Get data pointer
        let data_ptr = unsafe {
            self.builder
                .build_gep(
                    self.context.i8_type(),
                    heap_ptr,
                    &[self.context.i32_type().const_int(8, false)],
                    "data_ptr",
                )
                .unwrap()
        };

        let map_ptr = self
            .builder
            .build_pointer_cast(
                data_ptr,
                self.context.ptr_type(AddressSpace::default()),
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

    /// Propagate array/map metadata from source to destination by checking all possible sources
    pub fn propagate_metadata(&mut self, dest_name: &str, source_name: &str) {
        eprintln!(
            "[DEBUG] propagate_metadata: dest='{}', source='{}'",
            dest_name, source_name
        );

        // Try to propagate array metadata directly
        if let Some(metadata) = self.array_metadata.get(source_name).cloned() {
            // Register under EXTENSIVE variations
            let dest_base = dest_name.trim_start_matches('%').trim_end_matches("_array");
            let dest_variations = vec![
                dest_name.to_string(),
                dest_name.trim_end_matches("_array").to_string(),
                dest_name.trim_start_matches('%').to_string(),
                format!("{}_array", dest_name),
                format!("{}_array", dest_base),
                dest_base.to_string(),
                format!("{}item_array", dest_base),
                format!("{}item", dest_base),
            ];

            eprintln!(
                "[DEBUG] Direct metadata found! Propagating from '{}' to '{}' with variations: {:?} (length: {})",
                source_name, dest_name, dest_variations, metadata.length
            );

            for variation in dest_variations {
                self.array_metadata.insert(variation, metadata.clone());
            }
            return;
        }

        // Try to propagate map metadata directly
        if let Some(metadata) = self.map_metadata.get(source_name).cloned() {
            self.map_metadata.insert(dest_name.to_string(), metadata);
            return;
        }

        // Try common variations of the source name
        let source_variations = vec![
            source_name.to_string(),
            source_name.trim_end_matches("_array").to_string(),
            format!("{}_array", source_name),
            source_name.trim_start_matches('%').to_string(),
            format!("%{}", source_name),
        ];

        for variation in &source_variations {
            if let Some(metadata) = self.array_metadata.get(variation).cloned() {
                // Register under EXTENSIVE dest variations
                let dest_base = dest_name.trim_start_matches('%').trim_end_matches("_array");
                let dest_variations = vec![
                    dest_name.to_string(),
                    dest_name.trim_end_matches("_array").to_string(),
                    dest_name.trim_start_matches('%').to_string(),
                    format!("{}_array", dest_name),
                    format!("{}_array", dest_base),
                    dest_base.to_string(),
                    format!("{}item_array", dest_base),
                    format!("{}item", dest_base),
                ];

                eprintln!(
                    "[DEBUG] Source variation '{}' found! Propagating to dest variations: {:?} (length: {})",
                    variation, dest_variations, metadata.length
                );

                for dest_var in dest_variations {
                    self.array_metadata.insert(dest_var, metadata.clone());
                }
                return;
            }

            if let Some(metadata) = self.map_metadata.get(variation).cloned() {
                self.map_metadata.insert(dest_name.to_string(), metadata);
                return;
            }
        }

        // Try dest_name variations against all metadata
        let dest_variations = vec![
            dest_name.to_string(),
            dest_name.trim_end_matches("_array").to_string(),
            dest_name.trim_start_matches('%').to_string(),
        ];

        for dest_var in &dest_variations {
            for source_var in &source_variations {
                if let Some(metadata) = self.array_metadata.get(source_var).cloned() {
                    // Register under ALL dest variations
                    for final_dest in &dest_variations {
                        self.array_metadata
                            .insert(final_dest.to_string(), metadata.clone());
                    }
                    return;
                }
            }
        }

        // Try by pointer equality
        if let Some(source_val) = self.temp_values.get(source_name) {
            if source_val.is_pointer_value() {
                let source_ptr = source_val.into_pointer_value();

                // Search through all array metadata for a matching pointer
                let array_metadata_clone = self.array_metadata.clone();
                for (other_name, metadata) in &array_metadata_clone {
                    if let Some(other_val) = self.temp_values.get(other_name) {
                        if other_val.is_pointer_value()
                            && other_val.into_pointer_value() == source_ptr
                        {
                            // Register under EXTENSIVE variations
                            let dest_base =
                                dest_name.trim_start_matches('%').trim_end_matches("_array");
                            let dest_variations = vec![
                                dest_name.to_string(),
                                dest_name.trim_end_matches("_array").to_string(),
                                dest_name.trim_start_matches('%').to_string(),
                                format!("{}_array", dest_name),
                                format!("{}_array", dest_base),
                                dest_base.to_string(),
                                format!("{}item_array", dest_base),
                                format!("{}item", dest_base),
                            ];

                            eprintln!(
                                "[DEBUG] Pointer equality match! '{}' == '{}', propagating to: {:?} (length: {})",
                                other_name, source_name, dest_variations, metadata.length
                            );

                            for variation in dest_variations {
                                self.array_metadata.insert(variation, metadata.clone());
                            }
                            return;
                        }
                    }
                }

                // Search through map metadata
                let map_metadata_clone = self.map_metadata.clone();
                for (other_name, metadata) in &map_metadata_clone {
                    if let Some(other_val) = self.temp_values.get(other_name) {
                        if other_val.is_pointer_value()
                            && other_val.into_pointer_value() == source_ptr
                        {
                            self.map_metadata
                                .insert(dest_name.to_string(), metadata.clone());
                            return;
                        }
                    }
                }
            }
        }

        // Enhanced fuzzy matching - check both directions and partial matches
        let array_metadata_clone = self.array_metadata.clone();
        for (meta_name, metadata) in &array_metadata_clone {
            let meta_base = meta_name.trim_end_matches("_array").trim_start_matches('%');
            let source_base = source_name
                .trim_end_matches("_array")
                .trim_start_matches('%');
            let dest_base = dest_name.trim_end_matches("_array").trim_start_matches('%');

            if meta_base == source_base
                || meta_base == dest_base
                || meta_name.contains(source_name)
                || source_name.contains(meta_name.as_str())
                || meta_name.contains(dest_name)
                || dest_name.contains(meta_name.as_str())
            {
                // Register under EXTENSIVE variations
                let dest_base_name = dest_name.trim_start_matches('%').trim_end_matches("_array");
                let dest_variations = vec![
                    dest_name.to_string(),
                    dest_name.trim_end_matches("_array").to_string(),
                    dest_name.trim_start_matches('%').to_string(),
                    format!("{}_array", dest_name),
                    format!("{}_array", dest_base_name),
                    dest_base_name.to_string(),
                    format!("{}item_array", dest_base_name),
                    format!("{}item", dest_base_name),
                ];

                eprintln!(
                    "[DEBUG] Fuzzy match! '{}' matches patterns, propagating to: {:?} (length: {})",
                    meta_name, dest_variations, metadata.length
                );

                for variation in dest_variations {
                    self.array_metadata.insert(variation, metadata.clone());
                }
                return;
            }
        }

        let map_metadata_clone = self.map_metadata.clone();
        for (meta_name, metadata) in &map_metadata_clone {
            let meta_base = meta_name.trim_start_matches('%');
            let source_base = source_name.trim_start_matches('%');

            if meta_base == source_base
                || meta_name.contains(source_name)
                || source_name.contains(meta_name.as_str())
            {
                self.map_metadata
                    .insert(dest_name.to_string(), metadata.clone());
                return;
            }
        }

        // Try loading from symbols and comparing pointers
        if let Some(source_sym) = self.symbols.get(source_name) {
            if let Ok(loaded) =
                self.builder
                    .build_load(source_sym.ty, source_sym.ptr, "propagate_check")
            {
                if loaded.is_pointer_value() {
                    let source_ptr = loaded.into_pointer_value();

                    // Search through all array metadata for a matching pointer
                    let mut found_array_meta: Option<crate::codegen::ArrayMetadata> = None;
                    let array_metadata_clone = self.array_metadata.clone();
                    for (other_name, metadata) in &array_metadata_clone {
                        if let Some(other_val) = self.temp_values.get(other_name) {
                            if other_val.is_pointer_value()
                                && other_val.into_pointer_value() == source_ptr
                            {
                                found_array_meta = Some(metadata.clone());
                                break;
                            }
                        }

                        // Also check symbols
                        if let Some(other_sym) = self.symbols.get(other_name) {
                            if let Ok(other_loaded) = self.builder.build_load(
                                other_sym.ty,
                                other_sym.ptr,
                                "other_propagate",
                            ) {
                                if other_loaded.is_pointer_value()
                                    && other_loaded.into_pointer_value() == source_ptr
                                {
                                    found_array_meta = Some(metadata.clone());
                                    break;
                                }
                            }
                        }
                    }

                    if let Some(metadata) = found_array_meta {
                        // Register under EXTENSIVE variations
                        let dest_base =
                            dest_name.trim_start_matches('%').trim_end_matches("_array");
                        let dest_variations = vec![
                            dest_name.to_string(),
                            dest_name.trim_end_matches("_array").to_string(),
                            dest_name.trim_start_matches('%').to_string(),
                            format!("{}_array", dest_name),
                            format!("{}_array", dest_base),
                            dest_base.to_string(),
                            format!("{}item_array", dest_base),
                            format!("{}item", dest_base),
                        ];

                        eprintln!(
                            "[DEBUG] Symbol pointer match! Propagating to: {:?} (length: {})",
                            dest_variations, metadata.length
                        );

                        for variation in dest_variations {
                            self.array_metadata.insert(variation, metadata.clone());
                        }
                        return;
                    }

                    // Search through map metadata
                    let mut found_map_meta: Option<crate::codegen::MapMetadata> = None;
                    let map_metadata_clone = self.map_metadata.clone();
                    for (other_name, metadata) in &map_metadata_clone {
                        if let Some(other_val) = self.temp_values.get(other_name) {
                            if other_val.is_pointer_value()
                                && other_val.into_pointer_value() == source_ptr
                            {
                                found_map_meta = Some(metadata.clone());
                                break;
                            }
                        }
                    }

                    if let Some(metadata) = found_map_meta {
                        self.map_metadata.insert(dest_name.to_string(), metadata);
                        return;
                    }
                }
            }
        }
    }

    /// Helper implementations for array and map operations with RC
    pub fn get_array_length(&self, array_name: &str) -> inkwell::values::IntValue<'ctx> {
        eprintln!("[DEBUG] get_array_length called for '{}'", array_name);

        // STEP 1: Direct metadata lookup
        if let Some(metadata) = self.array_metadata.get(array_name) {
            eprintln!(
                "[DEBUG] Found array length for '{}': {}",
                array_name, metadata.length
            );
            return self
                .context
                .i32_type()
                .const_int(metadata.length as u64, false);
        }

        // STEP 2: Try name variations
        let search_names = vec![
            array_name.trim_end_matches("_array").to_string(),
            format!("{}_array", array_name),
            array_name.trim_start_matches('%').to_string(),
        ];

        for search_name in &search_names {
            if let Some(metadata) = self.array_metadata.get(search_name) {
                eprintln!(
                    "[DEBUG] Found array length via variation '{}' -> '{}': {}",
                    array_name, search_name, metadata.length
                );
                return self
                    .context
                    .i32_type()
                    .const_int(metadata.length as u64, false);
            }
        }

        // STEP 3: Try pointer equality matching
        if let Some(sym) = self.symbols.get(array_name) {
            if let Ok(loaded) = self.builder.build_load(sym.ty, sym.ptr, "check_ptr") {
                if loaded.is_pointer_value() {
                    let ptr_val = loaded.into_pointer_value();

                    for (other_name, metadata) in &self.array_metadata {
                        if let Some(other_val) = self.temp_values.get(other_name) {
                            if other_val.is_pointer_value()
                                && other_val.into_pointer_value() == ptr_val
                            {
                                eprintln!(
                                    "[DEBUG] Found array length via pointer match '{}' -> '{}': {}",
                                    array_name, other_name, metadata.length
                                );
                                return self
                                    .context
                                    .i32_type()
                                    .const_int(metadata.length as u64, false);
                            }
                        }
                    }
                }
            }
        }

        // STEP 4: CRITICAL - Runtime length extraction from heap header
        // For dynamically created arrays (like innerarr), extract length at runtime
        eprintln!(
            "[DEBUG] Attempting runtime length extraction for '{}'",
            array_name
        );

        if let Some(sym) = self.symbols.get(array_name) {
            if let Ok(loaded) = self.builder.build_load(sym.ty, sym.ptr, "runtime_load") {
                if loaded.is_pointer_value() {
                    let arr_ptr = loaded.into_pointer_value();

                    // Array layout: [RC: 4 bytes][Length: 4 bytes][data at offset 8]
                    // arr_ptr points to data, so length is at offset -4
                    let len_ptr_result = unsafe {
                        self.builder.build_in_bounds_gep(
                            self.context.i8_type(),
                            arr_ptr,
                            &[self.context.i32_type().const_int((-4_i32) as u64, true)],
                            &format!("{}_runtime_len_ptr", array_name),
                        )
                    };

                    if let Ok(len_ptr) = len_ptr_result {
                        let len_ptr_cast_result = self.builder.build_pointer_cast(
                            len_ptr,
                            self.context.ptr_type(inkwell::AddressSpace::default()),
                            &format!("{}_len_ptr_cast", array_name),
                        );

                        if let Ok(len_ptr_cast) = len_ptr_cast_result {
                            if let Ok(runtime_len) = self.builder.build_load(
                                self.context.i32_type(),
                                len_ptr_cast,
                                &format!("{}_runtime_len", array_name),
                            ) {
                                eprintln!(
                                    "[SUCCESS] Extracted runtime length for '{}'",
                                    array_name
                                );
                                return runtime_len.into_int_value();
                            }
                        }
                    }
                }
            }
        }

        // FINAL FALLBACK: Return 0 to skip loop safely
        eprintln!(
            "[ERROR] Could not determine length for '{}', defaulting to 0",
            array_name
        );
        eprintln!(
            "[DEBUG] Available metadata: {:?}",
            self.array_metadata.keys().collect::<Vec<_>>()
        );
        self.context.i32_type().const_int(0, false)
    }

    pub fn get_array_element_type(&self, array_name: &str) -> inkwell::types::BasicTypeEnum<'ctx> {
        if let Some(metadata) = self.array_metadata.get(array_name) {
            match metadata.element_type.as_str() {
                "Int" => self.context.i32_type().into(), // Only i32 for integers
                "Bool" => self.context.bool_type().into(),
                "Str" => self.context.ptr_type(AddressSpace::default()).into(),
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
            // Try one more time to find by pointer equality before giving up
            if let Some(sym) = self.symbols.get(map_name) {
                if let Ok(loaded) = self.builder.build_load(sym.ty, sym.ptr, "check_map_len") {
                    if loaded.is_pointer_value() {
                        let ptr_val = loaded.into_pointer_value();
                        for (other_name, metadata) in &self.map_metadata {
                            if let Some(other_val) = self.temp_values.get(other_name) {
                                if other_val.is_pointer_value()
                                    && other_val.into_pointer_value() == ptr_val
                                {
                                    return self
                                        .context
                                        .i32_type()
                                        .const_int(metadata.length as u64, false);
                                }
                            }
                        }
                    }
                }
            }

            eprintln!(
                "Warning: No map metadata found for '{}' in get_map_length",
                map_name
            );
            eprintln!(
                "Available map metadata: {:?}",
                self.map_metadata.keys().collect::<Vec<_>>()
            );
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
                "Str" => self.context.ptr_type(AddressSpace::default()).into(),
                _ => {
                    eprintln!(
                        "WARNING: Unknown key type '{}' for map '{}', defaulting to i32",
                        metadata.key_type, map_name
                    );
                    self.context.i32_type().into()
                }
            };

            let val_type = match metadata.value_type.as_str() {
                "Int" => self.context.i32_type().into(),
                "Bool" => self.context.bool_type().into(),
                "Str" => self.context.ptr_type(AddressSpace::default()).into(),
                _ => {
                    eprintln!(
                        "WARNING: Unknown value type '{}' for map '{}', defaulting to i32",
                        metadata.value_type, map_name
                    );
                    self.context.i32_type().into()
                }
            };

            (key_type, val_type)
        } else {
            eprintln!("\n╔════════════════════════════════════════════════════════════════════╗");
            eprintln!("║ ERROR: No metadata found for map '{}'", map_name);
            eprintln!("╚════════════════════════════════════════════════════════════════════╝");
            eprintln!("\n📊 Available map metadata:");
            if self.map_metadata.is_empty() {
                eprintln!("  (none)");
            } else {
                for (key, meta) in &self.map_metadata {
                    eprintln!(
                        "  • '{}' → {{{}:{}}}, length={}",
                        key, meta.key_type, meta.value_type, meta.length
                    );
                }
            }
            eprintln!("\n⚠️  Cannot determine map types without metadata - IR will be incorrect!");
            eprintln!("═══════════════════════════════════════════════════════════════════\n");

            // Return dummy types, but this will produce incorrect IR
            panic!(
                "FATAL: Cannot proceed without map metadata for '{}'",
                map_name
            );
        }
    }

    /// Returns true if the array contains string elements.
    pub fn array_contains_strings(&self, array_name: &str) -> bool {
        if let Some(metadata) = self.array_metadata.get(array_name) {
            metadata.contains_strings
        } else {
            false
        }
    }

    /// Load array element with proper RC management for strings
    pub fn load_array_element_with_rc(
        &mut self,
        array_ptr: inkwell::values::PointerValue<'ctx>,
        index: inkwell::values::IntValue<'ctx>,
        elem_type: inkwell::types::BasicTypeEnum<'ctx>,
        is_string: bool,
    ) -> inkwell::values::BasicValueEnum<'ctx> {
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
                    &[self.context.i32_type().const_int((-8_i32) as u64, true)],
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

    /// Generate cleanup when exiting a loop (called from loops.rs)
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
                    // Free the array - __decref will handle element cleanup recursively
                    self.emit_decref(var);
                } else if self.heap_maps.contains(var) {
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
                                                .i32_type()
                                                .const_int((-8_i32) as u64, true)],
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

    /// Returns the pair type string for a map (used for struct type).
    pub fn get_map_pair_type(&self, map_name: &str) -> inkwell::types::StructType<'ctx> {
        let (key_type, val_type) = self.get_map_types(map_name);
        self.context.struct_type(&[key_type, val_type], false)
    }

    /// Returns true if the map contains string keys or values.
    pub fn map_contains_strings(&self, map_name: &str) -> (bool, bool) {
        if let Some(metadata) = self.map_metadata.get(map_name) {
            (metadata.key_is_string, metadata.value_is_string)
        } else {
            (false, false)
        }
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
                    &[self.context.i32_type().const_int((-8_i32) as u64, true)],
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
                    &[self.context.i32_type().const_int((-8_i32) as u64, true)],
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

    /// Get or declare strlen function for string length calculation
    pub fn get_or_declare_strlen(&self) -> FunctionValue<'ctx> {
        if let Some(func) = self.module.get_function("strlen") {
            return func;
        }

        // Declare strlen: size_t strlen(const char *s)
        let i8_ptr = self.context.ptr_type(AddressSpace::default());
        let size_t = self.context.i32_type(); // Using i32 for size_t
        let fn_type = size_t.fn_type(&[i8_ptr.into()], false);

        self.module.add_function("strlen", fn_type, None)
    }
}
