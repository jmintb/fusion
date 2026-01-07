use std::collections::btree_map::BTreeMap;
use std::collections::HashSet;

use anyhow::{bail, Result};
use tracing::debug;

use super::ir_transformer::{IrInterpreter, TransformContext};
use crate::ast::identifiers::FunctionDeclarationID;
use crate::control_flow_graph::ControlFlowGraph;
use crate::ir::intrinsics::ResultfullIntrinsicCall;
use crate::ir::{AnnotatedAssignment, BlockId, Instruction, IrProgram, Ssaid};
use crate::types::{ArrayTypeID, FlatEntityStore, StructTypeID, Type};

pub type TypeName = crate::ast::nodes::Type;

#[derive(Clone, Default, Debug)]
pub struct IrProgramTypes {
    pub array_types: FlatEntityStore<ArrayType, ArrayTypeID>,
    pub variable_types: BTreeMap<Ssaid, Ssaid>,
    pub struct_types: FlatEntityStore<StructType, StructTypeID>,
    projected_types: HashSet<Ssaid>,
    pub comp_time_types: BTreeMap<Ssaid, Type>,
    pub type_name_ids: BTreeMap<TypeName, Ssaid>, // TODO: replace Type with a Type tag, reference or similar as this is not an actual type.
                                                  // it is the representation of the type annotation which hopefully has a matching type in scope.
                                                  // Also turn this into a stack to support scopes
}

impl IrProgramTypes {
    pub fn lookup_variable_type(&self, variable_id: Ssaid) -> Result<Type> {
        let Some(type_ssaid) = self.variable_types.get(&variable_id) else {
            bail!("failed to find type id");
        };

        let Some(&r#type) = self.comp_time_types.get(type_ssaid) else {
            bail!("failed to find comp time type");
        };

        Ok(r#type)
    }

    pub fn lookup_type_name_type(&self, type_name: &TypeName) -> Result<Type> {
        if let TypeName::Unit = type_name {
            return Ok(Type::Unit);
        }

        let Some(type_ssaid) = self.type_name_ids.get(type_name) else {
            panic!("{:?}", type_name);
        };
        Ok(self.comp_time_types[type_ssaid])
    }

    pub fn is_projection(&self, ssaid: &Ssaid) -> bool {
        self.projected_types.contains(ssaid)
    }

    pub fn set_variable_as_projected(&mut self, ssaid: &Ssaid) {
        self.projected_types.insert(*ssaid);
    }

    pub fn calculate_struct_field_position(
        &self,
        struct_ssaid: Ssaid,
        field_id_index: usize,
        ir_program: &IrProgram,
    ) -> Result<usize> {
        let Ok(Type::Struct(struct_type_id)) = self.lookup_variable_type(struct_ssaid) else {
            bail!("failed to find struct type id when calcuating struct field positiojn");
        };

        let Some(actual_type) = self.struct_types.get(struct_type_id) else {
            bail!("faile to find actual struct type when calcuating struct field position");
        };

        let struct_field_id_to_read = ir_program.struct_field_identifier[field_id_index].clone();
        let field_index = actual_type
            .field_ids
            .iter()
            .position(|struct_field_id_index| {
                let struct_field_id = ir_program
                    .struct_field_identifier
                    .get(*struct_field_id_index)
                    .unwrap();

                *struct_field_id == struct_field_id_to_read
            })
            .unwrap();

        Ok(field_index)
    }

    pub fn calcuate_struct_field_type(
        &self,
        struct_ssaid: Ssaid,
        field_name_index: usize,
        ir_program: &IrProgram,
    ) -> Result<Ssaid> {
        let field_index =
            self.calculate_struct_field_position(struct_ssaid, field_name_index, ir_program)?;

        let Ok(Type::Struct(struct_type_id)) = self.lookup_variable_type(struct_ssaid) else {
            bail!("failed to find struct type id when calcuating struct field positiojn");
        };

        let Some(actual_type) = self.struct_types.get(struct_type_id) else {
            bail!("faile to find actual struct type when calcuating struct field position");
        };

        // TODO: field here is not the field index but a reference to the field identifer.
        let Some(field_type_id) = actual_type.field_types.get(field_index) else {
            bail!("failed to find field type id for struct field read");
        };

        Ok(*field_type_id)
    }
}

// TODO: make this fault tolerant

#[derive(Debug, Clone, Default)]
pub struct StructType {
    pub field_ids: Vec<usize>,
    pub field_types: Vec<Ssaid>,
}

#[derive(Debug, Clone, Default)]
pub struct ArrayType {
    pub length: usize,
    pub element_type: Ssaid,
}

pub fn evaluate_types(
    ir_program: &IrProgram,
) -> Result<BTreeMap<FunctionDeclarationID, IrProgramTypes>> {
    let mut types_in_functions: BTreeMap<FunctionDeclarationID, IrProgramTypes> = BTreeMap::new();
    let top_level_cfg = ControlFlowGraph::new(ir_program.top_level_block);

    let top_level_block_interpreter: IrInterpreter<'_, IrProgramTypes> =
        IrInterpreter::new(&top_level_cfg, ir_program);

    let top_level_types = top_level_block_interpreter.transform(&mut check_types)?;
    debug!("found top level types: {:#?}", top_level_types);

    for function_id in ir_program.control_flow_graphs.keys() {
        let ir_interpreter: IrInterpreter<'_, IrProgramTypes> = IrInterpreter::new_with_ctx(
            ir_program.control_flow_graphs.get(function_id).unwrap(),
            ir_program.clone(),
            top_level_types.clone(),
        );

        let types = ir_interpreter.transform(&mut check_types)?;

        types_in_functions.insert(*function_id, types);
    }

