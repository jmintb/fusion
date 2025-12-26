use std::fmt::Display;

use anyhow::{bail, Result};

use super::Ssaid;

#[derive(Clone, Debug, PartialEq)]
pub struct ResultlessIntrinsicCall {
    pub intrinsic: ResultlessIntrinsic,
    pub arguments: Vec<Ssaid>,
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Copy)]
pub enum ResultlessIntrinsic {
    Write,
}

impl TryFrom<&str> for ResultlessIntrinsic {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value == "write_bytes" {
            Ok(Self::Write)
        } else {
            bail!("recieved invalid resutless intrinsic: {}", value)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Copy)]
pub enum ResultfullIntrinsic {
    Read,
    Offset,
}

impl Display for ResultlessIntrinsic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Write => f.write_str("memory_write"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultfullIntrinsicCall {
    pub intrinsic: ResultfullIntrinsic,
    pub arguments: Vec<Ssaid>,
    pub result_receiver: Ssaid,
    pub return_type_name_id: usize, // TODO: figure out how to make the type name thing less confusing.
}

impl Display for ResultfullIntrinsic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => f.write_str("memory_read"),
            Self::Offset => f.write_str("memory_offset"),
        }
    }
}

impl TryFrom<&str> for ResultfullIntrinsic {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "pointer_from_offset" => Ok(Self::Offset),
            _ => bail!("recieved invalid resutfull intrinsic: {}", value),
        }
    }
}
