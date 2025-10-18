use crate::codegen::core::CodeGen;
use crate::mir::MirInstr;
use inkwell::values::BasicValueEnum;
use inkwell::AddressSpace;

impl<'ctx> CodeGen<'ctx> {
    pub fn generate_const_int(&mut self, name: &str, value: i32) -> Option<BasicValueEnum<'ctx>> {
        let val = self.context.i32_type().const_int(value as u64, true);
        // If this temp was pre-allocated as a symbol (cross-block usage), store it there
        if let Some(sym) = self.symbols.get(name) {
            self.builder.build_store(sym.ptr, val).unwrap();
        }
        self.temp_values.insert(name.to_string(), val.into());
        Some(val.into())
    }

    pub fn generate_const_bool(&mut self, name: &str, value: bool) -> Option<BasicValueEnum<'ctx>> {
        // Use i32 instead of i1 for consistency with rest of codegen
        let val = self.context.i32_type().const_int(value as u64, false);
        // If this temp was pre-allocated as a symbol (cross-block usage), store it there
        if let Some(sym) = self.symbols.get(name) {
            self.builder.build_store(sym.ptr, val).unwrap();
        }
        self.temp_values.insert(name.to_string(), val.into());
        Some(val.into())
    }

    pub fn generate_const_string(
        &mut self,
        name: &str,
        value: &str,
    ) -> Option<BasicValueEnum<'ctx>> {
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
                self.context.ptr_type(inkwell::AddressSpace::default()),
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

        self.temp_values.insert(name.to_string(), data_ptr.into());
        self.heap_strings.insert(name.to_string());

        Some(data_ptr.into())
    }
}
