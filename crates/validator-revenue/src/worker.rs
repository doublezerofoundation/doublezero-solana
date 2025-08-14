use crate::{Reward, ValidatorRewards};
use chrono::Utc;
use cron::Schedule;
use solana_client::rpc_config::{RpcBlockConfig, RpcGetVoteAccountsConfig};
use std::{
    collections::HashMap,
    panic,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    task::{self, JoinHandle},
    time::{self, Duration},
};

pub struct Supervisor {
    supervisor_handle: Option<JoinHandle<()>>,
    is_running: Arc<AtomicBool>,
    last_known_epoch: Option<u64>,
}

impl Supervisor {
    pub async fn new(
        fee_payment_calculator: Arc<impl ValidatorRewards + Send + Sync + 'static>,
        rpc_get_vote_accounts_config: RpcGetVoteAccountsConfig,
        rpc_block_config: RpcBlockConfig,
        validator_ids: Vec<String>,
        cron_schedule: &str,
    ) -> Result<Self, solana_client::client_error::ClientError> {
        let is_running = Arc::new(AtomicBool::new(true));
        let supervisor_is_running = is_running.clone();
        let cron_string = cron_schedule.to_string();

        // fetch current epoch on start
        let epoch_info = fee_payment_calculator.get_epoch_info().await?;
        let last_known_epoch = epoch_info.epoch;
        // TODO: check if record already exists, if so, sleep for an hour, if not get and post reward

        // now go into the configured update interval
        let supervisor_handle = task::spawn(async move {
            println!("[Supervisor] Starting up");

            while supervisor_is_running.load(Ordering::SeqCst) {
                let worker_running_signal = supervisor_is_running.clone();
                let cron_string = cron_string.clone();
                let fee_payment_calculator = fee_payment_calculator.clone();

                println!("[Supervisor] spawning a new worker");
                let worker_handle = spawn_worker_task(
                    worker_running_signal,
                    cron_string,
                    rpc_get_vote_accounts_config.clone(),
                    rpc_block_config,
                    fee_payment_calculator,
                    validator_ids.clone(),
                    last_known_epoch,
                );

                let join_result = worker_handle.await;

                if !supervisor_is_running.load(Ordering::SeqCst) {
                    println!("[Supervisor] Shutdown received");
                    break;
                };

                match join_result {
                    Err(e) if e.is_panic() => {
                        eprintln!("[Supervisor] Worker panicked - restarting in 5 seconds");
                    }
                    Ok(_) => {
                        println!("[Supervisor] Worker task completed - restarting in 5 seconds");
                    }
                    Err(e) => {
                        eprintln!("[Supervisor] Worker task was canceled or failed with error: {e} - restarting in 5 seconds")
                    }
                }

                time::sleep(Duration::from_secs(5)).await;
            }

            println!("[Supervisor] Shutting down")
        });

        Ok(Self {
            supervisor_handle: Some(supervisor_handle),
            is_running,
            last_known_epoch: Some(last_known_epoch),
        })
    }

    pub fn get_last_known_epoch(self) -> u64 {
        self.last_known_epoch.unwrap()
    }

    pub async fn stop(mut self) {
        self.is_running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.supervisor_handle.take() {
            handle.await.expect("failed to join thread");
        }
        println!("worker stopped");
    }
}

