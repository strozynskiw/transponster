use thiserror::Error;

use super::models::{ClientId, OperationType, TransactionId};
#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Parsing error")]
    Parsing(#[from] csv::Error),
    #[error("IO read error")]
    Reading(#[from] std::io::Error),

    // This one is not returned, just printed to stderr
    // so we don't break the transaction processing
    #[error(transparent)]
    Processing(#[from] ProcessingError),
}

// This error is only for internal error reporting to stderr
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ProcessingError {
    #[error("Negative amount")]
    NegativeAmount,

    #[error("Value overflow detected for transaction id `{0}`")]
    Overflow(TransactionId),

    #[error("Value underflow detected for transaction id `{0}`")]
    Underflow(TransactionId),

    #[error("Duplicated transaction `{0}` for account `{1}`")]
    DuplicatedTransaction(TransactionId, ClientId),

    #[error("Duplicated dispute for transaction `{0}`, by transaction `{0}` for account `{1}`")]
    DuplicatedDispute(TransactionId, TransactionId, ClientId),

    #[error("Account `{0}` is locked")]
    AccountLocked(ClientId),

    #[error("No amount in transaction `{0}`")]
    MissingAmount(TransactionId),

    #[error("insufficient founds for transaction `{0}`; account: `{1}`")]
    InsufficientFounds(TransactionId, ClientId),

    #[error("Referenced transaction `{0}` doesn't exist")]
    MissingTransaction(TransactionId),

    #[error("Invalid operation `{0}` under dispute for transaction `{1}`")]
    InvalidOperationUnderDispute(OperationType, TransactionId),

    #[error("Resolve called on not disputed operation `{0}` for transaction `{1}`")]
    IncorrectResolve(OperationType, TransactionId),

    #[error("Chargeback called on not disputed operation `{0}` for transaction `{1}`")]
    IncorrectChargeback(OperationType, TransactionId),
}
