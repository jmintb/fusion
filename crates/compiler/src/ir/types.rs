use super::Ssaid;

#[derive(Clone, Debug, PartialEq)]
pub struct TypeDeclaration {
    pub receiver: Ssaid,
    pub type_name_id: usize,
}
