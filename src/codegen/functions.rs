use crate::codegen::core::CodeGen;
use crate::mir::mir::{CodegenBlock, MirBlock, MirFunction, MirInstr, MirProgram, MirTerminator};
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::types::StructType;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::FunctionValue;
use inkwell::AddressSpace;
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

        // Pre-scan and declare all functions for forward references
        // This allows functions to call each other regardless of definition order
        for func in &program.functions {
            self.predeclare_function(func);
        }

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

    // ADD THIS NEW METHOD:
    fn predeclare_function(&mut self, func: &MirFunction) {
        if self.declared_functions.contains(&func.name) {
            return;
        }

        // Build parameter types
        let param_types: Vec<BasicMetadataTypeEnum> = func
            .param_types
            .iter()
            .map(|type_opt| self.map_type_to_llvm(type_opt))
            .collect();

        // Determine return type
        let fn_type = if func.name == "main" {
            // Force main to be i32 () for C/Clang compatibility
            self.context.i32_type().fn_type(&param_types, false)
        } else if let Some(ref ret_type_str) = func.return_type {
            if ret_type_str.contains("Void") {
                self.context.void_type().fn_type(&param_types, false)
            } else if ret_type_str.contains("String") || ret_type_str.contains("Str") {
                self.context
                    .ptr_type(AddressSpace::default())
                    .fn_type(&param_types, false)
            } else if ret_type_str.contains("Array") || ret_type_str.contains("Map") {
                self.context
                    .ptr_type(AddressSpace::default())
                    .fn_type(&param_types, false)
            } else {
                self.context.i32_type().fn_type(&param_types, false)
            }
        } else {
            self.context.void_type().fn_type(&param_types, false)
        };

        // Declare function
        self.module.add_function(&func.name, fn_type, None);
        self.declared_functions.insert(func.name.clone());
    }

    fn map_type_to_llvm(&self, type_opt: &Option<String>) -> BasicMetadataTypeEnum<'ctx> {
        if let Some(type_str) = type_opt {
            if type_str.contains("String") || type_str.contains("Str") {
                self.context.ptr_type(AddressSpace::default()).into()
            } else if type_str.contains("Array") || type_str.contains("Map") {
                self.context.ptr_type(AddressSpace::default()).into()
            } else {
                self.context.i32_type().into()
            }
        } else {
            self.context.i32_type().into()
        }
    }

    /// Creates a minimal `main` function (`i32 ()`) that returns 0.
    /// This is a fallback to guarantee the presence of a valid entry point in the generated binary.
    /// Also executes any global-scope runtime statements (like print).
    pub fn generate_default_main(&mut self) {
        let main_type = self.context.i32_type().fn_type(&[], false);
        let main_func = self.module.add_function("main", main_type, None);

        let entry_bb = self.context.append_basic_block(main_func, "entry");
        self.builder.position_at_end(entry_bb);

        // Execute any runtime instructions from global scope (like Print, BinaryOp for runtime values)
        for instr in &self.globals.clone() {
            match instr {
                MirInstr::Print { .. } => {
                    self.generate_instr(instr);
                }
                MirInstr::BinaryOp(_, _, _, _) => {
                    // Generate runtime binary operations that weren't constant-folded
                    self.generate_instr(instr);
                }
                _ => {
                    // Other instructions are already handled in generate_global
                }
            }
        }

        let zero = self.context.i32_type().const_int(0, false);
        // Generates the `ret i32 0` instruction.
        self.builder.build_return(Some(&zero)).unwrap();
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
        // Clear symbols table to prevent conflicts between functions
        self.symbols.clear();
        self.temp_values.clear();
        self.heap_strings.clear();
        self.heap_arrays.clear();
        self.heap_maps.clear();
        self.array_metadata.clear();
        self.map_metadata.clear();
        self.composite_string_ptrs.clear();
        self.composite_strings.clear();

        // Store function return type for RC tracking when this function is called
        if let Some(ref ret_type_str) = func.return_type {
            self.function_return_types
                .insert(func.name.clone(), ret_type_str.clone());
        }

        // Track function parameters for RC handling on return
        self.current_function_params.clear();
        for (i, param_name) in func.params.iter().enumerate() {
            let param_type = func.param_types.get(i).and_then(|t| t.clone());
            self.current_function_params
                .push((param_name.clone(), param_type));
        }

        // Build parameter types based on function signature
        let param_types: Vec<BasicMetadataTypeEnum> = func
            .param_types
            .iter()
            .map(|type_opt| {
                if let Some(type_str) = type_opt {
                    // Map MIR type strings to LLVM types
                    if type_str.contains("String") || type_str.contains("Str") {
                        self.context.ptr_type(AddressSpace::default()).into()
                    } else if type_str.contains("Array") {
                        self.context.ptr_type(AddressSpace::default()).into()
                    } else if type_str.contains("Map") {
                        self.context.ptr_type(AddressSpace::default()).into()
                    } else {
                        self.context.i32_type().into()
                    }
                } else {
                    self.context.i32_type().into()
                }
            })
            .collect();

        // Determine return type and create function signature
        let fn_type = if func.name == "main" {
            // Force main to be i32 () for C/Clang compatibility
            self.context.i32_type().fn_type(&param_types, false)
        } else if let Some(ref ret_type_str) = func.return_type {
            // Map MIR type strings to LLVM types
            if ret_type_str.contains("Void") {
                self.context.void_type().fn_type(&param_types, false)
            } else if ret_type_str.contains("String") || ret_type_str.contains("Str") {
                self.context
                    .ptr_type(AddressSpace::default())
                    .fn_type(&param_types, false)
            } else if ret_type_str.contains("Array") {
                self.context
                    .ptr_type(AddressSpace::default())
                    .fn_type(&param_types, false)
            } else if ret_type_str.contains("Map") {
                self.context
                    .ptr_type(AddressSpace::default())
                    .fn_type(&param_types, false)
            } else {
                self.context.i32_type().fn_type(&param_types, false)
            }
        } else {
            self.context.void_type().fn_type(&param_types, false)
        };

        // Check if function was already declared (for forward references/imports)
        let llvm_func = if let Some(existing_func) = self.module.get_function(&func.name) {
            // Verify signature matches
            if existing_func.get_type() == fn_type {
                existing_func
            } else {
                eprintln!(
                    "Warning: Function {} signature mismatch between declaration and definition",
                    func.name
                );
                eprintln!("  Declared: {:?}", existing_func.get_type());
                eprintln!("  Expected: {:?}", fn_type);
                // Create new function with correct signature
                self.module.add_function(&func.name, fn_type, None)
            }
        } else {
            self.module.add_function(&func.name, fn_type, None)
        };

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

            // Get the correct type for this parameter
            let param_type = if let Some(Some(ref type_str)) = func.param_types.get(i) {
                // Map MIR type strings to LLVM types
                if type_str.contains("String") || type_str.contains("Str") {
                    self.context.ptr_type(AddressSpace::default()).into()
                } else if type_str.contains("Array") {
                    self.context.ptr_type(AddressSpace::default()).into()
                } else if type_str.contains("Map") {
                    self.context.ptr_type(AddressSpace::default()).into()
                } else {
                    self.context.i32_type().into()
                }
            } else {
                self.context.i32_type().into()
            };

            let alloca = self
                .builder
                .build_alloca(param_type, param)
                .expect("Failed to allocate function parameter");

            self.builder.build_store(alloca, param_val);

            // Register the parameter in the symbol table for future lookups.
            self.symbols.insert(
                param.clone(),
                crate::codegen::Symbol {
                    ptr: alloca,
                    ty: param_type,
                },
            );
        }

        // Pre-allocate variables that are used across multiple blocks
        // This is necessary for proper SSA form and cross-block variable access
        use std::collections::HashSet;
        let mut defined_vars: HashMap<String, HashSet<String>> = HashMap::new(); // block -> vars defined
        let mut used_vars: HashMap<String, HashSet<String>> = HashMap::new(); // block -> vars used

        // Scan all blocks to find variable definitions and uses
        for block in &func.blocks {
            let mut block_defs = HashSet::new();
            let mut block_uses = HashSet::new();

            for instr in &block.instrs {
                match instr {
                    crate::mir::MirInstr::Assign { name, value, .. } => {
                        block_defs.insert(name.clone());
                        if !value.starts_with('%')
                            && !value.parse::<i32>().is_ok()
                            && value != "true"
                            && value != "false"
                        {
                            block_uses.insert(value.clone());
                        }
                    }
                    crate::mir::MirInstr::BinaryOp(_, _, left, right) => {
                        if !left.starts_with('%')
                            && !left.parse::<i32>().is_ok()
                            && left != "true"
                            && left != "false"
                        {
                            block_uses.insert(left.clone());
                        }
                        if !right.starts_with('%')
                            && !right.parse::<i32>().is_ok()
                            && right != "true"
                            && right != "false"
                        {
                            block_uses.insert(right.clone());
                        }
                    }
                    crate::mir::MirInstr::ArrayLen { array, .. } => {
                        if !array.starts_with('%') {
                            block_uses.insert(array.clone());
                        }
                    }
                    crate::mir::MirInstr::ArrayGet { array, index, .. } => {
                        if !array.starts_with('%') {
                            block_uses.insert(array.clone());
                        }
                        if !index.starts_with('%') {
                            block_uses.insert(index.clone());
                        }
                    }
                    _ => {}
                }
            }

            // Check terminator for variable uses
            if let Some(term) = &block.terminator {
                match term {
                    crate::mir::MirInstr::CondJump { cond, .. } => {
                        if !cond.starts_with('%')
                            && !cond.parse::<i32>().is_ok()
                            && cond != "true"
                            && cond != "false"
                        {
                            block_uses.insert(cond.clone());
                        }
                    }
                    _ => {}
                }
            }

            defined_vars.insert(block.label.clone(), block_defs);
            used_vars.insert(block.label.clone(), block_uses);
        }

        // Find variables that are defined in one block and used in another
        let mut cross_block_vars = HashSet::new();
        for (use_block, uses) in &used_vars {
            for var in uses {
                // Check if this variable is defined in a different block
                let mut defined_elsewhere = false;
                for (def_block, defs) in &defined_vars {
                    if def_block != use_block && defs.contains(var) {
                        defined_elsewhere = true;
                        break;
                    }
                }
                if defined_elsewhere {
                    cross_block_vars.insert(var.clone());
                }
            }
        }

        // Determine variable types by scanning instructions that define them
        let mut var_types: HashMap<String, BasicTypeEnum<'ctx>> = HashMap::new();
        for block in &func.blocks {
            for instr in &block.instrs {
                match instr {
                    // Arrays are always pointers
                    crate::mir::MirInstr::Array { name, .. } => {
                        var_types.insert(
                            name.clone(),
                            self.context.ptr_type(AddressSpace::default()).into(),
                        );
                    }
                    // Maps are always pointers
                    crate::mir::MirInstr::Map { name, .. } => {
                        var_types.insert(
                            name.clone(),
                            self.context.ptr_type(AddressSpace::default()).into(),
                        );
                    }
                    // Strings are always pointers
                    crate::mir::MirInstr::ConstString { name, .. } => {
                        var_types.insert(
                            name.clone(),
                            self.context.ptr_type(AddressSpace::default()).into(),
                        );
                    }
                    // Variables with "_array" or "_map" suffix are pointers
                    // BUT: exclude index variables (ending with __index)
                    crate::mir::MirInstr::Assign { name, value, .. } => {
                        // Index variables are always i32
                        if name.ends_with("__index") || name.ends_with("_end") {
                            var_types.insert(name.clone(), self.context.i32_type().into());
                        } else if name.ends_with("_array")
                            || name.ends_with("_map")
                            || name.ends_with("item_array")
                            || name.ends_with("_ptr")
                        {
                            // Only mark as pointer if it's NOT an index variable
                            var_types.insert(
                                name.clone(),
                                self.context.ptr_type(AddressSpace::default()).into(),
                            );
                        }
                        // If assigned from a known pointer type, it's also a pointer
                        // BUT: not if this is an index variable
                        else if !name.ends_with("__index") && !name.ends_with("_end") {
                            if let Some(val_type) = var_types.get(value) {
                                if val_type.is_pointer_type() {
                                    var_types.insert(name.clone(), *val_type);
                                }
                            }
                        }
                    }
                    // ArrayLen results are i32
                    crate::mir::MirInstr::ArrayLen { name, .. } => {
                        var_types.insert(name.clone(), self.context.i32_type().into());
                    }
                    // MapLen results are i32
                    crate::mir::MirInstr::MapLen { name, .. } => {
                        var_types.insert(name.clone(), self.context.i32_type().into());
                    }
                    // Integer constants are i32
                    crate::mir::MirInstr::ConstInt { name, .. } => {
                        var_types.insert(name.clone(), self.context.i32_type().into());
                    }
                    // Boolean constants are i32
                    crate::mir::MirInstr::ConstBool { name, .. } => {
                        var_types.insert(name.clone(), self.context.i32_type().into());
                    }
                    // Binary operations produce i32
                    crate::mir::MirInstr::BinaryOp(_, name, ..) => {
                        var_types.insert(name.clone(), self.context.i32_type().into());
                    }
                    _ => {}
                }
            }
        }

        // Allocate stack space for cross-block variables with correct types
        for var in &cross_block_vars {
            if !self.symbols.contains_key(var) {
                // Determine the correct type for this variable
                let var_type = var_types.get(var).copied().unwrap_or_else(|| {
                    // Index and end variables are always i32
                    if var.ends_with("__index")
                        || var.ends_with("_end")
                        || var == "i"
                        || var == "counter"
                    {
                        self.context.i32_type().into()
                    }
                    // Default heuristic: if name suggests array/map/string, use ptr, otherwise i32
                    else if var.ends_with("_array")
                        || var.ends_with("_map")
                        || var.ends_with("item_array")
                        || var.ends_with("_ptr")
                    {
                        self.context.ptr_type(AddressSpace::default()).into()
                    } else {
                        self.context.i32_type().into()
                    }
                });

                let alloca = self
                    .builder
                    .build_alloca(var_type, var)
                    .expect("Failed to allocate cross-block variable");

                self.symbols.insert(
                    var.clone(),
                    crate::codegen::Symbol {
                        ptr: alloca,
                        ty: var_type,
                    },
                );
            }
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
            } else {
                // No terminator - check if function is void or non-void
                let fn_type = llvm_func.get_type();
                let return_type = fn_type.get_return_type();

                if return_type.is_none() {
                    // Void function - add cleanup and return void
                    self.generate_function_exit_cleanup();
                    self.builder.build_return(None).unwrap();
                } else {
                    // Non-void function without terminator - this is an unreachable block
                    // Just add cleanup but no return (LLVM will handle unreachable)
                    self.generate_function_exit_cleanup();
                    self.builder.build_unreachable().unwrap();
                }
            }
        }

        llvm_func
    }

    /// Generate cleanup for all RC variables at function exit
    /// This ensures variables in conditional blocks are properly cleaned up
    fn generate_function_exit_cleanup(&mut self) {
        // Collect all RC variables from symbols (including loop arrays)
        let mut heap_strings: Vec<String> = self
            .symbols
            .keys()
            .filter(|name| self.heap_strings.contains(*name))
            .cloned()
            .collect();
        heap_strings.reverse();

        let mut heap_arrays: Vec<String> = self
            .symbols
            .keys()
            .filter(|name| self.heap_arrays.contains(*name))
            .cloned()
            .collect();
        heap_arrays.reverse();

        let mut heap_maps: Vec<String> = self
            .symbols
            .keys()
            .filter(|name| self.heap_maps.contains(*name))
            .cloned()
            .collect();
        heap_maps.reverse();

        // Cleanup composite strings in arrays/maps
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

        // Cleanup arrays
        for var_name in heap_arrays {
            self.emit_decref(&var_name);
        }

        // Cleanup maps
        for var_name in heap_maps {
            self.emit_decref(&var_name);
        }

        // Cleanup strings
        for var_name in heap_strings {
            self.emit_decref(&var_name);
        }

        // Cleanup temporary RC values not in symbols
        let mut temp_str_vars: Vec<String> = self
            .temp_values
            .keys()
            .filter(|name| self.heap_strings.contains(*name) && !self.symbols.contains_key(*name))
            .cloned()
            .collect();
        temp_str_vars.reverse();

        for var_name in temp_str_vars {
            if let Some(val) = self.temp_values.get(&var_name) {
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

                // Determine what value is being returned (if any) to exclude it from cleanup
                let return_value_name = if !values.is_empty() {
                    Some(values[0].as_str())
                } else {
                    None
                };

                // 2. Free arrays (exclude return value)
                let mut heap_array_vars: Vec<String> = self
                    .symbols
                    .keys()
                    .filter(|name| {
                        self.heap_arrays.contains(*name)
                            && return_value_name.map_or(true, |ret| ret != *name)
                    })
                    .cloned()
                    .collect();
                heap_array_vars.reverse();

                for var_name in heap_array_vars {
                    self.emit_decref(&var_name);
                }

                // 3. Free maps (exclude return value)
                let mut heap_map_vars: Vec<String> = self
                    .symbols
                    .keys()
                    .filter(|name| {
                        self.heap_maps.contains(*name)
                            && return_value_name.map_or(true, |ret| ret != *name)
                    })
                    .cloned()
                    .collect();
                heap_map_vars.reverse();

                for var_name in heap_map_vars {
                    self.emit_decref(&var_name);
                }

                // 4. Free simple strings from symbols (exclude return value)
                let mut heap_str_vars: Vec<String> = self
                    .symbols
                    .keys()
                    .filter(|name| {
                        self.heap_strings.contains(*name)
                            && return_value_name.map_or(true, |ret| ret != *name)
                    })
                    .cloned()
                    .collect();
                heap_str_vars.reverse();

                for var_name in heap_str_vars {
                    self.emit_decref(&var_name);
                }

                // 5. Free temporary RC strings (from temp_values not in symbols, exclude return value)
                // These are truly temporary values like function call arguments that are not stored
                let mut temp_str_vars: Vec<String> = self
                    .temp_values
                    .keys()
                    .filter(|name| {
                        self.heap_strings.contains(*name)
                            && !self.symbols.contains_key(*name)
                            && return_value_name.map_or(true, |ret| ret != *name)
                    })
                    .cloned()
                    .collect();
                temp_str_vars.reverse();

                for var_name in temp_str_vars {
                    if let Some(val) = self.temp_values.get(&var_name) {
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

                // 6. Free temporary RC arrays (from temp_values not in symbols, exclude return value)
                let mut temp_array_vars: Vec<String> = self
                    .temp_values
                    .keys()
                    .filter(|name| {
                        self.heap_arrays.contains(*name)
                            && !self.symbols.contains_key(*name)
                            && return_value_name.map_or(true, |ret| ret != *name)
                    })
                    .cloned()
                    .collect();
                temp_array_vars.reverse();

                for var_name in temp_array_vars {
                    if let Some(val) = self.temp_values.get(&var_name) {
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

                // 7. Free temporary RC maps (from temp_values not in symbols, exclude return value)
                let mut temp_map_vars: Vec<String> = self
                    .temp_values
                    .keys()
                    .filter(|name| {
                        self.heap_maps.contains(*name)
                            && !self.symbols.contains_key(*name)
                            && return_value_name.map_or(true, |ret| ret != *name)
                    })
                    .cloned()
                    .collect();
                temp_map_vars.reverse();

                for var_name in temp_map_vars {
                    if let Some(val) = self.temp_values.get(&var_name) {
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

                if values.is_empty() {
                    // Void return - no value
                    self.builder.build_return(None).unwrap();
                } else {
                    let return_value_name = &values[0];

                    // Check if we're returning a function parameter that needs RC increment
                    let needs_incref =
                        self.current_function_params
                            .iter()
                            .any(|(param_name, param_type)| {
                                if param_name == return_value_name {
                                    // Check if this parameter is RC-typed
                                    if let Some(type_str) = param_type {
                                        return type_str.contains("String")
                                            || type_str.contains("Str")
                                            || type_str.contains("Array")
                                            || type_str.contains("Map");
                                    }
                                }
                                false
                            });

                    let val = self.resolve_value(return_value_name);

                    // If returning an RC-typed parameter, incref it (caller expects ownership)
                    if needs_incref && val.is_pointer_value() {
                        let ptr = val.into_pointer_value();
                        let rc_header = unsafe {
                            self.builder.build_in_bounds_gep(
                                self.context.i8_type(),
                                ptr,
                                &[self.context.i32_type().const_int((-8_i32) as u64, true)],
                                "return_rc_header",
                            )
                        }
                        .unwrap();

                        let incref_fn = self.incref_fn.unwrap();
                        self.builder
                            .build_call(incref_fn, &[rc_header.into()], "")
                            .unwrap();
                    }

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
                let cond_val = self.resolve_value(cond);

                // Check if condition is already i1 (from comparison) or i32 (from bool variable)
                let cond_i1 = if cond_val.is_int_value() {
                    let int_val = cond_val.into_int_value();
                    let int_type = int_val.get_type();

                    if int_type.get_bit_width() == 1 {
                        // Already i1, use directly
                        int_val
                    } else {
                        // i32 boolean, convert to i1
                        self.builder
                            .build_int_compare(
                                inkwell::IntPredicate::NE,
                                int_val,
                                self.context.i32_type().const_zero(),
                                "cond_i1",
                            )
                            .unwrap()
                    }
                } else {
                    panic!("Condition value is not an integer type");
                };

                let then_bb = bb_map.get(then_block).expect("Then BB not found");
                let else_bb = bb_map.get(else_block).expect("Else BB not found");
                // Generates `br i1 %cond, label %then, label %else`
                self.builder
                    .build_conditional_branch(cond_i1, *then_bb, *else_bb);
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
