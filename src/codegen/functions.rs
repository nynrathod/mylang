use crate::codegen::CodeGen;
use crate::mir::mir::{CodegenBlock, MirBlock, MirFunction, MirInstr, MirProgram, MirTerminator};
use inkwell::types::BasicType;
use inkwell::types::StructType;
use inkwell::values::FunctionValue;
use std::collections::HashMap;

impl<'ctx> CodeGen<'ctx> {
    /// The main entry point for code generation. Processes the entire MIR program.
    /// This function orchestrates the translation of the MIR (Mid-level Intermediate Representation)
    /// into LLVM IR, handling global variables, functions, and the main entry point.
    /// It also initializes reference counting runtime and applies optimization passes.
    pub fn generate_program(&mut self, program: &MirProgram) {
        // Initialize RC runtime FIRST to ensure reference counting functions are available.
        self.init_rc_runtime();

        // Store the global instructions for later use (e.g., initialization).
        self.globals = program.globals.clone();

        // --- PRE-PROCESSING ---
        // Scan all global instructions to identify strings involved in concatenation.
        // This helps optimize string handling and memory management.
        for instr in &program.globals {
            if let MirInstr::StringConcat { left, right, .. } = instr {
                self.strings_to_concat.insert(left.clone());
                self.strings_to_concat.insert(right.clone());
            }
        }

        // --- GLOBAL GENERATION ---
        // Generate LLVM IR for all global variables and constants.
        for g in &program.globals {
            self.generate_global(g);
        }

        // --- FUNCTION GENERATION ---
        // Generate LLVM IR for all user-defined functions and apply optimizations.
        for func in &program.functions {
            let llvm_func = self.generate_function(func);
            // Apply registered optimization passes (like O1, O2, O3) to the generated function.
            self.fpm.run_on(&llvm_func);
        }

        // --- MAIN ENTRY POINT ---
        // Ensures the final executable has a standard `main` function if the source didn't define one.
        if self.module.get_function("main").is_none() {
            self.generate_default_main();
        }
    }

    /// Creates a minimal `main` function (`i32 ()`) that returns 0.
    /// This is a fallback to guarantee the presence of a valid entry point in the generated binary.
    pub fn generate_default_main(&mut self) {
        let main_type = self.context.i32_type().fn_type(&[], false);
        let main_func = self.module.add_function("main", main_type, None);

        let entry_bb = self.context.append_basic_block(main_func, "entry");
        self.builder.position_at_end(entry_bb);

        let zero = self.context.i32_type().const_int(0, false);
        // Generates the `ret i32 0` instruction.
        self.builder.build_return(Some(&zero));
    }

    /// Generates the LLVM structure and code for a single MIR function.
    /// Generates LLVM IR for a user-defined function.
    /// This method:
    /// - Defines the function signature (return type and parameter types).
    /// - Creates all basic blocks for control flow.
    /// - Allocates and registers parameters in the symbol table.
    /// - Translates MIR blocks and instructions into LLVM IR.
    /// - Handles block terminators (return, jump, conditional jump).
    /// Returns the LLVM FunctionValue for further manipulation or optimization.
    pub fn generate_function(&mut self, func: &MirFunction) -> FunctionValue<'ctx> {
        // Define the LLVM function signature (return type and parameter types).
        // Only i32 is supported for all function return and parameter types.
        let fn_type = self.context.i32_type().fn_type(
            &vec![self.context.i32_type().into(); func.params.len()],
            false,
        );

        let llvm_func = self.module.add_function(&func.name, fn_type, None);

        // Create all necessary basic blocks within the function (e.g., entry, if.then, loop.body).
        let mut bb_map = HashMap::new();
        for block in &func.blocks {
            let bb = self.context.append_basic_block(llvm_func, &block.label);
            bb_map.insert(block.label.clone(), bb);
        }

        // Set the builder position to the function's entry block.
        // Start with the first block from MIR (Block0, entry, etc.)
        if let Some(first_block) = func.blocks.first() {
            if let Some(bb) = bb_map.get(&first_block.label) {
                self.builder.position_at_end(*bb);
            }
        }

        // Allocate space for parameters and store their incoming values.
        // This ensures parameters are available as local variables in the function scope.
        for (i, param) in func.params.iter().enumerate() {
            let param_val = llvm_func.get_nth_param(i as u32).unwrap();
            let alloca = self
                .builder
                .build_alloca(self.context.i32_type(), param)
                .expect("Failed to allocate function parameter");

            self.builder.build_store(alloca, param_val);

            // Register the parameter in the symbol table for future lookups.
            self.symbols.insert(
                param.clone(),
                crate::codegen::Symbol {
                    ptr: alloca,
                    ty: self.context.i32_type().into(),
                },
            );
        }

