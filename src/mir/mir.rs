use crate::parser::ast::{AstNode, Pattern, TypeNode};

/// Mid-level Intermediate Representation for the language
/// Contains the core data structures used after AST parsing
/// and before LLVM IR generation

/// Represents a complete MIR program with functions and globals
#[derive(Debug, Clone)]
pub struct MirProgram {
    pub functions: Vec<MirFunction>, // All function definitions
    pub globals: Vec<MirInstr>,      // Global variable initializations
}

/// A single function in MIR form
#[derive(Debug, Clone)]
pub struct MirFunction {
    pub name: String,                // Function identifier
    pub params: Vec<String>,         // Parameter names
    pub return_type: Option<String>, // Return type (None = void)
    pub blocks: Vec<MirBlock>,       // Basic blocks in SSA form
}

/// A basic block - sequence of instructions with single entry/exit
#[derive(Debug, Clone)]
pub struct MirBlock {
    pub label: String,                // Block identifier
    pub instrs: Vec<MirInstr>,        // Sequential instructions
    pub terminator: Option<MirInstr>, // Block terminator (jump/return)
}

/// MIR instruction types - covers all operations in the language
#[derive(Debug, Clone)]
pub enum MirInstr {
    // Basic constants
    ConstInt {
        name: String,
        value: i64,
    },
    ConstBool {
        name: String,
        value: bool,
    },
    ConstString {
        name: String,
        value: String,
    },

    // Collections
    Array {
        name: String,
        elements: Vec<String>,
    },
    Map {
        name: String,
        entries: Vec<(String, String)>,
    },

    // Range operations (NEW)
    RangeCreate {
        name: String,
        start: String,
        end: String,
        inclusive: bool,
    },

    // Collection operations
    ArrayLen {
        name: String,
        array: String,
    },
    ArrayGet {
        name: String,
        array: String,
        index: String,
    },
    ArraySet {
        array: String,
        index: String,
        value: String,
    },
    MapGet {
        name: String,
        map: String,
        key: String,
    },
    MapSet {
        map: String,
        key: String,
        value: String,
    },

    // Arithmetic operations
    Add(String, String, String), // (dest, lhs, rhs)
    Sub(String, String, String),
    Mul(String, String, String),
    Div(String, String, String),

    // Generic binary operations (covers arithmetic and comparisons)
    BinaryOp(String, String, String, String), // (op, dest, lhs, rhs)

    // Assignment and variable operations
    Assign {
        name: String,
        value: String,
        mutable: bool,
    },

    // Tuple operations
    TupleCreate {
        name: String,
        elements: Vec<String>,
    },
    TupleExtract {
        name: String,
        source: String,
        index: usize,
    },

    // Function related
    Arg {
        name: String,
    },
    Call {
        dest: Vec<String>, // multiple temps for tuple destructuring
        func: String,      // function name
        args: Vec<String>, // arguments (as temp names)
    },
    Return {
        values: Vec<String>,
    },

    // Control flow
    Jump {
        target: String,
    },
    CondJump {
        cond: String,
        then_block: String,
        else_block: String,
    },

    // I/O operations
    Print {
        values: Vec<String>,
    },

    // Struct and enum operations
    StructInit {
        name: String,
        struct_name: String,
        fields: Vec<(String, String)>,
    },
    StructGet {
        name: String,
        struct_instance: String,
        field: String,
    },
    StructSet {
        struct_instance: String,
        field: String,
        value: String,
    },

    EnumInit {
        name: String,
        enum_name: String,
        variant: String,
        value: Option<String>,
    },
    EnumMatch {
        name: String,
        enum_instance: String,
        variant: String,
    },

    // Memory and reference operations (for future use)
    Load {
        name: String,
        address: String,
    },
    Store {
        address: String,
        value: String,
    },
}

