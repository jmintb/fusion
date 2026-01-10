use std::collections::HashMap;

use anyhow::{bail, Result};
use melior::dialect::arith::{self};
use melior::ir::{BlockRef, Value};

use crate::ast::nodes::Operator;
use crate::backend::mlir::codegen::{CodeGen, MlirBlockId};
use crate::ir::binary_operations::{BinaryOperation, Signage};
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

    if binary_operation.is_floating_point_operation(code_gen_context.current_fn_types())? {
        generate_floating_point_binary_operation(
            binary_operation,
            code_gen_context,
            current_mlir_block,
            block_references,
            variable_store,
            first_operand_value,
            second_operand_value,
        )
    } else if binary_operation.is_integer_operation(code_gen_context.current_fn_types())? {
        generate_integer_binary_operation(
            binary_operation,
            code_gen_context,
            current_mlir_block,
            block_references,
            variable_store,
            first_operand_value,
            second_operand_value,
        )
    } else {
        bail!("expected either a integer or floating point binary operation")
    }
}

fn generate_floating_point_binary_operation(
    binary_operation: &BinaryOperation,
    code_gen_context: &'_ CodeGen,
    current_mlir_block: MlirBlockId,
    block_references: &HashMap<usize, BlockRef<'_, '_>>,
    variable_store: &HashMap<Ssaid, Value<'_, '_>>,
    first_operand_value: Value<'_, '_>,
    second_operand_value: Value<'_, '_>,
) -> Result<()> {
    // TODO: We are using ordered comparisons for now but should perform some design work
    // is this area to figure out how we want to treat floats.

    let operation = match binary_operation.operation_id {
        Operator::Inequality => melior::dialect::arith::cmpf(
            code_gen_context.context,
            arith::CmpfPredicate::One,
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        Operator::Equality => melior::dialect::arith::cmpf(
            code_gen_context.context,
            arith::CmpfPredicate::Oeq,
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        Operator::LessThan => melior::dialect::arith::cmpf(
            code_gen_context.context,
            arith::CmpfPredicate::Olt,
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        Operator::LessThanOrEqual => melior::dialect::arith::cmpf(
            code_gen_context.context,
            arith::CmpfPredicate::Ole,
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        Operator::GreaterThanOrEqual => melior::dialect::arith::cmpf(
            code_gen_context.context,
            arith::CmpfPredicate::Oge,
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),

        Operator::GreaterThan => melior::dialect::arith::cmpf(
            code_gen_context.context,
            arith::CmpfPredicate::Ogt,
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        Operator::Addition => melior::dialect::arith::addf(
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        Operator::Subtraction => melior::dialect::arith::subf(
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        Operator::Multiplication => melior::dialect::arith::mulf(
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        Operator::Division => melior::dialect::arith::divf(
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
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

fn generate_integer_binary_operation(
    binary_operation: &BinaryOperation,
    code_gen_context: &'_ CodeGen,
    current_mlir_block: MlirBlockId,
    block_references: &HashMap<usize, BlockRef<'_, '_>>,
    variable_store: &HashMap<Ssaid, Value<'_, '_>>,
    first_operand_value: Value<'_, '_>,
    second_operand_value: Value<'_, '_>,
) -> Result<()> {
    let signage = binary_operation.signage(code_gen_context.current_fn_types())?;

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
        Operator::LessThan => {
            let predicate = match signage {
                Signage::Signed => arith::CmpiPredicate::Slt,
                Signage::Unsigned => arith::CmpiPredicate::Ugt,
            };

            melior::dialect::arith::cmpi(
                code_gen_context.context,
                predicate,
                first_operand_value,
                second_operand_value,
                code_gen_context.unknown_location(),
            )
        }
        Operator::LessThanOrEqual => {
            let predicate = match signage {
                Signage::Signed => arith::CmpiPredicate::Sle,
                Signage::Unsigned => arith::CmpiPredicate::Uge,
            };

            melior::dialect::arith::cmpi(
                code_gen_context.context,
                predicate,
                first_operand_value,
                second_operand_value,
                code_gen_context.unknown_location(),
            )
        }
        Operator::GreaterThanOrEqual => {
            let predicate = match signage {
                Signage::Signed => arith::CmpiPredicate::Sge,
                Signage::Unsigned => arith::CmpiPredicate::Uge,
            };

            melior::dialect::arith::cmpi(
                code_gen_context.context,
                predicate,
                first_operand_value,
                second_operand_value,
                code_gen_context.unknown_location(),
            )
        }

        Operator::GreaterThan => {
            let predicate = match signage {
                Signage::Signed => arith::CmpiPredicate::Sgt,
                Signage::Unsigned => arith::CmpiPredicate::Ugt,
            };

            melior::dialect::arith::cmpi(
                code_gen_context.context,
                predicate,
                first_operand_value,
                second_operand_value,
                code_gen_context.unknown_location(),
            )
        }
        Operator::Addition => melior::dialect::arith::addi(
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        Operator::Subtraction => melior::dialect::arith::subi(
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        Operator::Multiplication => melior::dialect::arith::muli(
            first_operand_value,
            second_operand_value,
            code_gen_context.unknown_location(),
        ),
        Operator::Division => {
            match signage {
                Signage::Signed => melior::dialect::arith::divsi(
                    first_operand_value,
                    second_operand_value,
                    code_gen_context.unknown_location(),
                ),
                Signage::Unsigned => melior::dialect::arith::divui(
                    // TODO: add some information during type checking to mark if this is signed or not.
                    first_operand_value,
                    second_operand_value,
                    code_gen_context.unknown_location(),
                ),
            }
        }
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
