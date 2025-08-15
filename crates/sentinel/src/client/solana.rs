use crate::{AccessIds, Error, Result};

use base64::{engine::general_purpose::STANDARD as BASE64_STD, Engine};
use bincode;
use borsh::de::BorshDeserialize;
use doublezero_passport::{
    id as passport_id,
    instruction::{
        account::{DenyAccessAccounts, GrantAccessAccounts},
        PassportInstructionData,
    },
    state::AccessRequest,
};
use doublezero_program_tools::{instruction::try_build_instruction, PrecomputedDiscriminator};
use futures::{future::BoxFuture, stream::BoxStream, StreamExt, TryStreamExt};
use solana_account_decoder_client_types::UiAccountEncoding;
use solana_client::{
    nonblocking::{pubsub_client::PubsubClient, rpc_client::RpcClient},
    rpc_config::{
        RpcAccountInfoConfig, RpcBlockProductionConfig, RpcBlockProductionConfigRange,
        RpcProgramAccountsConfig, RpcTransactionLogsConfig, RpcTransactionLogsFilter,
    },
    rpc_filter::{Memcmp, RpcFilterType},
    rpc_response::{Response, RpcLogsResponse},
};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    message::{v0::Message, VersionedMessage},
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::{Transaction, VersionedTransaction},
};
use solana_transaction_status_client_types::{
    EncodedTransaction, TransactionBinaryEncoding, UiTransactionEncoding,
};
use std::sync::Arc;
use url::Url;

pub struct SolRpcClient {
    client: RpcClient,
    payer: Arc<Keypair>,
}

impl SolRpcClient {
    pub fn new(rpc_url: Url, payer: Arc<Keypair>) -> Self {
        Self {
            client: RpcClient::new_with_commitment(rpc_url.into(), CommitmentConfig::confirmed()),
            payer,
        }
    }

    pub async fn grant_access(
        &self,
        access_request_key: &Pubkey,
        rent_beneficiary_key: &Pubkey,
    ) -> Result<Signature> {
        let signer = &self.payer;
        let grant_ix = try_build_instruction(
            &passport_id(),
            GrantAccessAccounts::new(&signer.pubkey(), access_request_key, rent_beneficiary_key),
            &PassportInstructionData::GrantAccess,
        )?;

        let recent_blockhash = self.client.get_latest_blockhash().await?;

        let transaction = new_transaction(&[grant_ix], &[signer], recent_blockhash);

        Ok(self
            .client
            .send_and_confirm_transaction(&transaction)
            .await?)
    }

    pub async fn deny_access(&self, access_request_key: &Pubkey) -> Result<Signature> {
        let signer = &self.payer;
        let deny_ix = try_build_instruction(
            &passport_id(),
            DenyAccessAccounts::new(&signer.pubkey(), access_request_key),
            &PassportInstructionData::DenyAccess,
        )?;

        let recent_blockhash = self.client.get_latest_blockhash().await?;

        let transaction = new_transaction(&[deny_ix], &[signer], recent_blockhash);

        Ok(self
            .client
            .send_and_confirm_transaction(&transaction)
            .await?)
    }

    pub async fn get_access_request_from_signature(
        &self,
        signature: Signature,
    ) -> Result<AccessIds> {
        let txn = self
            .client
            .get_transaction(&signature, UiTransactionEncoding::Binary)
            .await?;

        if let EncodedTransaction::Binary(data, TransactionBinaryEncoding::Base64) =
            txn.transaction.transaction
        {
            let data: &[u8] = &BASE64_STD.decode(data)?;
            let tx: Transaction = bincode::deserialize(data)?;

            deserialize_access_request_ids(tx)
        } else {
            Err(Error::TransactionEncoding(signature))
        }
    }

    pub async fn gets_access_request(&self) -> Result<Vec<AccessIds>> {
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

        let access_ids = futures::stream::iter(accounts)
            .then(|(pubkey, _acct)| async move {
                let signatures = self.client.get_signatures_for_address(&pubkey).await?;

                let creation_signature: Signature = signatures
                    .first()
                    .ok_or(Error::MissingTxnSignature)
                    .and_then(|sig| sig.signature.parse().map_err(Error::from))?;

                self.get_access_request_from_signature(creation_signature)
                    .await
            })
            .try_collect::<Vec<_>>()
            .await?;

        Ok(access_ids)
    }

