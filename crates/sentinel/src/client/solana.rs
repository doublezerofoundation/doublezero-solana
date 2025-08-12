use crate::{Error, Result};

use base64::{engine::general_purpose::STANDARD as BASE64_STD, Engine};
use bincode;
use borsh::de::BorshDeserialize;
use doublezero_passport::{
    id as passport_id,
    instruction::{AccessMode, PassportInstructionData},
    state::AccessRequest,
};
use doublezero_program_tools::{
    zero_copy::checked_from_bytes_with_discriminator, PrecomputedDiscriminator,
};
use futures::{StreamExt, TryStreamExt};
use solana_account_decoder_client_types::UiAccountEncoding;
use solana_client::{
    nonblocking::{pubsub_client::PubsubClient, rpc_client::RpcClient},
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, RpcFilterType},
};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    transaction::Transaction,
};
use solana_transaction_status_client_types::{
    EncodedTransaction, TransactionBinaryEncoding, UiTransactionEncoding,
};
use url::Url;

pub struct SolRpcClient {
    client: RpcClient,
    payer: Keypair,
}

impl SolRpcClient {
    pub fn new(rpc_url: Url, payer: Keypair) -> Self {
        Self {
            client: RpcClient::new_with_commitment(rpc_url.into(), CommitmentConfig::confirmed()),
            payer,
        }
    }

    /// Get all AccessRequest accounts with their creation instruction data
    pub async fn get_access_requests_with_instructions(
        &self,
    ) -> Result<Vec<(Pubkey, AccessRequest, AccessMode)>> {
        let config = RpcProgramAccountsConfig {
            filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
                0,
                AccessRequest::discriminator_slice().to_vec(),
            ))]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                ..Default::default()
            },
            ..Default::default()
        };

        let accounts = self
            .client
            .get_program_accounts_with_config(&passport_id(), config)
            .await?;

        let access_requests_with_instructions = futures::stream::iter(accounts)
            .filter_map(|(pubkey, account)| async move {
                checked_from_bytes_with_discriminator::<AccessRequest>(&account.data)
                    .map(|(access_request, _)| (pubkey, *access_request))
            })
            .then(|(pubkey, access_request)| async move {
                let signatures = self.client.get_signatures_for_address(&pubkey).await?;

                let creation_signature: Signature = signatures
                    .first()
                    .ok_or(Error::SignatureNotFound(pubkey))
                    .and_then(|sig| sig.signature.parse().map_err(Error::from))?;

                let txn = self
                    .client
                    .get_transaction(&creation_signature, UiTransactionEncoding::Binary)
                    .await?;

                if let EncodedTransaction::Binary(data, TransactionBinaryEncoding::Base64) =
                    txn.transaction.transaction
                {
                    let data: &[u8] = &BASE64_STD.decode(data)?;
                    let tx: Transaction = bincode::deserialize(data)?;

                    let ix_data = tx
                        .message
                        .instructions
                        .iter()
                        .find(|ix| ix.program_id(&tx.message.account_keys) == &passport_id())
                        .map(|ix| ix.data.clone())
                        .ok_or(Error::InstructionNotFound(creation_signature))?;

                    if let PassportInstructionData::RequestAccess(access_mode) =
                        PassportInstructionData::try_from_slice(&ix_data)?
                    {
                        Ok((pubkey, access_request, access_mode))
                    } else {
                        Err(Error::InstructionInvalid(creation_signature))
                    }
                } else {
                    Err(Error::TransactionEncoding(creation_signature))
                }
            })
            .try_collect::<Vec<_>>()
            .await?;

        Ok(access_requests_with_instructions)
    }
}

pub struct SolPubsubClient {
    client: PubsubClient,
}

impl SolPubsubClient {
    pub async fn new(ws_url: Url) -> Result<Self> {
        let client = PubsubClient::new(ws_url.as_ref()).await?;

        Ok(Self { client })
    }
}
