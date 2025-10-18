use crate::codegen::core::CodeGen;
use crate::mir::MirInstr;
use inkwell::values::BasicValueEnum;
use inkwell::IntPredicate;

impl<'ctx> CodeGen<'ctx> {
    pub fn generate_binary_op(
        &mut self,
        op: &str,
        dst: &str,
        lhs: &str,
        rhs: &str,
    ) -> Option<inkwell::values::BasicValueEnum<'ctx>> {
        // Check if this is a string concatenation (add operation with pointer operands)
        let lhs_val = self.resolve_value(lhs);
        let rhs_val = self.resolve_value(rhs);

        // If both are pointers and operation is "add", treat as string concatenation
        if op == "add" && lhs_val.is_pointer_value() && rhs_val.is_pointer_value() {
            // Delegate to string concatenation logic
            return self.generate_instr(&crate::mir::MirInstr::StringConcat {
                name: dst.to_string(),
                left: lhs.to_string(),
                right: rhs.to_string(),
            });
        }

        // Otherwise, treat as integer operations
        let lhs_val = lhs_val.into_int_value();
        let rhs_val = rhs_val.into_int_value();

        let res = match op {
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
        self.temp_values.insert(dst.to_string(), res.into());

        // If this temp was pre-allocated as a symbol (cross-block usage), store it there too
        if let Some(sym) = self.symbols.get(dst) {
            self.builder.build_store(sym.ptr, res).unwrap();
        }

        Some(res.into())
    }
}
