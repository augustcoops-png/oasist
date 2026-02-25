//! Defines the [`OracleError`] type for the Oracle Adapter program.

use {solana_sdk::instruction::InstructionError, thiserror::Error};

/// Domain-specific errors produced by the Oracle Adapter program.
///
/// Each variant maps to the appropriate [`InstructionError`] so that the
/// on-chain error codes are unchanged for existing clients.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OracleError {
    /// The price feed account is not owned by this program.
    #[error("Price feed account is not owned by the Oracle Adapter program")]
    NotFeedOwner,

    /// The signer is not the authority recorded in the price feed.
    #[error("Signer is not the feed authority")]
    NotFeedAuthority,

    /// The price feed account data could not be serialized or deserialized.
    #[error("Failed to serialize or deserialize the price feed account data")]
    InvalidFeedData,

    /// The price feed account data buffer is too small to hold a serialized feed.
    #[error("Price feed account data is too small")]
    AccountDataTooSmall,
}

impl From<OracleError> for InstructionError {
    fn from(e: OracleError) -> Self {
        match e {
            OracleError::NotFeedOwner => InstructionError::InvalidAccountOwner,
            OracleError::NotFeedAuthority => InstructionError::MissingRequiredSignature,
            OracleError::InvalidFeedData => InstructionError::InvalidAccountData,
            OracleError::AccountDataTooSmall => InstructionError::AccountDataTooSmall,
        }
    }
}
