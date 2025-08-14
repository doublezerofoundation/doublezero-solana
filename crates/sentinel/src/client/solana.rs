use crate::{Error, Result};

use base64::{engine::general_purpose::STANDARD as BASE64_STD, Engine};
use bincode;
use borsh::de::BorshDeserialize;
use doublezero_passport::{
    id as passport_id,
    instruction::{
        account::{DenyAccessAccounts, GrantAccessAccounts},
        AccessMode, PassportInstructionData,
    },
    state::AccessRequest,
};
use doublezero_program_tools::{instruction::try_build_instruction, PrecomputedDiscriminator};
use futures::{future::BoxFuture, stream::BoxStream, StreamExt, TryStreamExt};
use solana_account_decoder_client_types::UiAccountEncoding;
use solana_client::{
    nonblocking::{pubsub_client::PubsubClient, rpc_client::RpcClient},
    rpc_config::{
        RpcAccountInfoConfig, RpcProgramAccountsConfig, RpcTransactionLogsConfig,
        RpcTransactionLogsFilter,
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

    pub async fn get_access_mode_from_signature(&self, signature: Signature) -> Result<AccessMode> {
        let txn = self
            .client
            .get_transaction(&signature, UiTransactionEncoding::Binary)
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
                .ok_or(Error::InstructionNotFound(signature))?;

            if let PassportInstructionData::RequestAccess(access_mode) =
                PassportInstructionData::try_from_slice(&ix_data)?
            {
                Ok(access_mode)
            } else {
                Err(Error::InstructionInvalid(signature))
            }
        } else {
            Err(Error::TransactionEncoding(signature))
        }
    }

    pub async fn gets_access_modes(&self) -> Result<Vec<AccessMode>> {
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
            .then(|(pubkey, _acct)| async move {
                let signatures = self.client.get_signatures_for_address(&pubkey).await?;

                let creation_signature: Signature = signatures
                    .first()
                    .ok_or(Error::SignatureNotFound(pubkey))
                    .and_then(|sig| sig.signature.parse().map_err(Error::from))?;

                self.get_access_mode_from_signature(creation_signature)
                    .await
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
