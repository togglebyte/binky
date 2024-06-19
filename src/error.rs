//! Errors

pub(crate) type Result<T> = std::result::Result<T, Error>;

/// Binky errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Flume error
    #[error("channel closed")]
    Flume,

    /// Invalid value type when trying to downcast `AnyValue` or deserialize an incorrect value
    #[error("invalid value type")]
    InvalidValueType,

    /// Serde json error
    #[error("failed to deserialize message: {0}")]
    SerdeJson(#[from] serde_json::Error),

    /// Postcard error
    #[error("failed to deserialize message: {0}")]
    Postcard(#[from] postcard::Error),

    /// Io error
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Malformed message header
    #[error("malformed header")]
    MalformedHeader,

    /// Invalid serializer
    #[error("invalid serializer, the serializer was encoded incorrectly {0}")]
    InvalidSerializer(u8),

    /// Address not found
    #[error("address not found")]
    AddressNotFound,

    /// Failed to deliver a request
    #[error("failed to deliver the request to the recipient")]
    FailedDelivery,

    /// Tried to send / receive a remote message on a local agent
    #[error("tried to receive / send on a local-only agent")]
    RemoteActionOnLocal,

    /// Tried to perform an operation with a remote address
    #[error("this operation is only available on local addresses")]
    LocalOnly,

    /// Exhausted all retries
    #[error("no more retries")]
    NoRetry,
}

impl<T> From<flume::SendError<T>> for Error {
    fn from(_: flume::SendError<T>) -> Self {
        Self::Flume
    }
}

impl From<flume::RecvError> for Error {
    fn from(_: flume::RecvError) -> Self {
        Self::Flume
    }
}
