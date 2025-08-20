use anyhow::{Context, Result};
use doublezero_record::state::RecordData;
use doublezero_sdk::record::{self, client, state::read_record_data};
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig};
use solana_sdk::{commitment_config::CommitmentConfig, signer::keypair::Keypair, signer::Signer};
use std::fmt;

/// Result of a write operation
#[derive(Debug)]
pub enum WriteResult {
    Success(String),
    Failed(String, String), // (description, error)
}

/// Summary of all ledger writes
#[derive(Debug, Default)]
pub struct WriteSummary {
    pub results: Vec<WriteResult>,
}

impl WriteSummary {
    pub fn add_success(&mut self, description: String) {
        self.results.push(WriteResult::Success(description));
    }

    pub fn add_failure(&mut self, description: String, error: String) {
        self.results.push(WriteResult::Failed(description, error));
    }

    pub fn successful_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r, WriteResult::Success(_)))
            .count()
    }

    pub fn failed_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r, WriteResult::Failed(_, _)))
            .count()
    }

    pub fn total_count(&self) -> usize {
        self.results.len()
    }

    pub fn all_successful(&self) -> bool {
        self.failed_count() == 0
    }
}

impl fmt::Display for WriteSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=========================================")?;
        writeln!(f, "Ledger Write Summary")?;
        writeln!(f, "=========================================")?;
        writeln!(
            f,
            "Total: {}/{} successful",
            self.successful_count(),
            self.total_count()
        )?;

        if !self.all_successful() {
            writeln!(f, " Failed writes:")?;
            for result in &self.results {
                if let WriteResult::Failed(desc, error) = result {
                    writeln!(f, "  ❌ {desc}: {error}")?;
                }
            }
        }
        writeln!(f, " All writes:")?;
        for result in &self.results {
            match result {
                WriteResult::Success(desc) => writeln!(f, "  ✅ {desc}")?,
                WriteResult::Failed(desc, _) => writeln!(f, "  ❌ {desc}")?,
            }
        }

        writeln!(f, "=========================================")?;
        Ok(())
    }
}

pub async fn write_record_to_ledger<T: borsh::BorshSerialize>(
    rpc_client: &RpcClient,
    payer_signer: &Keypair,
    record_data: &T,
    commitment_config: CommitmentConfig,
    seeds: &[&[u8]],
) -> Result<()> {
    let recent_blockhash = rpc_client.get_latest_blockhash().await?;
    let payer_key = payer_signer.pubkey();

    let serialized = borsh::to_vec(record_data)?;
    client::try_create_record(
        rpc_client,
        recent_blockhash,
        payer_signer,
        seeds,
        serialized.len(),
    )
    .await?;

    for chunk in record::instruction::write_record_chunks(&payer_key, seeds, &serialized) {
        chunk
            .into_send_transaction_with_config(
                rpc_client,
                recent_blockhash,
                payer_signer,
                true,
                RpcSendTransactionConfig {
                    preflight_commitment: Some(commitment_config.commitment),
                    ..Default::default()
                },
            )
            .await?;
        // println!("Successfully wrote {} to {}", data_type, record_key);
    }

    Ok(())
}

pub async fn read_from_ledger(
    rpc_client: &RpcClient,
    payer_signer: &Keypair,
    seeds: &[&[u8]],
    commitment_config: CommitmentConfig,
) -> Result<(RecordData, Vec<u8>)> {
    let payer_key = payer_signer.pubkey();

    let record_key = record::pubkey::create_record_key(&payer_key, seeds);
    let get_account_response = rpc_client
        .get_account_with_commitment(&record_key, commitment_config)
        .await
        .with_context(|| format!("Failed to fetch account {record_key}"))?;

    let record_account_info = get_account_response
        .value
        .ok_or_else(|| anyhow::anyhow!("Record acconut not found at address {record_key}"))?;

    let (record_header, record_body) = read_record_data(&record_account_info.data)
        .with_context(|| format!("Failed to parse record data from account {record_key}"))?;

    Ok((*record_header, record_body.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fee_payment_calculator::FeePaymentCalculator;
    use crate::rewards::{EpochRewards, Reward};

    use solana_client::{
        nonblocking::rpc_client::RpcClient,
        rpc_config::{RpcBlockConfig, RpcGetVoteAccountsConfig},
    };
    use solana_sdk::{commitment_config::CommitmentConfig, signer::Signer};

    use solana_transaction_status_client_types::{TransactionDetails, UiTransactionEncoding};
    use std::{str::FromStr, time::Duration};

    #[ignore] // this test will fail until we hook up the validator script
    #[tokio::test]
    async fn test_write_to_read_from_ledger() -> anyhow::Result<()> {
        let validator_id = "devgM7SXHvoHH6jPXRsjn97gygPUo58XEnc9bqY1jpj";
        let commitment_config = CommitmentConfig::processed();
        let rpc_client =
            RpcClient::new_with_commitment("http://localhost:8899".to_string(), commitment_config);
        let vote_account_config = RpcGetVoteAccountsConfig {
            vote_pubkey: Some(validator_id.to_string()),
            commitment: CommitmentConfig::finalized().into(),
            keep_unstaked_delinquents: None,
            delinquent_slot_distance: None,
        };

        let rpc_block_config = RpcBlockConfig {
            encoding: Some(UiTransactionEncoding::Base58),
            transaction_details: Some(TransactionDetails::None),
            rewards: Some(true),
            commitment: None,
            max_supported_transaction_version: Some(0),
        };
        let fpc = FeePaymentCalculator::new(rpc_client, rpc_block_config, vote_account_config);
        let rpc_client = fpc.rpc_client;
        let epoch_info = rpc_client.get_epoch_info().await?;
        let payer_signer = Keypair::new();

        let seeds: &[&[u8]] = &[b"test_validator_revenue", &epoch_info.epoch.to_le_bytes()];

        let tx_sig = rpc_client
            .request_airdrop(&payer_signer.pubkey(), 1_000_000_000)
            .await
            .unwrap();

        while !rpc_client
            .confirm_transaction_with_commitment(&tx_sig, commitment_config)
            .await
            .unwrap()
            .value
        {
            tokio::time::sleep(Duration::from_millis(400)).await;
        }

        // Make sure airdrop went through.
        while rpc_client
            .get_balance_with_commitment(&payer_signer.pubkey(), commitment_config)
            .await
            .unwrap()
            .value
            == 0
        {
            // Airdrop doesn't get processed after a slot unfortunately.
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        let data = EpochRewards {
            epoch: epoch_info.epoch,
            rewards: vec![Reward {
                epoch: epoch_info.epoch,
                validator_id: validator_id.to_string(),
                total: 17500,
                block_priority: 0,
                jito: 10000,
                inflation: 2500,
                block_base: 5000,
            }],
        };

        write_record_to_ledger(&rpc_client, &payer_signer, &data, commitment_config, seeds).await?;

        let (record_header, record_body) =
            read_from_ledger(&rpc_client, &payer_signer, seeds, commitment_config).await?;

        assert_eq!(record_header.version, 1);

        let deserialized = borsh::from_slice::<EpochRewards>(record_body.as_slice()).unwrap();

        assert_eq!(deserialized.epoch, epoch_info.epoch);
        assert_eq!(
            deserialized.rewards.first().unwrap().validator_id,
            String::from_str(validator_id).unwrap()
        );

        Ok(())
    }
}