fn spawn_worker_task(
    is_running: Arc<AtomicBool>,
    cron_expression: String,
    _rpc_get_vote_accounts_config: RpcGetVoteAccountsConfig,
    _rpc_block_config: RpcBlockConfig,
    fee_payment_calculator: Arc<impl ValidatorRewards + Send + Sync + 'static>,
    _validator_ids: Vec<String>,
    epoch: u64,
) -> JoinHandle<()> {
    task::spawn(async move {
        let schedule = Schedule::from_str(&cron_expression).unwrap();

        println!("[Worker] started with schedule {}", &schedule);

        while is_running.load(Ordering::SeqCst) {
            if let Some(next) = schedule.upcoming(Utc).next() {
                let now = Utc::now();
                if let Ok(sleep_duration) = next.signed_duration_since(now).to_std() {
                    println!("[Worker] Next job in -{sleep_duration:.1?} at {next}");
                    tokio::select! {
                        _ = time::sleep(sleep_duration) => {
                            // slumber has ended, get to work
                        }
                        _ = async { while is_running.load(Ordering::SeqCst) { time::sleep(Duration::from_millis(100)).await; } } => {
                            println!("[Worker] Shutdown signal received during sleep. Exiting.");
                            continue;
                        }
                    }
                }

                if !is_running.load(Ordering::SeqCst) {
                    continue;
                }

                println!("[Worker] Executing job at {}", Utc::now());
                let _epoch_info = match fee_payment_calculator.get_epoch_info().await {
                    Ok(epoch_info) => epoch_info,
                    Err(e) => {
                        eprintln!("[Worker] Failed to get epoch info {e}");
                        break;
                    }
                };

                // TODO: this is going to fail so faking it for now
                println!("Checking epoch {epoch}");
                if epoch == 999 {
                    break;
                }

                // TODO: fetch rewards
                // let _rewards = match get_total_rewards(
                //     &*fee_payment_calculator,
                //     validator_ids.as_slice(),
                //     epoch,
                //     rpc_get_vote_accounts_config.clone(),
                //     rpc_block_config.clone(),
                // )
                // .await
                // {
                //     Ok(rewards) => rewards,
                //     Err(e) => {
                //         eprintln!("[Worker] Failed to get_total_rewards: {e} ");
                //         break;
                //     }
                // };
                //
                // TODO: post rewards
                // TOOD: generate merkle root
            } else {
                println!("[Worker] No future jobs. Shutting down");
                break;
            }
        }

        println!("[Worker] Task shutting down gracefully");
    })
}
#[allow(dead_code)]
async fn post_rewards_for_epoch(
    _reward: HashMap<String, Reward>,
    _epoch: u64,
) -> anyhow::Result<bool> {
    // TODO get the rewards
    // TODO post reward
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_total_rewards;
    use crate::rewards::{self, JitoReward, JitoRewards, MockValidatorRewards};
    use solana_client::rpc_config::RpcGetVoteAccountsConfig;
    use solana_client::rpc_response::{
        RpcInflationReward, RpcVoteAccountInfo, RpcVoteAccountStatus,
    };
    use solana_sdk::{
        commitment_config::CommitmentConfig, epoch_info::EpochInfo, reward_type::RewardType::Fee,
    };
    use solana_transaction_status_client_types::{
        TransactionDetails, UiConfirmedBlock, UiTransactionEncoding,
    };

    #[tokio::test]
    async fn test_supervisor() {
        let validator_id = "6WgdYhhGE53WrZ7ywJA15hBVkw7CRbQ8yDBBTwmBtAHN";
        let validator_ids: Vec<String> = vec![String::from(validator_id)];
        let epoch = 819;
        let block_reward: u64 = 5000;
        let inflation_reward = 2500;
        let jito_reward = 10000;

        let mut mock_fee_payment_calculator = MockValidatorRewards::new();

        // Define RPC configurations that will be passed to the function under test.
        let rpc_get_vote_accounts_config = RpcGetVoteAccountsConfig {
            vote_pubkey: None,
            commitment: CommitmentConfig::finalized().into(),
            keep_unstaked_delinquents: None,
            delinquent_slot_distance: None,
        };

        // Set up mock expectations for the ValidatorRewards trait.
        // These mocks simulate the behavior of external dependencies.
        let mock_rpc_vote_account_status = RpcVoteAccountStatus {
            current: vec![RpcVoteAccountInfo {
                vote_pubkey: "6WgdYhhGE53WrZ7ywJA15hBVkw7CRbQ8yDBBTwmBtABB".to_string(),
                node_pubkey: validator_id.to_string(),
                activated_stake: 4_200_000_000_000,
                epoch_vote_account: true,
                epoch_credits: vec![(812, 256, 128), (811, 128, 64)],
                commission: 10,
                last_vote: 123456789,
                root_slot: 123456700,
            }],
            delinquent: vec![],
        };

        mock_fee_payment_calculator
            .expect_get_vote_accounts_with_config()
            .withf(move |_| true)
            .times(1)
            // Use a move closure here
            .returning(move |_| Ok(mock_rpc_vote_account_status.clone()));

        let mock_rpc_inflation_reward = vec![Some(RpcInflationReward {
            epoch,
            effective_slot: 123456789,
            amount: inflation_reward,
            post_balance: 1_500_002_500,
            commission: Some(1),
        })];

        mock_fee_payment_calculator
            .expect_get_inflation_reward()
            .times(1)
            // Use a move closure here
            .returning(move |_, _| Ok(mock_rpc_inflation_reward.clone()));

        let first_slot = rewards::get_first_slot_for_epoch(epoch);
        let slot_index: usize = 10;
        let slot = first_slot + slot_index as u64;

        let mut leader_schedule = HashMap::new();
        leader_schedule.insert(validator_id.to_string(), vec![slot_index]);

        mock_fee_payment_calculator
            .expect_get_leader_schedule()
            .times(1)
            // Use a move closure here
            .returning(move || Ok(leader_schedule.clone()));

        let mock_block = UiConfirmedBlock {
            num_reward_partitions: Some(1),
            signatures: Some(vec!["One".to_string()]),
            rewards: Some(vec![solana_transaction_status_client_types::Reward {
                pubkey: validator_id.to_string(),
                lamports: block_reward as i64,
                post_balance: block_reward,
                reward_type: Some(Fee),
                commission: None,
            }]),
            previous_blockhash: "".to_string(),
            blockhash: "".to_string(),
            parent_slot: 0,
            transactions: None,
            block_time: None,
            block_height: None,
        };

        let epoch_info = EpochInfo {
            epoch,
            slot_index: 0,
            slots_in_epoch: 432000,
            absolute_slot: epoch * 432000,
            block_height: 0,
            transaction_count: Some(0),
        };

        mock_fee_payment_calculator
            .expect_get_epoch_info()
            .returning(move || Ok(epoch_info.clone()));

        mock_fee_payment_calculator
            .expect_get_block_with_config()
            .withf(move |s, _| *s == slot)
            .times(1)
            .returning(move |_, _| Ok(mock_block.clone()));

        mock_fee_payment_calculator
            .expect_get::<JitoRewards>()
            .withf(move |url| url.contains(&format!("epoch={epoch}")))
            .times(1)
            .returning(move |_| {
                Ok(JitoRewards {
                    total_count: 1000,
                    rewards: vec![JitoReward {
                        vote_account: validator_id.to_string(),
                        mev_revenue: jito_reward,
                    }],
                })
            });

        let rpc_block_config = solana_client::rpc_config::RpcBlockConfig {
            encoding: Some(UiTransactionEncoding::Base58),
            transaction_details: Some(TransactionDetails::None),
            rewards: Some(true),
            commitment: Some(CommitmentConfig::finalized()),
            max_supported_transaction_version: Some(0),
        };

        let _rewards = get_total_rewards(
            &mock_fee_payment_calculator,
            validator_ids.as_slice(),
            epoch,
            rpc_get_vote_accounts_config.clone(),
            rpc_block_config,
        )
        .await
        .unwrap();

        let test_schedule = "*/2 * * * * *"; // Every 2 seconds

        let supervisor = Supervisor::new(
            Arc::new(mock_fee_payment_calculator),
            rpc_get_vote_accounts_config,
            rpc_block_config,
            validator_ids,
            test_schedule,
        )
        .await
        .unwrap();

        time::sleep(Duration::from_secs(1)).await;

        supervisor.stop().await;
    }
}