    pub async fn check_leader_schedule(&self, validator_id: &Pubkey) -> Result<bool> {
        let latest_slot = self.client.get_slot().await?;
        let oldest_slot = latest_slot.saturating_sub(DAY_OF_SLOTS * 7);

        for (start, end) in ReverseSlotRange::new(oldest_slot, latest_slot) {
            let config = RpcBlockProductionConfig {
                range: Some(RpcBlockProductionConfigRange {
                    first_slot: start,
                    last_slot: Some(end),
                }),
                identity: Some(validator_id.to_string()),
                ..Default::default()
            };

            if !self
                .client
                .get_block_production_with_config(config)
                .await?
                .value
                .by_identity
                .is_empty()
            {
                return Ok(true);
            }
        }

        Ok(false)
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

    pub async fn subscribe_to_access_requests(
        &self,
    ) -> Result<(
        BoxStream<'_, Response<RpcLogsResponse>>,
        Box<dyn FnOnce() -> BoxFuture<'static, ()> + Send>,
    )> {
        let config = RpcTransactionLogsConfig {
            commitment: Some(CommitmentConfig::confirmed()),
        };

        let filter = RpcTransactionLogsFilter::Mentions(vec![passport_id().to_string()]);

        Ok(self.client.logs_subscribe(filter, config).await?)
    }
}

fn new_transaction(
    instructions: &[Instruction],
    signers: &[&Keypair],
    recent_blockhash: Hash,
) -> VersionedTransaction {
    let message =
        Message::try_compile(&signers[0].pubkey(), instructions, &[], recent_blockhash).unwrap();

    VersionedTransaction::try_new(VersionedMessage::V0(message), signers).unwrap()
}

fn deserialize_access_request_ids(txn: Transaction) -> Result<AccessIds> {
    let signature = txn.signatures.first().ok_or(Error::MissingTxnSignature)?;
    let compiled_ix = txn
        .message
        .instructions
        .iter()
        .find(|ix| ix.program_id(&txn.message.account_keys) == &passport_id())
        .ok_or(Error::InstructionNotFound(*signature))?;
    let accounts = compiled_ix
        .accounts
        .iter()
        .map(|&idx| txn.message.account_keys.get(idx as usize).copied())
        .collect::<Option<Vec<_>>>()
        .ok_or(Error::MissingAccountKeys(*signature))?;
    let Ok(PassportInstructionData::RequestAccess(mode)) =
        PassportInstructionData::try_from_slice(&compiled_ix.data)
    else {
        return Err(Error::InstructionInvalid(*signature));
    };
    match (accounts.get(2), accounts.get(1)) {
        (Some(request_pda), Some(payer)) => Ok(AccessIds {
            request_pda: *request_pda,
            rent_beneficiary_key: *payer,
            mode,
        }),
        _ => Err(Error::InstructionInvalid(*signature)),
    }
}

// Chunk the request by roughly 1 days worth of slots
// Assumes average slot time of 0.4 seconds
const DAY_OF_SLOTS: u64 = 216_000;

struct ReverseSlotRange {
    current_start: u64,
    last_slot: u64,
    chunk_size: u64,
}

impl ReverseSlotRange {
    fn new(starting_slot: u64, oldest_slot: u64) -> Self {
        Self {
            current_start: starting_slot,
            last_slot: oldest_slot,
            chunk_size: DAY_OF_SLOTS,
        }
    }
}

impl Iterator for ReverseSlotRange {
    type Item = (u64, u64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_start <= self.last_slot {
            None
        } else {
            let end = self.current_start;
            let start = std::cmp::max(
                self.current_start.saturating_sub(self.chunk_size),
                self.last_slot,
            );
            self.current_start = start - 1;
            Some((start, end))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_reverse_iter() {
        let start_slot = 2_000_000;
        let oldest_slot = 1_000_000;
        let slot_range = ReverseSlotRange::new(start_slot, oldest_slot);
        let results = slot_range.into_iter().collect::<Vec<_>>();
        assert_eq!(results.len(), 5);
        assert_eq!(
            results.first().map(|(start, end)| end - start).unwrap(),
            DAY_OF_SLOTS
        );
        assert_eq!(
            results.last().map(|(start, end)| end - start).unwrap(),
            (start_slot - oldest_slot) - (4 * DAY_OF_SLOTS + 4),
        );
        assert_eq!(
            results.first().map(|(_, start)| *start).unwrap(),
            start_slot
        );
        assert_eq!(results.last().map(|(end, _)| *end).unwrap(), oldest_slot);
    }
}