    Ok(types_in_functions)
}

fn check_types(
    instruction_counter: usize,
    ctx: &mut TransformContext,
    block_id: &BlockId,
    bc_ctx: &mut IrProgramTypes,
) -> Result<usize> {
    let block = ctx.scope.blocks.get_mut(block_id).unwrap();

    let Some(instruction) = block.instructions.get(instruction_counter) else {
        return Ok(0);
    };

    debug!("evaluating type for instruction: {:?}", instruction);

    match instruction {
        Instruction::YieldingCall(_, _, receiver, return_type_name_id) => {
            let type_name = ctx.ir_program.type_names.get(*return_type_name_id).unwrap();
            let Some(type_ssaid) = bc_ctx.type_name_ids.get(type_name) else {
                bail!("failed to find type ssaid for type name: {:#?}", type_name);
            };
            bc_ctx.variable_types.insert(*receiver, *type_ssaid);
        }
        Instruction::Call(_, _, receiver, return_type_name_id) => {
            let type_name = ctx.ir_program.type_names.get(*return_type_name_id).unwrap();
            let Some(type_ssaid) = bc_ctx.type_name_ids.get(type_name) else {
                bail!("failed to find type ssaid for type name: {:#?}", type_name);
            };
            bc_ctx.variable_types.insert(*receiver, *type_ssaid);
        }
        Instruction::CallIntrinsic(ResultfullIntrinsicCall {
            result_receiver,
            return_type_name_id,
            ..
        }) => {
            let type_name = ctx.ir_program.type_names.get(*return_type_name_id).unwrap();
            let Some(type_ssaid) = bc_ctx.type_name_ids.get(type_name) else {
                bail!("failed to find type ssaid for type name: {:#?}", type_name);
            };
            bc_ctx.variable_types.insert(*result_receiver, *type_ssaid);
        }
        Instruction::LessThan(lhs, rhs, receiver) => {
            let lhs_type_ssaid = bc_ctx.variable_types.get(lhs).unwrap();
            let rhs_type_ssaid = bc_ctx.variable_types.get(rhs).unwrap();
            assert!(lhs_type_ssaid == rhs_type_ssaid);

            let type_ssaid = bc_ctx.type_name_ids.get(&TypeName::Boolean).unwrap();
            bc_ctx.variable_types.insert(*receiver, *type_ssaid);
        }
        Instruction::GreaterThan(lhs, rhs, receiver) => {
            let lhs_type_ssaid = bc_ctx.variable_types.get(lhs).unwrap();
            let rhs_type_ssaid = bc_ctx.variable_types.get(rhs).unwrap();
            assert!(lhs_type_ssaid == rhs_type_ssaid);

            let type_ssaid = bc_ctx.type_name_ids.get(&TypeName::Boolean).unwrap();
            bc_ctx.variable_types.insert(*receiver, *type_ssaid);
        }
        Instruction::Addition(lhs, rhs, receiver) => {
            let lhs_type_ssaid = bc_ctx.variable_types.get(lhs).unwrap();
            let rhs_type_ssaid = bc_ctx.variable_types.get(rhs).unwrap();
            assert!(lhs_type_ssaid == rhs_type_ssaid);

            bc_ctx.variable_types.insert(*receiver, *lhs_type_ssaid);
        }
        Instruction::AssignFnArg(receiver, _, type_name_id) => {
            let type_name = ctx.ir_program.type_names.get(*type_name_id).unwrap();
            let Some(type_ssaid) = bc_ctx.type_name_ids.get(type_name) else {
                bail!("failed to find type ssaid for type name: {:#?}", type_name);
            };
            bc_ctx.variable_types.insert(*receiver, *type_ssaid);
        }
        Instruction::DeclareBooleanType {
            receiver,
            type_name_id,
        } => {
            bc_ctx.comp_time_types.insert(*receiver, Type::Boolean);
            let type_name = ctx.ir_program.type_names.get(*type_name_id).unwrap();
            bc_ctx.type_name_ids.insert(type_name.clone(), *receiver);
        }
        Instruction::DeclarePointerType {
            receiver,
            type_name_id,
        } => {
            bc_ctx.comp_time_types.insert(*receiver, Type::Pointer);
            let type_name = ctx.ir_program.type_names.get(*type_name_id).unwrap();
            bc_ctx.type_name_ids.insert(type_name.clone(), *receiver);
        }
        Instruction::DeclareUnsignedIntegerType {
            receiver,
            type_name_id,
            bit_width,
        } => {
            bc_ctx.comp_time_types.insert(
                *receiver,
                Type::UnsignedInteger(crate::types::UnsignedIntegerType(*bit_width)),
            );
            let type_name = ctx.ir_program.type_names.get(*type_name_id).unwrap();
            bc_ctx.type_name_ids.insert(type_name.clone(), *receiver);
        }
        Instruction::DeclareIntegerType {
            receiver,
            type_name_id,
            bit_width,
        } => {
            bc_ctx.comp_time_types.insert(
                *receiver,
                Type::Integer(crate::types::SignedIntegerType(*bit_width)),
            );
            let type_name = ctx.ir_program.type_names.get(*type_name_id).unwrap();
            bc_ctx.type_name_ids.insert(type_name.clone(), *receiver);
        }
        Instruction::DeclareStringType {
            receiver,
            type_name_id,
        } => {
            bc_ctx.comp_time_types.insert(*receiver, Type::String);
            let type_name = ctx.ir_program.type_names.get(*type_name_id).unwrap();
            bc_ctx.type_name_ids.insert(type_name.clone(), *receiver);

            // TODO: this is a hack t support string literals, should they just be removed?
            bc_ctx
                .type_name_ids
                .insert(TypeName::StringLiteral, *receiver);
        }
        Instruction::AnonymousValue(ssaid) => {
            let value = ctx.ir_program.static_values.get(ssaid).unwrap();
            match *value {
                crate::ast::nodes::Value::String(_) => {
                    let type_ssaid = bc_ctx.type_name_ids.get(&TypeName::String).unwrap();
                    bc_ctx.variable_types.insert(*ssaid, *type_ssaid);
                }
                crate::ast::nodes::Value::Integer(_) => {
                    let type_ssaid = bc_ctx.type_name_ids.get(&TypeName::Integer32).unwrap();
                    bc_ctx.variable_types.insert(*ssaid, *type_ssaid);
                }
                _ => (),
            }
        }
        Instruction::StructDeclaration {
            receiver,
            field_name_ids,
            field_type_name_ids,
            type_name_id,
        } => {
            let field_type_names = field_type_name_ids
                .iter()
                .map(|name_id| ctx.ir_program.type_names.get(*name_id).unwrap().clone());

            let field_type_ids: Vec<Ssaid> = field_type_names
                .map(|type_identifier| {
                    bc_ctx
                        .type_name_ids
                        .get(&type_identifier)
                        .unwrap()
                        .to_owned()
                })
                .collect();

            let struct_type = StructType {
                field_ids: field_name_ids.clone(),
                field_types: field_type_ids,
            };

            let struct_type_id = bc_ctx.struct_types.insert(struct_type);
            bc_ctx
                .comp_time_types
                .insert(*receiver, Type::Struct(struct_type_id));

            let type_name = ctx.ir_program.type_names.get(*type_name_id).unwrap();

            bc_ctx.type_name_ids.insert(type_name.clone(), *receiver);
        }
        Instruction::StructInit {
            struct_identifier,
            receiver,
            field_values,
        } => {
            let struct_type_name = ctx.ir_program.type_names.get(*struct_identifier).unwrap();
            let type_ssaid = bc_ctx.type_name_ids.get(struct_type_name).unwrap();
            bc_ctx.variable_types.insert(*receiver, *type_ssaid);

            let Some(Type::Struct(struct_type_id)) = bc_ctx.comp_time_types.get(type_ssaid) else {
                bail!("failed to find struct type for {:?}", struct_type_name);
            };

            let struct_type = bc_ctx.struct_types.get(*struct_type_id).unwrap();
            for (expected_field_type_ssaid, field_value) in
                struct_type.field_types.iter().zip(field_values)
            {
                let field_value_type_ssaid = bc_ctx.variable_types.get(field_value).unwrap();

                if expected_field_type_ssaid != field_value_type_ssaid {
                    bail!(format!(
                        "struct {:?} field value has wrong type",
                        struct_type_name
                    ));
                }
            }
        }
        Instruction::ReadStructField {
            r#struct,
            field: field_id_index,
            receiver,
        } => {
            let field_type_id =
                bc_ctx.calcuate_struct_field_type(*r#struct, *field_id_index, &ctx.ir_program)?;

            bc_ctx.variable_types.insert(*receiver, field_type_id);
        }
        Instruction::InitArray(elements, receiver, type_ssaid) => {
            let first_element = elements[0];
            let element_type = bc_ctx.variable_types[&first_element];

            let array_type = ArrayType {
                element_type,
                length: elements.len(),
            };
            let type_id = bc_ctx.array_types.insert(array_type);
            bc_ctx
                .comp_time_types
                .insert(*type_ssaid, Type::Array(type_id));
            bc_ctx.variable_types.insert(*receiver, *type_ssaid);
            debug!("set type {:?} for variable {}", type_id, receiver.0);
        }

        Instruction::AnnotatedAssign(AnnotatedAssignment {
            reciever,
            value,
            type_name_id,
        }) => {
            let annotated_type_name = ctx.ir_program.type_names.get(*type_name_id).unwrap();
            let annotated_type_id = bc_ctx.type_name_ids[annotated_type_name];
            let value_type_id = bc_ctx.variable_types[value];

            // For built in Integer types the annotated type is applied to
            // the initial value. Otherwise the default i32 would be used
            // and the types would conflict. This matches Rust's behavior.
            if annotated_type_id != value_type_id
                && ctx.ir_program.static_values.contains_key(value)
            {
                let value_type = bc_ctx.comp_time_types[&value_type_id];
                match value_type {
                    Type::Integer(_) | Type::UnsignedInteger(_) => {
                        bc_ctx.variable_types.insert(*value, annotated_type_id);
                    }
                    _ => panic!("can't assign value to to type"),
                }
            } else {
                assert!(annotated_type_id == value_type_id);
            }

            bc_ctx.variable_types.insert(*reciever, annotated_type_id);
            if bc_ctx.is_projection(value) {
                bc_ctx.set_variable_as_projected(reciever);
            };
        }
        Instruction::Assign(result, value) => {
            if let Some(r#type) = bc_ctx.variable_types.get(value) {
                bc_ctx.variable_types.insert(*result, *r#type);
                if bc_ctx.is_projection(value) {
                    bc_ctx.set_variable_as_projected(result);
                };
            } else {
                panic!()
            }
        }

        Instruction::StructAssign {
            r#struct: struct_id,
            field_name_index,
            reciever,
        } => {
            if let Ok(_field_type_id) =
                bc_ctx.calcuate_struct_field_type(*struct_id, *field_name_index, &ctx.ir_program)
            {
                // TODO: type checking
                if bc_ctx.is_projection(reciever) {
                    panic!()
                };
            } else {
                panic!()
            }
        }

        Instruction::Project {
            projector,
            reciever,
        } => {
            if let Some(r#type) = bc_ctx.variable_types.get(projector) {
                bc_ctx.variable_types.insert(*reciever, *r#type);
                bc_ctx.set_variable_as_projected(reciever);
            } else {
                panic!()
            }
        }

        Instruction::ArrayLookup { array, result, .. } => {
            let Ok(Type::Array(array_variable_type_id)) = bc_ctx.lookup_variable_type(*array)
            else {
                bail!("expected to find array type for {}", array.0);
            };

            let array_type = bc_ctx
                .array_types
                .get(array_variable_type_id)
                .expect("type must exist at this point");
            bc_ctx
                .variable_types
                .insert(*result, array_type.element_type);
            debug!(
                "set type {:#?} for variable {}",
                array_type.element_type, result.0
            );
        }
        _ => (),
    }

    Ok(instruction_counter + 1)
}

#[cfg(test)]
mod test {

    use std::path::PathBuf;

    use anyhow::Result;
    use rstest::rstest;

    use crate::compiler::produce_ir;

    #[rstest]
    #[test_log::test]
    fn test_type_checking_incorrect_programs(
        #[files("./type_checking_test_programs/invalid_programs/test_*.ts")] path: PathBuf,
    ) -> Result<()> {
        let ir_program = produce_ir(path.to_str().unwrap())?;

        let analysis_result = super::evaluate_types(&ir_program);

        assert!(analysis_result.is_err());

        insta::assert_snapshot!(
            format!(
                "test_type_checking{}",
                path.file_name().unwrap().to_str().unwrap()
            ),
            format!("{:#?}", analysis_result)
        );
        Ok(())
    }
}
