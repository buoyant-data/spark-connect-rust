//! The error module has custom errors for the Spark Connect library

use thiserror::Error;

/// Spark Connect errors
#[derive(Error, Debug)]
pub enum Error {
    #[error("Arrow error: {0}")]
    ArrowError(#[from] arrow::error::ArrowError),
    #[error("HTTP error: {0}")]
    HttpError(#[from] http::uri::InvalidUri),
    /// Wrapped error from the tonic library
    #[error("Tonic transport error: {0}")]
    TransportError(#[from] tonic::transport::Error),
    #[error("Tonic gRPC error: {0}")]
    RpcError(#[from] tonic::Status),
    /// Unknown error
    #[error("Unknown Spark Connect error")]
    Unknown,
    /// Generic wrapped error
    #[error("Generic error: {0}")]
    Generic(Box<dyn std::error::Error>),
}
