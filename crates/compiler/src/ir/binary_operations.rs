use crate::ast::nodes::Operator;
use crate::ir::Ssaid;

#[derive(Clone, Debug, PartialEq, Copy)]
pub struct BinaryOperation {
    pub left_hand_side: Ssaid,
    pub right_hand_side: Ssaid,
    pub reciever: Ssaid,
    pub operation_id: Operator,
}