        // Convert MIR block terminators to a unified structure for easier handling.
        // This simplifies codegen for control flow instructions.
        let codegen_blocks: Vec<CodegenBlock> = func
            .blocks
            .iter()
            // The map operation transforms the block structure to handle terminators uniformly.
            .map(|b| CodegenBlock {
                label: &b.label,
                instrs: &b.instrs,
                // Pattern match to extract the inner values from the Instruction enum.
                terminator: match &b.terminator {
                    Some(MirInstr::Return { values }) => Some(MirTerminator::Return {
                        values: values.clone(),
                    }),
                    Some(MirInstr::Jump { target }) => Some(MirTerminator::Jump {
                        target: target.clone(),
                    }),
                    Some(MirInstr::CondJump {
                        cond,
                        then_block,
                        else_block,
                    }) => Some(MirTerminator::CondJump {
                        cond: cond.clone(),
                        then_block: then_block.clone(),
                        else_block: else_block.clone(),
                    }),
                    _ => None,
                },
            })
            .collect();

        // Generate instructions and terminators for all blocks.
        for block in &codegen_blocks {
            let bb = bb_map.get(block.label).unwrap();
            // Position builder at the start of the block
            self.builder.position_at_end(*bb);

            // Generate all instructions (assignments, operations) within the block.
            for instr in block.instrs {
                self.generate_instr(instr);
            }

            // Generate the block's terminating instruction (branch, return).
            if let Some(term) = &block.terminator {
                self.generate_terminator(term, llvm_func, &bb_map);
            }
        }

