use anyhow::{bail, Result};

use crate::analysis::type_evaluation::{IrProgramTypes, TypeName};
use crate::ast::nodes::Operator;
use crate::ir::Ssaid;
use crate::types::Type;

#[derive(Clone, Debug, PartialEq, Copy)]
pub struct BinaryOperation {
    pub left_hand_side: Ssaid,
    pub right_hand_side: Ssaid,
    pub reciever: Ssaid,
    pub operation_id: Operator,
}

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum Signage {
    Signed,
    Unsigned,
}

impl BinaryOperation {
    fn element_type_id(&self, progam_types: &IrProgramTypes) -> Result<Ssaid> {
        let left_hand_side_type_id = progam_types.variable_types[&self.left_hand_side];
        let right_hand_side_type_id = progam_types.variable_types[&self.right_hand_side];

        if left_hand_side_type_id != right_hand_side_type_id {
            bail!("expected both operators to have the same type, but got {:?} and {:?} for a {:?} operation",
                progam_types.comp_time_types[&left_hand_side_type_id], 
                progam_types.comp_time_types[&right_hand_side_type_id], 
                self.operation_id.to_debug_string()
                );
        }

        Ok(left_hand_side_type_id)
    }

    fn element_type(&self, progam_types: &IrProgramTypes) -> Result<Type> {
        let element_type_id = self.element_type_id(progam_types)?;
        Ok(progam_types.comp_time_types[&element_type_id])
    }

    pub fn signage(&self, progam_types: &IrProgramTypes) -> Result<Signage> {
        let element_type_id = self.element_type_id(progam_types)?;
        let element_type = progam_types.comp_time_types[&element_type_id];
        let signage = match element_type {
            Type::Integer(_) => Signage::Signed,
            Type::UnsignedInteger(_) => Signage::Unsigned,
            _ => bail!(
                "got unsupported type {:?} for binary operation {}",
                element_type,
                self.operation_id.to_debug_string()
            ),
        };

        Ok(signage)
    }

    pub fn result_type_id(&self, progam_types: &IrProgramTypes) -> Result<Ssaid> {
        match self.operation_id {
            Operator::Addition
            | Operator::Division
            | Operator::Multiplication
            | Operator::Subtraction => self.element_type_id(progam_types),

            Operator::Equality
            | Operator::LessThan
            | Operator::Inequality
            | Operator::GreaterThan
            | Operator::LessThanOrEqual
            | Operator::GreaterThanOrEqual => {
                Ok(*progam_types.type_name_ids.get(&TypeName::Boolean).unwrap())
            }
        }
    }

    pub fn is_floating_point_operation(&self, progam_types: &IrProgramTypes) -> Result<bool> {
        match self.element_type(progam_types)? {
            Type::Float(_) => Ok(true),
            _ => Ok(false),
        }
    }

    pub fn is_integer_operation(&self, progam_types: &IrProgramTypes) -> Result<bool> {
        match self.element_type(progam_types)? {
            Type::Integer(_) => Ok(true),
            _ => Ok(false),
        }
    }
}
