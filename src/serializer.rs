use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{Error, Result};

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u8)]
pub(crate) enum Serializer {
    // Plain text
    Json = 0,
    // Binary
    Postcard = 1,
}

impl Serializer {
    pub(crate) fn serialize(self, value: &impl Serialize) -> Result<Box<[u8]>> {
        let bytes = match self {
            Self::Json => serde_json::to_vec(&value)?,
            Self::Postcard => postcard::to_stdvec(&value)?,
        }.into_boxed_slice();
        Ok(bytes)
    }

    pub(crate) fn deserialize<T: DeserializeOwned>(self, bytes: impl AsRef<[u8]>) -> Result<T> {
        match self {
            Self::Json => Ok(serde_json::from_slice(bytes.as_ref())?),
            Self::Postcard => Ok(postcard::from_bytes(bytes.as_ref())?),
        }
    }
}

impl TryFrom<u8> for Serializer {
    type Error = Error;

    fn try_from(value: u8) -> std::prelude::v1::Result<Self, Self::Error> {
        let ser = match value {
            0 => Self::Json,
            1 => Self::Postcard,
            byte => return Err(Error::InvalidSerializer(byte))
        };
        Ok(ser)
    }
}
