use crate::codegen::core::CodeGen;
use crate::mir::MirInstr;
use inkwell::values::BasicValueEnum;

impl<'ctx> CodeGen<'ctx> {
    pub fn generate_call(
        &mut self,
        dest: &[String],
        func: &str,
        args: &[String],
    ) -> Option<inkwell::values::BasicValueEnum<'ctx>> {
        let callee = self.module.get_function(func).expect(&format!(
            "Function '{}' not found. Make sure it's declared before calling.",
            func
        ));

        let arg_values: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args
            .iter()
            .map(|arg| self.resolve_value(arg).into())
            .collect();

        let call_result = self
            .builder
            .build_call(callee, &arg_values, "call_result")
            .unwrap();

        if let Some(result) = call_result.try_as_basic_value().left() {
            if !dest.is_empty() {
                let dest_name = &dest[0];
                self.temp_values.insert(dest_name.clone(), result);

                if let Some(return_type_str) = self.function_return_types.get(func) {
                    if result.is_pointer_value() {
                        if return_type_str.contains("Str") || return_type_str.contains("String") {
                            self.heap_strings.insert(dest_name.clone());
                        } else if return_type_str.contains("Array") {
                            self.heap_arrays.insert(dest_name.clone());
                        } else if return_type_str.contains("Map") {
                            self.heap_maps.insert(dest_name.clone());
                        }
                    }
                }

                return Some(result);
            }
        }

        None
    }

    pub fn generate_print(&mut self, values: &[String]) {
        let printf_fn = self.get_or_declare_printf();

        for (idx, value) in values.iter().enumerate() {
            let value_base = value.trim_start_matches('%').trim_end_matches("_array");
            let is_single_char = value_base.len() == 1;
            let is_common_loop_var = matches!(
                value_base,
                "item" | "elem" | "element" | "key" | "val" | "value" | "n"
            );

            let is_array = !is_single_char
                && !is_common_loop_var
                && (self.array_metadata.contains_key(value) || self.heap_arrays.contains(value));
            let is_map = !is_single_char
                && !is_common_loop_var
                && (self.map_metadata.contains_key(value) || self.heap_maps.contains(value));

            if is_array {
                self.print_array(value);
                if idx < values.len() - 1 {
                    let space_fmt = self
                        .builder
                        .build_global_string_ptr(" ", "space_fmt")
                        .unwrap();
                    self.builder
                        .build_call(
                            printf_fn,
                            &[space_fmt.as_pointer_value().into()],
                            "space_call",
                        )
                        .unwrap();
                }
            } else if is_map {
                self.print_map(value);
                if idx < values.len() - 1 {
                    let space_fmt = self
                        .builder
                        .build_global_string_ptr(" ", "space_fmt")
                        .unwrap();
                    self.builder
                        .build_call(
                            printf_fn,
                            &[space_fmt.as_pointer_value().into()],
                            "space_call",
                        )
                        .unwrap();
                }
            } else {
                let val = self.resolve_value(value);

                if val.is_int_value() {
                    let format_str = if idx < values.len() - 1 { "%d " } else { "%d" };
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
                } else if val.is_pointer_value() {
                    let format_str = if idx < values.len() - 1 { "%s " } else { "%s" };
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
            }
        }

        let newline_fmt = self
            .builder
            .build_global_string_ptr("\n", "newline_fmt")
            .unwrap();
        self.builder
            .build_call(
                printf_fn,
                &[newline_fmt.as_pointer_value().into()],
                "newline_call",
            )
            .unwrap();
    }

    pub fn generate_array_len(
        &mut self,
        name: &str,
        array: &str,
    ) -> Option<inkwell::values::BasicValueEnum<'ctx>> {
        let array_name = array;

        if let Some(metadata) = self.array_metadata.get(array_name) {
            let len_val = self
                .context
                .i32_type()
                .const_int(metadata.length as u64, false);
            self.temp_values.insert(name.to_string(), len_val.into());
            if let Some(sym) = self.symbols.get(name) {
                self.builder.build_store(sym.ptr, len_val).unwrap();
            }
            return Some(len_val.into());
        }

        if let Some(metadata) = self.map_metadata.get(array_name) {
            let len_val = self
                .context
                .i32_type()
                .const_int(metadata.length as u64, false);
            self.temp_values.insert(name.to_string(), len_val.into());
            if let Some(sym) = self.symbols.get(name) {
                self.builder.build_store(sym.ptr, len_val).unwrap();
            }
            return Some(len_val.into());
        }

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
                        let len_val = runtime_len.into_int_value();
                        self.temp_values.insert(name.to_string(), len_val.into());
                        if let Some(sym) = self.symbols.get(name) {
                            self.builder.build_store(sym.ptr, len_val).unwrap();
                        }
                        return Some(len_val.into());
                    }
                }
            }
        }

        let len_val = self.context.i32_type().const_int(0, false);
        self.temp_values.insert(name.to_string(), len_val.into());
        if let Some(sym) = self.symbols.get(name) {
            self.builder.build_store(sym.ptr, len_val).unwrap();
        }
        Some(len_val.into())
    }
}