        llvm_func
    }

    /// Generates LLVM IR for a single MIR block.
    /// This method:
    /// - Handles loop markers and loop setup instructions.
    /// - Processes instructions for assignments, operations, and memory management.
    /// - Manages reference counting for heap-allocated variables.
    /// - Handles block terminators and loop continuation logic.
    /// It ensures correct control flow and memory cleanup for loops and regular blocks.
    pub fn generate_block(
        &mut self,
        block: &MirBlock,
        func: FunctionValue<'ctx>,
        bb_map: &HashMap<String, inkwell::basic_block::BasicBlock<'ctx>>,
    ) {
        let bb = bb_map.get(&block.label).unwrap();
        self.builder.position_at_end(*bb);

        // Track if this is a loop body and what kind
        let mut loop_increment_var: Option<String> = None;
        let mut loop_cond_block: Option<String> = None;
        let mut is_range_loop = false;
        let mut is_array_loop = false;
        let mut is_map_loop = false;
        let mut array_name: Option<String> = None;
        let mut index_var: Option<String> = None;
        let mut item_var: Option<String> = None;
        let mut map_name: Option<String> = None;
        let mut key_var: Option<String> = None;
        let mut val_var: Option<String> = None;

        // Scan for loop markers to identify loop context and variables.
        for instr in &block.instrs {
            match instr {
                MirInstr::LoopBodyMarker {
                    var, cond_block, ..
                } => {
                    is_range_loop = true;
                    loop_increment_var = Some(var.clone());
                    loop_cond_block = Some(cond_block.clone());
                }
                MirInstr::ArrayLoopMarker {
                    array,
                    index,
                    item,
                    cond_block,
                } => {
                    is_array_loop = true;
                    array_name = Some(array.clone());
                    index_var = Some(index.clone());
                    item_var = Some(item.clone());
                    loop_cond_block = Some(cond_block.clone());
                }
                MirInstr::MapLoopMarker {
                    map,
                    index,
                    key,
                    value,
                    cond_block,
                } => {
                    is_map_loop = true;
                    map_name = Some(map.clone());
                    index_var = Some(index.clone());
                    key_var = Some(key.clone());
                    val_var = Some(value.clone());
                    loop_cond_block = Some(cond_block.clone());
                }
                _ => {}
            }
        }

        // Process instructions in the block.
        for instr in &block.instrs {
            match instr {
                // Skip marker instructions (used only for loop context).
                MirInstr::LoopBodyMarker { .. }
                | MirInstr::ArrayLoopMarker { .. }
                | MirInstr::MapLoopMarker { .. } => continue,

                // Handle loop setup instructions.
                MirInstr::ForRange { .. }
                | MirInstr::ForArray { .. }
                | MirInstr::ForMap { .. }
                | MirInstr::ForInfinite { .. } => {
                    self.generate_for_loop(instr, bb_map);
                }

                // Handle break/continue with cleanup of loop variables.
                MirInstr::Break { .. } | MirInstr::Continue { .. } => {
                    // Clean up loop variables before jumping.
                    if is_array_loop && item_var.is_some() {
                        let item = item_var.as_ref().unwrap();
                        if self.heap_strings.contains(item) {
                            self.emit_decref(item);
                        }
                    }
                    if is_map_loop {
                        if let Some(key) = &key_var {
                            if self.heap_strings.contains(key) {
                                self.emit_decref(key);
                            }
                        }
                        if let Some(val) = &val_var {
                            if self.heap_strings.contains(val) {
                                self.emit_decref(val);
                            }
                        }
                    }

                    self.generate_for_loop(instr, bb_map);
                    return; // These terminate the block
                }

                // Handle array element and map pair loading.
                MirInstr::LoadArrayElement { .. } | MirInstr::LoadMapPair { .. } => {
                    self.generate_instr(instr);
                }

                // Regular instructions (assignments, operations, etc.).
                _ => {
                    self.generate_instr(instr);
                }
            }
        }

        // After all instructions, handle loop continuation logic.
        if is_range_loop {
            // Range loop: increment variable and jump to condition.
            if let (Some(var), Some(cond_block)) = (loop_increment_var, loop_cond_block) {
                let cond_bb = bb_map.get(&cond_block).expect("Condition block not found");
                self.generate_loop_increment_and_branch(&var, *cond_bb);
                return; // Don't process terminator
            }
        } else if is_array_loop {
            // Array loop: decref item (if string), increment index, jump to condition.
            if let (Some(item), Some(index), Some(cond_block)) =
                (item_var, index_var, loop_cond_block)
            {
                // Decref the item if it's a string (was incref'd when loaded).
                if self.heap_strings.contains(&item) {
                    self.emit_decref(&item);
                }

                // Increment index.
                if let Some(symbol) = self.symbols.get(&index) {
                    let current = self
                        .builder
                        .build_load(self.context.i32_type(), symbol.ptr, "current_idx")
                        .unwrap()
                        .into_int_value();

                    let one = self.context.i32_type().const_int(1, false);
                    let incremented = self
                        .builder
                        .build_int_add(current, one, "incremented_idx")
                        .unwrap();

                    self.builder.build_store(symbol.ptr, incremented).unwrap();
                }

                // Jump back to condition.
                let cond_bb = bb_map.get(&cond_block).expect("Condition block not found");
                self.builder.build_unconditional_branch(*cond_bb).unwrap();
                return;
            }
        } else if is_map_loop {
            // Map loop: decref key and value (if strings), increment index, jump to condition.
            if let (Some(key), Some(val), Some(index), Some(cond_block)) =
                (key_var, val_var, index_var, loop_cond_block)
            {
                // Decref key if string.
                if self.heap_strings.contains(&key) {
                    self.emit_decref(&key);
                }

                // Decref value if string.
                if self.heap_strings.contains(&val) {
                    self.emit_decref(&val);
                }

                // Increment index.
                if let Some(symbol) = self.symbols.get(&index) {
                    let current = self
                        .builder
                        .build_load(self.context.i32_type(), symbol.ptr, "current_idx")
                        .unwrap()
                        .into_int_value();

                    let one = self.context.i32_type().const_int(1, false);
                    let incremented = self
                        .builder
                        .build_int_add(current, one, "incremented_idx")
                        .unwrap();

                    self.builder.build_store(symbol.ptr, incremented).unwrap();
                }

                // Jump back to condition.
                let cond_bb = bb_map.get(&cond_block).expect("Condition block not found");
                self.builder.build_unconditional_branch(*cond_bb).unwrap();
                return;
            }
        }

        // Handle regular block terminator (return, jump, cond jump).
        if let Some(instr) = &block.terminator {
            let term = match instr {
                MirInstr::Return { values } => MirTerminator::Return {
                    values: values.clone(),
                },
                MirInstr::Jump { target } => MirTerminator::Jump {
                    target: target.clone(),
                },
                MirInstr::CondJump {
                    cond,
                    then_block,
                    else_block,
                } => MirTerminator::CondJump {
                    cond: cond.clone(),
                    then_block: then_block.clone(),
                    else_block: else_block.clone(),
                },
                _ => return,
            };
            self.generate_terminator(&term, func, bb_map);
        }
    }

    /// Generates the final instruction of a basic block (the control flow transfer).
    /// Generates LLVM IR for block terminators (return, jump, conditional jump).
    /// This method:
    /// - Handles memory cleanup for heap-allocated variables (strings, arrays, maps) on return.
    /// - Emits LLVM IR for unconditional and conditional branches.
    /// - Ensures correct control flow and resource management at block boundaries.
    pub fn generate_terminator(
        &mut self,
        term: &MirTerminator,
        _l1func: FunctionValue<'ctx>, // Note: l1func is unused but kept for context/FFI
        bb_map: &HashMap<String, inkwell::basic_block::BasicBlock<'ctx>>,
    ) {
        match term {
            // Handles function return.
            // In functions.rs, MirTerminator::Return
            MirTerminator::Return { values } => {
                // 1. Free strings in composites (both arrays AND maps)
                for (_var_name, str_ptrs) in &self.composite_string_ptrs {
                    for str_ptr in str_ptrs {
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

                // Also handle map string tracking
                for (var_name, str_names) in &self.composite_strings {
                    if self.heap_maps.contains(var_name) {
                        for str_name in str_names {
                            if let Some(val) = self.temp_values.get(str_name) {
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
                }

                // 2. Free arrays
                let mut heap_array_vars: Vec<String> = self
                    .symbols
                    .keys()
                    .filter(|name| self.heap_arrays.contains(*name))
                    .cloned()
                    .collect();
                heap_array_vars.reverse();

                for var_name in heap_array_vars {
                    self.emit_decref(&var_name);
                }

                // 3. Free maps
                let mut heap_map_vars: Vec<String> = self
                    .symbols
                    .keys()
                    .filter(|name| self.heap_maps.contains(*name))
                    .cloned()
                    .collect();
                heap_map_vars.reverse();

                for var_name in heap_map_vars {
                    self.emit_decref(&var_name);
                }

                // 4. Free simple strings
                let mut heap_str_vars: Vec<String> = self
                    .symbols
                    .keys()
                    .filter(|name| self.heap_strings.contains(*name))
                    .cloned()
                    .collect();
                heap_str_vars.reverse();

                for var_name in heap_str_vars {
                    self.emit_decref(&var_name);
                }

                if values.is_empty() {
                    let zero = self.context.i32_type().const_int(0, false);
                    self.builder.build_return(Some(&zero)).unwrap();
                } else {
                    let val = self.resolve_value(&values[0]);
                    self.builder.build_return(Some(&val)).unwrap();
                }
            }
            // Handles unconditional jump (goto).
            MirTerminator::Jump { target } => {
                let target_bb = bb_map.get(target).expect("Target BB not found");
                // Generates `br label %target`
                self.builder.build_unconditional_branch(*target_bb);
            }
            // Handles conditional jump (if/else).
            MirTerminator::CondJump {
                cond,
                then_block,
                else_block,
            } => {
                let cond_val = self.resolve_value(cond).into_int_value();
                let then_bb = bb_map.get(then_block).expect("Then BB not found");
                let else_bb = bb_map.get(else_block).expect("Else BB not found");
                // Generates `br i1 %cond, label %then, label %else`
                self.builder
                    .build_conditional_branch(cond_val, *then_bb, *else_bb);
            }
        }
    }

    /// Generates LLVM IR for a block that is part of a loop structure.
    /// This method:
    /// - Handles loop body markers and identifies loop variables and blocks.
    /// - Processes instructions, including loop setup and element loading with reference counting.
    /// - Manages incrementing loop variables and jumping back to condition blocks.
    /// - Handles block terminators for control flow.
    /// It ensures correct loop semantics and memory management for complex loop constructs.
    pub fn generate_block_with_loops(
        &mut self,
        block: &MirBlock,
        func: FunctionValue<'ctx>,
        bb_map: &HashMap<String, inkwell::basic_block::BasicBlock<'ctx>>,
    ) {
        let bb = bb_map.get(&block.label).unwrap();
        self.builder.position_at_end(*bb);

        // Track if this is a loop body block
        let mut is_loop_body = false;
        let mut loop_var = None;
        let mut loop_cond_bb = None;
        let mut loop_increment_bb = None;

        // Check if any instruction marks this as a loop body
        for instr in &block.instrs {
            match instr {
                MirInstr::LoopBodyMarker {
                    var,
                    cond_block,
                    increment_block,
                } => {
                    is_loop_body = true;
                    loop_var = Some(var.clone());
                    loop_cond_bb = bb_map.get(cond_block).copied();
                    loop_increment_bb = bb_map.get(increment_block).copied();
                }
                _ => {}
            }
        }

        // Generate all instructions in the block.
        for instr in &block.instrs {
            // Check for loop-related instructions and handle accordingly.
            match instr {
                MirInstr::ForRange { .. }
                | MirInstr::ForArray { .. }
                | MirInstr::ForMap { .. }
                | MirInstr::ForInfinite { .. } => {
                    self.generate_for_loop(instr, bb_map);
                }

                MirInstr::LoadArrayElement { dest, array, index } => {
                    // Load array element with RC handling.
                    let array_ptr = self.resolve_value(array).into_pointer_value();
                    let index_val = self.resolve_value(index).into_int_value();

                    // Determine if elements are strings for RC logic.
                    let is_string =
                        self.heap_strings.contains(array) || self.array_contains_strings(array);

                    let elem_type = self.get_array_element_type(array);
                    let elem_val =
                        self.load_array_element_with_rc(array_ptr, index_val, elem_type, is_string);

                    // Store in destination variable.
                    if let Some(symbol) = self.symbols.get(dest) {
                        self.builder.build_store(symbol.ptr, elem_val).unwrap();
                    }
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
                    let pair_type = self.get_map_pair_type(map);

                    let (key_val, val_val) = self.load_map_pair_with_rc(
                        map_ptr,
                        index_val,
                        pair_type,
                        key_is_string,
                        val_is_string,
                    );

                    // Store key and value.
                    if let Some(symbol) = self.symbols.get(key_dest) {
                        self.builder.build_store(symbol.ptr, key_val).unwrap();
                    }
                    if let Some(symbol) = self.symbols.get(val_dest) {
                        self.builder.build_store(symbol.ptr, val_val).unwrap();
                    }
                }

                MirInstr::Break { .. } | MirInstr::Continue { .. } => {
                    self.generate_for_loop(instr, bb_map);
                    return; // These terminate the block
                }

                _ => {
                    self.generate_instr(instr);
                }
            }
        }

        // If this is a loop body, handle increment and loop back.
        if is_loop_body {
            if let (Some(var), Some(inc_bb), Some(cond_bb)) =
                (loop_var, loop_increment_bb, loop_cond_bb)
            {
                // Generate increment: var = var + 1
                if let Some(symbol) = self.symbols.get(&var) {
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

                // Jump back to condition block for next loop iteration.
                self.builder.build_unconditional_branch(cond_bb).unwrap();
                return;
            }
        }

        // Handle terminator if present (return, jump, cond jump).
        if let Some(instr) = &block.terminator {
            let term = match instr {
                MirInstr::Return { values } => crate::mir::mir::MirTerminator::Return {
                    values: values.clone(),
                },
                MirInstr::Jump { target } => crate::mir::mir::MirTerminator::Jump {
                    target: target.clone(),
                },
                MirInstr::CondJump {
                    cond,
                    then_block,
                    else_block,
                } => crate::mir::mir::MirTerminator::CondJump {
                    cond: cond.clone(),
                    then_block: then_block.clone(),
                    else_block: else_block.clone(),
                },
                _ => return,
            };
            self.generate_terminator(&term, func, bb_map);
        }
    }

    /// Enhanced cleanup for loop exit with RC
    /// Cleans up heap-allocated loop variables when exiting a loop.
    /// This method:
    /// - Decrements reference counts for strings, arrays, and maps.
    /// - Handles cleanup of composite string pointers in arrays and maps.
    /// - Ensures proper memory management and avoids leaks in loop constructs.
    pub fn generate_loop_cleanup(&mut self, loop_vars: &[String]) {
        // When exiting a loop, clean up any heap-allocated loop variables.
        for var in loop_vars {
            if self.heap_strings.contains(var) {
                self.emit_decref(var);
            }
            if self.heap_arrays.contains(var) {
                // Clean up strings in array elements if needed.
                if let Some(str_ptrs) = self.composite_string_ptrs.get(var) {
                    for str_ptr in str_ptrs {
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
                self.emit_decref(var);
            }
            if self.heap_maps.contains(var) {
                // Clean up strings in map if needed.
                if let Some(str_names) = self.composite_strings.get(var) {
                    for str_name in str_names {
                        if let Some(val) = self.temp_values.get(str_name) {
                            if val.is_pointer_value() {
                                let data_ptr = val.into_pointer_value();
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
                }
                self.emit_decref(var);
            }
        }
    }
}
