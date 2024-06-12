use serde::Serialize;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("channel closed")]
    Flume,

    #[error("invalid value type")]
    InvalidValueType,

    #[error("failed to deserialize message: {0}")]
    Serde(#[from] serde_json::Error),
    
    #[error("failed to deserialize message: {0}")]
    Postcard(#[from] postcard::Error),
    
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("malformed header")]
    MalformedHeader,

    #[error("invalid serializer, the serializer was encoded incorrectly {0}")]
    InvalidSerializer(u8),

    #[error("address not found")]
    AddressNotFound,

    #[error("failed to deliver the request to the recipient")]
    FailedDelivery,
}

impl<T> From<flume::SendError<T>> for Error {
    fn from(value: flume::SendError<T>) -> Self {
        Self::Flume
    }
}

impl From<flume::RecvError> for Error {
    fn from(value: flume::RecvError) -> Self {
        Self::Flume
    }
}
