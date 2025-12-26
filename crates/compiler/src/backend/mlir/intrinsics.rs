//! Tooling for code generation of intrinsic invocations.
//! Intrinsics cover special case functionality which the language can not express natively.
//! They are invoked as function calls where the compiler will emit the underlying
//! instructions inside the callee's stack frame.

use std::collections::HashMap;

use anyhow::Result;
use melior::dialect::{llvm, ods};
use melior::ir::{BlockRef, Operation, Value};

use crate::backend::mlir::codegen::{CodeGen, MlirBlockId};
use crate::ir::intrinsics::{
    ResultfullIntrinsic,
    ResultfullIntrinsicCall,
    ResultlessIntrinsic,
    ResultlessIntrinsicCall,
};
use crate::ir::Ssaid;

pub fn generate_intrinsic_call<'c, 'a>(
    intrinsic_call: &ResultfullIntrinsicCall,
    code_gen_context: &'c CodeGen,
    current_mlir_block: MlirBlockId,
    block_references: &HashMap<usize, BlockRef<'c, 'a>>,
    variable_store: &HashMap<Ssaid, Value<'c, 'a>>,
) -> Result<()> {
    let argument_values = code_gen_context.load_variables(
        &intrinsic_call.arguments,
        block_references,
        variable_store,
        current_mlir_block,
    )?;

    match intrinsic_call.intrinsic {
        ResultfullIntrinsic::Offset => {
            debug_assert!(argument_values.len() == 2);

            let base_pointer = argument_values[0];
            let offset = argument_values[1];

            // We use an integer consisting of 8 bits as the element type
            // for the GEP operation.
            // That is because we assume the offset is specified in number of bytes.
            // A byte is 8 bits which matches our element type.
            // Eventually we will want to make the element type defined in the intrinsic call.
            // TODO: revaluate this approach once the core types are more well defined, we likely
            // don't want to hardcode element type assumption.
            let integer_type_with_width_8 = code_gen_context.signless_integer_type(8);
            let location = melior::ir::Location::unknown(code_gen_context.context);

            let gep_operation = llvm::get_element_ptr_dynamic(
                code_gen_context.context,
                base_pointer,
                &[offset],
                integer_type_with_width_8.into(),
                code_gen_context.opaque_pointer_type(),
                location,
            );

            let gep_operation_value: Value = code_gen_context
                .append_operation(current_mlir_block.0, block_references, gep_operation)
                .result(0)?
                .into();

            code_gen_context.save_value_to_variable(
                current_mlir_block.0,
                block_references,
                gep_operation_value,
                &intrinsic_call.result_receiver,
                variable_store,
                location,
            );
        }
        ResultfullIntrinsic::Read => todo!(),
    }

    Ok(())
}

pub fn generate_resultless_intrinsic_call<'c, 'a>(
    intrinsic_call: &ResultlessIntrinsicCall,
    code_gen_context: &'c CodeGen,
    current_mlir_block: MlirBlockId,
    block_references: &HashMap<usize, BlockRef<'c, 'a>>,
    variable_store: &HashMap<Ssaid, Value<'c, 'a>>,
) -> Result<()> {
    let arguments = code_gen_context.load_variables(
        &intrinsic_call.arguments,
        block_references,
        variable_store,
        current_mlir_block,
    )?;

    match intrinsic_call.intrinsic {
        ResultlessIntrinsic::Write => {
            // Note: this intrinsic writes a number of bytes
            // from the first argument to the location pointed to by the second.

            // Consider this a pointer starting at the first element of
            // Byte array. Bytes will be copied from this array to the
            // destination.
            // TODO: This intrinsic should accept a Byte array instead of a pointer as the first
            // argument, once the core types are more well established.
            let value_to_write_from = arguments[0];
            let destination_pointer = arguments[1];
            // The number of bytes to write.
            let length_to_write = arguments[2];

            let is_volatile = code_gen_context.integer_attribute(1, 0);

            let memcpy_operation: Operation = ods::llvm::intr_memcpy(
                code_gen_context.context,
                destination_pointer,
                value_to_write_from,
                length_to_write,
                is_volatile,
                code_gen_context.unknown_location(),
            )
            .into();

            code_gen_context.append_operation(
                current_mlir_block.0,
                block_references,
                memcpy_operation,
            );
        }
    };

    Ok(())
}
