use crate::codegen::CodeGen;
use inkwell::values::FunctionValue;
use inkwell::AddressSpace;

impl<'ctx> CodeGen<'ctx> {
    pub fn init_rc_runtime(&mut self) {
        self.incref_fn = Some(self.create_incref_function());
        self.decref_fn = Some(self.create_decref_function());
    }

    fn create_incref_function(&self) -> FunctionValue<'ctx> {
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let void_type = self.context.void_type();
        let fn_type = void_type.fn_type(&[i8_ptr.into()], false);

        let function = self.module.add_function("__incref", fn_type, None);
        let entry = self.context.append_basic_block(function, "entry");

        self.builder.position_at_end(entry);

        // Parameter is already the RC header pointer (not data pointer)
        let rc_ptr = function.get_nth_param(0).unwrap().into_pointer_value();

        let i64_ptr_type = self.context.i64_type().ptr_type(AddressSpace::default());
        let rc_ptr_typed = self
            .builder
            .build_pointer_cast(rc_ptr, i64_ptr_type, "rc_ptr")
            .unwrap();

        let rc = self
            .builder
            .build_load(self.context.i64_type(), rc_ptr_typed, "rc")
            .unwrap()
            .into_int_value();

        let new_rc = self
            .builder
            .build_int_add(rc, self.context.i64_type().const_int(1, false), "new_rc")
            .unwrap();

        self.builder.build_store(rc_ptr_typed, new_rc).unwrap();
        self.builder.build_return(None).unwrap();

        function
    }

    fn create_decref_function(&self) -> FunctionValue<'ctx> {
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let void_type = self.context.void_type();
        let fn_type = void_type.fn_type(&[i8_ptr.into()], false);

        let function = self.module.add_function("__decref", fn_type, None);
        let entry = self.context.append_basic_block(function, "entry");
        let free_block = self.context.append_basic_block(function, "free");
        let exit_block = self.context.append_basic_block(function, "exit");

        self.builder.position_at_end(entry);

        let rc_ptr = function.get_nth_param(0).unwrap().into_pointer_value();

        let i64_ptr_type = self.context.i64_type().ptr_type(AddressSpace::default());
        let rc_ptr_typed = self
            .builder
            .build_pointer_cast(rc_ptr, i64_ptr_type, "rc_ptr")
            .unwrap();

        let rc = self
            .builder
            .build_load(self.context.i64_type(), rc_ptr_typed, "rc")
            .unwrap()
            .into_int_value();

        let new_rc = self
            .builder
            .build_int_sub(rc, self.context.i64_type().const_int(1, false), "new_rc")
            .unwrap();

        self.builder.build_store(rc_ptr_typed, new_rc).unwrap();

        let should_free = self
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                new_rc,
                self.context.i64_type().const_int(0, false),
                "should_free",
            )
            .unwrap();

        self.builder
            .build_conditional_branch(should_free, free_block, exit_block)
            .unwrap();

        self.builder.position_at_end(free_block);
        let free_fn = self.get_or_declare_free();
        self.builder
            .build_call(free_fn, &[rc_ptr.into()], "")
            .unwrap();
        self.builder.build_unconditional_branch(exit_block).unwrap();

        self.builder.position_at_end(exit_block);
        self.builder.build_return(None).unwrap();

        function
    }

    fn get_or_declare_free(&self) -> FunctionValue<'ctx> {
        if let Some(func) = self.module.get_function("free") {
            return func;
        }

        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let void_type = self.context.void_type();
        let fn_type = void_type.fn_type(&[i8_ptr.into()], false);

        self.module.add_function("free", fn_type, None)
    }

    pub fn get_or_declare_malloc(&self) -> FunctionValue<'ctx> {
        if let Some(func) = self.module.get_function("malloc") {
            return func;
        }

        let i64_type = self.context.i64_type();
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let fn_type = i8_ptr.fn_type(&[i64_type.into()], false);

        self.module.add_function("malloc", fn_type, None)
    }

    pub fn get_or_declare_memcpy(&self) -> FunctionValue<'ctx> {
        if let Some(func) = self.module.get_function("llvm.memcpy.p0.p0.i64") {
            return func;
        }
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let i64_type = self.context.i64_type();
        let i1_type = self.context.bool_type();

        let fn_type = self.context.void_type().fn_type(
            &[
                i8_ptr.into(),
                i8_ptr.into(),
                i64_type.into(),
                i1_type.into(),
            ],
            false,
        );
        self.module
            .add_function("llvm.memcpy.p0.p0.i64", fn_type, None)
    }

    pub fn emit_incref(&self, var_name: &str) {
        if let Some(symbol) = self.symbols.get(var_name) {
            // Load the data pointer
            let data_ptr = self
                .builder
                .build_load(symbol.ty, symbol.ptr, "loaded")
                .unwrap()
                .into_pointer_value();

            // Get RC header by subtracting 8 bytes using in_bounds_gep
            let rc_header = unsafe {
                self.builder.build_in_bounds_gep(
                    self.context.i8_type(),
                    data_ptr,
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
    }

    pub fn emit_decref(&self, var_name: &str) {
        if let Some(symbol) = self.symbols.get(var_name) {
            let data_ptr = self
                .builder
                .build_load(symbol.ty, symbol.ptr, "loaded")
                .unwrap()
                .into_pointer_value();

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
