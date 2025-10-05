use crate::codegen::CodeGen;
use crate::mir::mir::{CodegenBlock, MirBlock, MirFunction, MirInstr, MirProgram, MirTerminator};
use inkwell::types::BasicType;
use inkwell::values::FunctionValue;
use std::collections::HashMap;

impl<'ctx> CodeGen<'ctx> {
    /// The main entry point for code generation. Processes the entire MIR program.
    pub fn generate_program(&mut self, program: &MirProgram) {
        // Initialize RC runtime FIRST
        self.init_rc_runtime();

        // Store the global instructions for later use (e.g., initialization).
        self.globals = program.globals.clone();

        // --- PRE-PROCESSING ---
        // Scan all global instructions to identify strings involved in concatenation.
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
    pub fn generate_function(&mut self, func: &MirFunction) -> FunctionValue<'ctx> {
        // Define the LLVM function signature (return type and parameter types).
        let fn_type = match &func.return_type {
            Some(ret_ty) => self.get_llvm_type(ret_ty).fn_type(
                &func
                    .params
                    .iter()
                    .map(|_| self.context.i32_type().into()) // Assuming parameters are i32 for now
                    .collect::<Vec<_>>(),
                false,
            ),
            None => self.context.void_type().fn_type(&[], false),
        };

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
        for (i, param) in func.params.iter().enumerate() {
            let param_val = llvm_func.get_nth_param(i as u32).unwrap();
            let alloca = self
                .builder
                .build_alloca(param_val.get_type(), param)
                .expect("Failed to allocate function parameter");

            self.builder.build_store(alloca, param_val);

            // Register the parameter in the symbol table for future lookups.
            self.symbols.insert(
                param.clone(),
                crate::codegen::Symbol {
                    ptr: alloca,
                    ty: param_val.get_type(),
                },
            );
        }

        // Convert MIR block terminators to a unified structure for easier handling.
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

    pub fn generate_block(
        &mut self,
        block: &MirBlock,
        func: FunctionValue<'ctx>,
        bb_map: &HashMap<String, inkwell::basic_block::BasicBlock<'ctx>>,
    ) {
        let bb = bb_map.get(&block.label).unwrap();
        self.builder.position_at_end(*bb);

        for instr in &block.instrs {
            self.generate_instr(instr);
        }

        if let Some(instr) = &block.terminator {
            let term = match instr {
                // ... (Logic to convert MirInstr to MirTerminator, similar to above)
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
                _ => panic!("Unexpected instruction in block terminator"),
            };
            self.generate_terminator(&term, func, bb_map);
        }
    }

    /// Generates the final instruction of a basic block (the control flow transfer).
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
}
