use std::marker::PhantomData;

use crate::ast::nodes::{AccessModes, FunctionKeyword};
use crate::identifiers::ID;

#[derive(Debug, Clone, Copy, Default, Ord, PartialEq, PartialOrd, Eq)]
pub enum Type {
    Struct(StructTypeID),
    Function(FunctionTypeID),
    Named(NamedTypeID),
    String,
    Unknown,
    Integer(SignedIntegerType),
    UnsignedInteger(UnsignedIntegerType),
    Float(FloatBitWidth),
    Boolean,
    Pointer,
    #[default]
    Unit,
    Array(ArrayTypeID),
}

#[derive(Clone, Debug, Copy, PartialEq, Ord, Eq, PartialOrd)]
pub enum FloatBitWidth {
    Bit64,
    Bit32,
}

#[derive(Clone, Debug, Copy, PartialEq, Ord, Eq, PartialOrd)]
pub enum IntegerBitWidth {
    Bit64,
    Bit32,
    Bit16,
    Bit8,
    PlatformSize,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct SignedIntegerType(pub IntegerBitWidth);

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct UnsignedIntegerType(pub IntegerBitWidth);

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum TypeID {
    Struct(StructTypeID),
    Function(FunctionTypeID),
    Named(NamedTypeID),
    Array(ArrayTypeID),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ArrayType {
    pub element_type: Type,
    pub length: usize,
}

#[derive(Debug, Clone, Default)]
pub struct StructType {
    pub field_ids: Vec<StructField>,
    pub field_types: Vec<TypeID>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub field_name: usize,
    pub field_type: TypeID,
}

#[derive(Debug, Clone)]
pub struct FunctionType {
    pub key_words: Vec<FunctionKeyword>,
    pub return_type: Type,
    pub parameter_types: Vec<Type>,
    pub parameter_access_modes: Vec<AccessModes>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug, Default)]
pub struct StructTypeID(ID);

impl From<ID> for StructTypeID {
    fn from(value: ID) -> Self {
        Self(value)
    }
}

impl From<StructTypeID> for TypeID {
    fn from(value: StructTypeID) -> Self {
        Self::Struct(value)
    }
}

impl From<StructTypeID> for usize {
    fn from(val: StructTypeID) -> Self {
        val.0
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub struct FunctionTypeID(ID);

impl From<ID> for FunctionTypeID {
    fn from(value: ID) -> Self {
        Self(value)
    }
}

impl From<FunctionTypeID> for TypeID {
    fn from(value: FunctionTypeID) -> Self {
        Self::Function(value)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub struct NamedTypeID(ID);

impl From<ID> for NamedTypeID {
    fn from(value: ID) -> Self {
        Self(value)
    }
}

impl From<NamedTypeID> for TypeID {
    fn from(value: NamedTypeID) -> Self {
        Self::Named(value)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug, Default)]
pub struct ArrayTypeID(ID);

impl From<ID> for ArrayTypeID {
    fn from(value: ID) -> Self {
        Self(value)
    }
}

impl From<ArrayTypeID> for usize {
    fn from(val: ArrayTypeID) -> Self {
        val.0
    }
}

impl From<ArrayTypeID> for TypeID {
    fn from(value: ArrayTypeID) -> Self {
        Self::Array(value)
    }
}

#[derive(Clone, Debug, Default)]
pub struct FlatEntityStore<T: PartialEq, K: From<usize>> {
    entities: Vec<T>,
    // This is only to force a specific type is used as the IDs in a store. It stores no data at runtime.
    id_type_marker: PhantomData<K>,
}

impl<T: PartialEq, K: From<usize> + Into<usize>> FlatEntityStore<T, K> {
    pub fn insert(&mut self, entity: T) -> K {
        let next_id = self.entities.len();
        self.entities.push(entity);
        K::from(next_id)
    }

    pub fn insert_if_not_present(&mut self, entity: T) -> K {
        if let Some(position) = self.entities.iter().position(|item| item == &entity) {
            return K::from(position);
        }

        self.insert(entity)
    }

    pub fn contains(&self, entity: &T) -> bool {
        self.entities.contains(entity)
    }

    pub fn get(&self, id: K) -> Option<&T> {
        self.entities.get(id.into())
    }

    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            id_type_marker: PhantomData {},
        }
    }
}
