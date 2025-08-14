use solana_client::{client_error::ClientError, nonblocking::pubsub_client::PubsubClientError};
use solana_sdk::signature::{ParseSignatureError, Signature};
use thiserror::Error;

pub type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),
    #[error("bincode deserialization error: {0}")]
    BincodeDeser(#[from] bincode::Error),
    #[error("borsh deserialization error: {0}")]
    BorshIo(#[from] borsh::io::Error),
    #[error("instruction not found in transaction: {0}")]
    InstructionNotFound(Signature),
    #[error("invalid instruction data: {0}")]
    InstructionInvalid(Signature),
    #[error("pubsub client error: {0}")]
    PubsubClient(#[from] PubsubClientError),
    #[error("request channel error: {0}")]
    ReqChannel(#[from] tokio::sync::mpsc::error::SendError<Signature>),
    #[error("rpc client error: {0}")]
    RpcClient(#[from] ClientError),
    #[error("signature not found for account: {0}")]
    SignatureNotFound(solana_sdk::pubkey::Pubkey),
    #[error("invalid transaction signature: {0}")]
    SignatureInvalid(#[from] ParseSignatureError),
    #[error("invalid transaction encoding: {0}")]
    TransactionEncoding(Signature),
}
