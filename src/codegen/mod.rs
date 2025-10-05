use inkwell::{
    builder::Builder,
    context::Context,
    module::Module,
    passes::PassManager,
    types::BasicTypeEnum,
    values::{BasicValueEnum, FunctionValue, PointerValue},
};
use std::collections::HashMap;

pub mod builder;
pub mod functions;
pub mod globals;
pub mod rc_runtime;

/// Represents a variable allocated on the stack or in global memory.
/// Stores the variable's pointer and its LLVM type.
#[derive(Debug)]
pub struct Symbol<'ctx> {
    pub ptr: PointerValue<'ctx>,
    pub ty: BasicTypeEnum<'ctx>,
}

/// The main context structure for generating LLVM Intermediate Representation (IR).
/// It holds all the necessary LLVM components and symbol tables.
pub struct CodeGen<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>, // The container for all generated code (globals, functions, types)
    pub builder: Builder<'ctx>, // The tool used to insert instructions into blocks
    pub fpm: PassManager<FunctionValue<'ctx>>, // Function Pass Manager for optimization (e.g., dead code elimination)
    pub symbols: HashMap<String, Symbol<'ctx>>, // Symbol table for local variables (maps names to stack pointers)
    pub temp_values: HashMap<String, BasicValueEnum<'ctx>>, // Stores temporary constant values (used for building complex constants)
    pub globals: Vec<crate::mir::mir::MirInstr>, // List of Intermediate Representation instructions for global definitions
    pub temp_strings: HashMap<String, String>, // Stores original Rust string values (used during string concatenation/definition)
    pub strings_to_concat: std::collections::HashSet<String>, // Tracks strings that need concatenation logic

    // NEW: RC runtime functions
    pub incref_fn: Option<FunctionValue<'ctx>>,
    pub decref_fn: Option<FunctionValue<'ctx>>,

    pub heap_strings: std::collections::HashSet<String>,

    pub heap_arrays: std::collections::HashSet<String>,
    pub heap_maps: std::collections::HashSet<String>,

    pub composite_strings: HashMap<String, Vec<String>>,
    pub composite_string_ptrs: HashMap<String, Vec<BasicValueEnum<'ctx>>>,
}

impl<'ctx> CodeGen<'ctx> {
    /// Creates a new CodeGen instance, initializing LLVM structures.
    pub fn new(module_name: &str, context: &'ctx Context) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        let fpm: PassManager<FunctionValue> = PassManager::create(&module);

        Self {
            context,
            module,
            builder,
            fpm,
            symbols: HashMap::new(),
            temp_values: HashMap::new(),
            globals: Vec::new(),
            temp_strings: HashMap::new(),
            strings_to_concat: std::collections::HashSet::new(),

            incref_fn: None,
            decref_fn: None,

            heap_strings: std::collections::HashSet::new(),

            heap_arrays: std::collections::HashSet::new(),
            heap_maps: std::collections::HashSet::new(),

            composite_strings: HashMap::new(),

            composite_string_ptrs: HashMap::new(),
        }
    }

    /// Prints the final generated LLVM IR to standard error (stderr).
    pub fn dump(&self) {
        self.module.print_to_stderr();
    }
}