// Implement Display trait for MirProgram as human readable format
// No production usecase
impl std::fmt::Display for MirProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print global variables
        if !self.globals.is_empty() {
            writeln!(f, "Globals:")?;
            for instr in &self.globals {
                writeln!(f, "  {}", instr)?;
            }
            writeln!(f)?;
        }

        // Print functions
        for func in &self.functions {
            writeln!(
                f,
                "Function {}({}) -> {}",
                func.name,
                func.params.join(", "),
                func.return_type.clone().unwrap_or("Void".to_string())
            )?;
            for block in &func.blocks {
                writeln!(f, "  {}:", block.label)?;
                for instr in &block.instrs {
                    writeln!(f, "    {}", instr)?;
                }
                if let Some(term) = &block.terminator {
                    writeln!(f, "    {}", term)?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for MirInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MirInstr::ConstInt { name, value } => write!(f, "Let {} = {}", name, value),
            MirInstr::ConstBool { name, value } => write!(f, "Let {} = {}", name, value),
            MirInstr::ConstString { name, value } => write!(f, "Let {} = \"{}\"", name, value),
            MirInstr::Array { name, elements } => {
                write!(f, "Let {} = [{}]", name, elements.join(", "))
            }
            MirInstr::Map { name, entries } => {
                let entries_str: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("\"{}\": {}", k, v))
                    .collect();
                write!(f, "Let {} = {{ {} }}", name, entries_str.join(", "))
            }
            MirInstr::Assign {
                name,
                value,
                mutable,
            } => {
                let mut_str = if *mutable { "mut " } else { "" };
                write!(f, "{}{} = {}", mut_str, name, value)
            }
            MirInstr::Arg { name } => write!(f, "Arg {}", name),
            MirInstr::Return { values } => write!(f, "ret ({})", values.join(", ")),
            MirInstr::Call { dest, func, args } => {
                if dest.len() == 1 {
                    write!(f, "Let {} = {}({})", dest[0], func, args.join(", "))
                } else {
                    write!(f, "Let {} = {}({})", dest.join(", "), func, args.join(", "))
                }
            }
            MirInstr::Add(dest, lhs, rhs) => write!(f, "Let {} = add {}, {}", dest, lhs, rhs),
            MirInstr::Sub(dest, lhs, rhs) => write!(f, "Let {} = sub {}, {}", dest, lhs, rhs),
            MirInstr::Mul(dest, lhs, rhs) => write!(f, "Let {} = mul {}, {}", dest, lhs, rhs),
            MirInstr::Div(dest, lhs, rhs) => write!(f, "Let {} = div {}, {}", dest, lhs, rhs),

            MirInstr::BinaryOp(op, dest, lhs, rhs) => match op.as_str() {
                "gt" => write!(f, "Let {} = gt {}, {}", dest, lhs, rhs),
                "lt" => write!(f, "Let {} = lt {}, {}", dest, lhs, rhs),
                "%" => write!(f, "Let {} = rem {}, {}", dest, lhs, rhs), // <-- add this
                _ => write!(f, "Let {} = {} {}, {}", dest, op, lhs, rhs),
            },

            MirInstr::Jump { target } => write!(f, "jump {}", target),
            MirInstr::CondJump {
                cond,
                then_block,
                else_block,
            } => {
                write!(f, "if {} then {} else {}", cond, then_block, else_block)
            }
            MirInstr::Print { values } => write!(f, "print({})", values.join(", ")),

            MirInstr::StructInit {
                name,
                struct_name,
                fields,
            } => {
                let f_str: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                write!(f, "{} = {} {{ {} }}", name, struct_name, f_str.join(", "))
            }
            MirInstr::EnumInit {
                name,
                enum_name,
                variant,
                value,
            } => {
                if let Some(v) = value {
                    write!(f, "{} = {}::{}({})", name, enum_name, variant, v)
                } else {
                    write!(f, "{} = {}::{}", name, enum_name, variant)
                }
            }

            MirInstr::TupleExtract {
                name,
                source,
                index,
            } => {
                write!(f, "Let {} = extract({}, {})", name, source, index)
            }
            MirInstr::ArrayLen { name, array } => {
                write!(f, "Let {} = len({})", name, array)
            }
            MirInstr::ArrayGet { name, array, index } => {
                write!(f, "Let {} = {}[{}]", name, array, index)
            }
            MirInstr::RangeCreate {
                name,
                start,
                end,
                inclusive,
            } => {
                let op = if *inclusive { "..=" } else { ".." };
                write!(f, "Let {} = {}{}{}", name, start, op, end)
            }

            // Catch-all for any future variants
            _ => write!(f, "<unimplemented MIR instruction>"),
        }
    }
}
