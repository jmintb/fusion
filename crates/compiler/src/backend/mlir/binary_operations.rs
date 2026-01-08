use std::collections::HashMap;

use anyhow::Result;
use melior::dialect::arith;
use melior::ir::{BlockRef, Value};

use crate::ast::nodes::Operator;
use crate::backend::mlir::codegen::{CodeGen, MlirBlockId};
use crate::ir::binary_operations::BinaryOperation;
use crate::ir::Ssaid;

pub fn generate_binary_operation_operation(
    binary_operation: &BinaryOperation,
    code_gen_context: &'_ CodeGen,
    current_mlir_block: MlirBlockId,
    block_references: &HashMap<usize, BlockRef<'_, '_>>,
    variable_store: &HashMap<Ssaid, Value<'_, '_>>,
) -> Result<()> {
    let first_operand_value = code_gen_context.gen_variable_load(
        binary_operation.left_hand_side,
        block_references,
        variable_store,
        current_mlir_block.0,
    )?;
    let second_operand_value = code_gen_context.gen_variable_load(
        binary_operation.right_hand_side,
        block_references,
        variable_store,
        current_mlir_block.0,
    )?;

    let operation = match binary_operation.operation_id {
        Operator::Inequality => melior::dialect::arith::cmpi(
            code_gen_context.context,
            arith::CmpiPredicate::Ne,
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        Operator::Equality => melior::dialect::arith::cmpi(
            code_gen_context.context,
            arith::CmpiPredicate::Eq,
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        _ => todo!(),
    };

    let result = code_gen_context
        .append_operation(current_mlir_block.0, block_references, operation)
        .result(0)?;

    code_gen_context.save_value_to_variable(
        current_mlir_block.0,
        block_references,
        result.into(),
        &binary_operation.reciever,
        variable_store,
        code_gen_context.unknown_location(),
    );

    Ok(())
}
