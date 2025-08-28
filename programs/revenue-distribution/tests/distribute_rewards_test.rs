#![allow(unused_imports)]
mod common;

//

use std::collections::HashMap;

use doublezero_program_tools::instruction::try_build_instruction;
use doublezero_revenue_distribution::{
    instruction::{
        account::SweepDistributionTokensAccounts, ContributorRewardsConfiguration,
        DistributionMerkleRootKind, ProgramConfiguration, ProgramFlagConfiguration,
        RevenueDistributionInstructionData,
    },
    state::{
        self, find_2z_token_pda_address, find_swap_authority_address, Distribution,
        SolanaValidatorDeposit,
    },
    types::{BurnRate, DoubleZeroEpoch, RewardShare, SolanaValidatorDebt, ValidatorFee},
    DOUBLEZERO_MINT_KEY, ID,
};
use solana_program_test::tokio;
use solana_pubkey::Pubkey;
use solana_sdk::{
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::TransactionError,
};
use spl_associated_token_account_interface::address::get_associated_token_address;
use svm_hash::merkle::{merkle_root_from_indexed_pod_leaves, MerkleProof};

//
// Distribute rewards.
//

#[cfg_attr(not(feature = "development"), ignore)]
#[tokio::test]
async fn test_distribute_rewards() {
    #[cfg(feature = "development")]
    test_distribute_rewards_development().await;

    #[cfg(not(feature = "development"))]
    test_distribute_rewards_mainnet().await;
}

//#[cfg(feature = "development")]
async fn test_distribute_rewards_development() {
    use doublezero_revenue_distribution::FIXED_SOL_2Z_SWAP_RATE_FOR_DEVELOPMENT;

    //

    let mut test_setup = common::start_test().await;

    let admin_signer = Keypair::new();

    let contributor_manager_signer = Keypair::new();
    let payments_accountant_signer = Keypair::new();
    let rewards_accountant_signer = Keypair::new();
    let solana_validator_base_block_rewards_pct_fee = 500; // 5%.

    // Community burn rate.
    let initial_cbr = 100_000_000; // 10%.
    let cbr_limit = 500_000_000; // 50%.
    let dz_epochs_to_increasing_cbr = 10;
    let dz_epochs_to_cbr_limit = 20;

    // Relay settings. We are setting this to 128 * 6960 to ensure that the
    // relayer can get paid without the transaction reverting. But practically,
    // the relayer will have enough lamports to be rent exempt so this will not
    // be a problem if the configured value is less than this.
    let contributor_reward_claim_relay_lamports = 128 * 6_960;

    // Distribution debt.

    let dz_epoch = DoubleZeroEpoch::new(1);
    let next_dz_epoch = dz_epoch.saturating_add_duration(1);

    let debt_data = (0..8)
        .map(|i| SolanaValidatorDebt {
            node_id: Pubkey::new_unique(),
            amount: 10_000_000_000 * (i + 1),
        })
        .collect::<Vec<_>>();

    let total_solana_validators = debt_data.len() as u32;
    let total_solana_validator_debt = debt_data.iter().map(|debt| debt.amount).sum();
    let solana_validator_debt_merkle_root =
        merkle_root_from_indexed_pod_leaves(&debt_data, Some(SolanaValidatorDebt::LEAF_PREFIX))
            .unwrap();

    let swap_authority_key = find_swap_authority_address().0;
    let swap_destination_key = find_2z_token_pda_address(&swap_authority_key).0;

    // Do not pay all debt. Forgive one poor soul.
    let uncollectible_index = 2;
    let uncollectible_debt = debt_data[uncollectible_index];

    // Swap destination has more than enough 2Z tokens to cover the SOL debt.
    let swap_destination_balance_before = 42_069_420 * u64::pow(10, 8);

    let expected_swept_2z_amount_1 =
        total_solana_validator_debt * FIXED_SOL_2Z_SWAP_RATE_FOR_DEVELOPMENT;
    let expected_swept_2z_amount_2 = (total_solana_validator_debt - uncollectible_debt.amount)
        * FIXED_SOL_2Z_SWAP_RATE_FOR_DEVELOPMENT;
    assert!(
        swap_destination_balance_before >= expected_swept_2z_amount_1 + expected_swept_2z_amount_2
    );

    let minimum_epoch_duration_to_finalize_rewards = 1;

    test_setup
        .initialize_program()
        .await
        .unwrap()
        .initialize_journal()
        .await
        .unwrap()
        .initialize_swap_destination(&DOUBLEZERO_MINT_KEY)
        .await
        .unwrap()
        .transfer_2z(&swap_destination_key, swap_destination_balance_before)
        .await
        .unwrap()
        .set_admin(&admin_signer.pubkey())
        .await
        .unwrap()
        .configure_program(
            &admin_signer,
            [
                ProgramConfiguration::ContributorManager(contributor_manager_signer.pubkey()),
                ProgramConfiguration::PaymentsAccountant(payments_accountant_signer.pubkey()),
                ProgramConfiguration::RewardsAccountant(rewards_accountant_signer.pubkey()),
                ProgramConfiguration::SolanaValidatorFeeParameters {
                    base_block_rewards_pct: solana_validator_base_block_rewards_pct_fee,
                    priority_block_rewards_pct: 0,
                    inflation_rewards_pct: 0,
                    jito_tips_pct: 0,
                    fixed_sol_amount: 0,
                    _unused: Default::default(),
                },
                ProgramConfiguration::CommunityBurnRateParameters {
                    limit: cbr_limit,
                    dz_epochs_to_increasing: dz_epochs_to_increasing_cbr,
                    dz_epochs_to_limit: dz_epochs_to_cbr_limit,
                    initial_rate: Some(initial_cbr),
                },
                ProgramConfiguration::ContributorRewardClaimLamports(
                    contributor_reward_claim_relay_lamports,
                ),
                ProgramConfiguration::MinimumEpochDurationToFinalizeRewards(
                    minimum_epoch_duration_to_finalize_rewards,
                ),
                ProgramConfiguration::Flag(ProgramFlagConfiguration::IsPaused(false)),
            ],
        )
        .await
        .unwrap()
        .initialize_distribution(&payments_accountant_signer)
        .await
        .unwrap()
        .initialize_distribution(&payments_accountant_signer)
        .await
        .unwrap()
        .configure_distribution_debt(
            dz_epoch,
            &payments_accountant_signer,
            total_solana_validators,
            total_solana_validator_debt,
            solana_validator_debt_merkle_root,
        )
        .await
        .unwrap()
        .finalize_distribution_debt(dz_epoch, &payments_accountant_signer)
        .await
        .unwrap()
        .initialize_distribution(&payments_accountant_signer)
        .await
        .unwrap()
        .configure_distribution_debt(
            next_dz_epoch,
            &payments_accountant_signer,
            total_solana_validators,
            total_solana_validator_debt,
            solana_validator_debt_merkle_root,
        )
        .await
        .unwrap()
        .finalize_distribution_debt(next_dz_epoch, &payments_accountant_signer)
        .await
        .unwrap();

    // 1. Initialize Solana validator deposit accounts.
    // 2. Transfer amount each validator owes so each can pay its debt.
    // 3. Pay each validator's debt.
    for (i, debt) in debt_data.iter().enumerate() {
        let node_id = &debt.node_id;
        let amount = debt.amount;

        let (deposit_key, _) = SolanaValidatorDeposit::find_address(node_id);

        let proof = MerkleProof::from_indexed_pod_leaves(
            &debt_data,
            i.try_into().unwrap(),
            Some(SolanaValidatorDebt::LEAF_PREFIX),
        )
        .unwrap();

        // Just pay for the second distribution.
        if i == uncollectible_index {
            test_setup
                .initialize_solana_validator_deposit(node_id)
                .await
                .unwrap()
                .transfer_lamports(&deposit_key, amount)
                .await
                .unwrap()
                .pay_solana_validator_debt(next_dz_epoch, node_id, amount, proof.clone())
                .await
                .unwrap()
                .forgive_solana_validator_debt(
                    dz_epoch,
                    next_dz_epoch,
                    &payments_accountant_signer,
                    &uncollectible_debt,
                    proof,
                )
                .await
                .unwrap();
        } else {
            test_setup
                .initialize_solana_validator_deposit(node_id)
                .await
                .unwrap()
                .transfer_lamports(&deposit_key, 2 * amount)
                .await
                .unwrap()
                .pay_solana_validator_debt(dz_epoch, node_id, amount, proof.clone())
                .await
                .unwrap()
                .pay_solana_validator_debt(next_dz_epoch, node_id, amount, proof)
                .await
                .unwrap();
        }
    }

    test_setup
        .sweep_distribution_tokens(dz_epoch)
        .await
        .unwrap()
        .sweep_distribution_tokens(next_dz_epoch)
        .await
        .unwrap();

    // Set up network contributor data.

    let rewards_manager_signer = Keypair::new();

    let rewards_data = [
        RewardShare::new(Pubkey::new_unique(), 400_000_000, false, 0).unwrap(), // 40.0%
        RewardShare::new(Pubkey::new_unique(), 250_000_000, false, 0).unwrap(), // 25.0%
        RewardShare::new(Pubkey::new_unique(), 100_000_000, false, 0).unwrap(), // 10.0%
        RewardShare::new(Pubkey::new_unique(), 50_000_000, false, 0).unwrap(),  // 5.0%
        RewardShare::new(Pubkey::new_unique(), 50_000_000, false, 0).unwrap(),  // 5.0%
        RewardShare::new(Pubkey::new_unique(), 50_000_000, false, 0).unwrap(),  // 5.0%
        RewardShare::new(Pubkey::new_unique(), 40_000_000, false, 0).unwrap(),  // 4.0%
        RewardShare::new(Pubkey::new_unique(), 30_000_000, false, 0).unwrap(),  // 3.0%
        RewardShare::new(Pubkey::new_unique(), 20_000_000, false, 0).unwrap(),  // 2.0%
        RewardShare::new(Pubkey::new_unique(), 5_000_000, false, 0).unwrap(),   // 0.5%
        RewardShare::new(Pubkey::new_unique(), 3_000_000, false, 0).unwrap(),   // 0.3%
        RewardShare::new(Pubkey::new_unique(), 2_000_000, false, 0).unwrap(),   // 0.2%
    ];
    assert_eq!(
        rewards_data.iter().map(|r| r.unit_share).sum::<u32>(),
        1_000_000_000
    );

    let total_contributors = rewards_data.len() as u32;
    let rewards_merkle_root =
        merkle_root_from_indexed_pod_leaves(&rewards_data, Some(RewardShare::LEAF_PREFIX)).unwrap();

    // Cache computed recipient proportions to check 2Z token distribution later
    // in the test.
    let mut recipient_shares = HashMap::new();

    for (
        i,
        RewardShare {
            contributor_key, ..
        },
    ) in rewards_data.iter().enumerate()
    {
        let recipients_with_abs_shares = (0..(i + 1).min(8))
            .map(|j| (j as u16 + 1, Pubkey::new_unique()))
            .collect::<Vec<_>>();

        // Attempt to normalize the shares to 10,000.
        let sum_shares = recipients_with_abs_shares
            .iter()
            .map(|(share, _)| share)
            .copied()
            .map(u32::from)
            .sum::<u32>();

        let mut recipients = recipients_with_abs_shares
            .iter()
            .copied()
            .map(|(share, recipient)| (recipient, (u32::from(share) * 10_000 / sum_shares) as u16))
            .collect::<Vec<_>>();

        // Adjust the first element to ensure the sum of shares equals 10,000.
        recipients[0].1 += 10_000 - recipients.iter().map(|(_, share)| share).sum::<u16>();
        assert_eq!(
            recipients.iter().map(|(_, share)| share).sum::<u16>(),
            10_000
        );

        recipient_shares.insert(contributor_key, recipients.clone());

        let recipient_keys = recipients
            .iter()
            .map(|(recipient, _)| recipient)
            .collect::<Vec<_>>();

        for recipient_key in recipient_keys.iter() {
            test_setup.create_2z_ata(recipient_key).await.unwrap();
        }

        test_setup
            .initialize_contributor_rewards(contributor_key)
            .await
            .unwrap()
            .set_rewards_manager(
                contributor_key,
                &contributor_manager_signer,
                &rewards_manager_signer.pubkey(),
            )
            .await
            .unwrap()
            .configure_contributor_rewards(
                contributor_key,
                &rewards_manager_signer,
                [ContributorRewardsConfiguration::Recipients(recipients)],
            )
            .await
            .unwrap();
    }

    // Post rewards merkle root and verify each reward share. Calling the verify
    // instruction doesn't actually do anything in this test, but it is
    // something the offchain process should parse the logs for to make sure
    // everything checks out.
    //
    // Finalize the rewards root immediately after.

    let proofs = rewards_data
        .iter()
        .enumerate()
        .map(|(i, _)| {
            MerkleProof::from_indexed_pod_leaves(
                &rewards_data,
                i.try_into().unwrap(),
                Some(RewardShare::LEAF_PREFIX),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();

    test_setup
        .configure_distribution_rewards(
            dz_epoch,
            &rewards_accountant_signer,
            total_contributors,
            rewards_merkle_root,
        )
        .await
        .unwrap();

    let kinds_and_proofs = rewards_data
        .iter()
        .copied()
        .zip(proofs.iter())
        .map(|(reward_share, proof)| {
            let kind = DistributionMerkleRootKind::RewardShare(reward_share);

            (kind, proof.clone())
        })
        .collect::<Vec<_>>();

    test_setup
        .verify_distribution_merkle_root(dz_epoch, kinds_and_proofs)
        .await
        .unwrap()
        .finalize_distribution_rewards(dz_epoch)
        .await
        .unwrap();

    for (share, proof) in rewards_data.iter().copied().zip(proofs) {
        let contributor_key = &share.contributor_key;
        let recipient_keys = recipient_shares[contributor_key]
            .iter()
            .map(|(key, _)| key)
            .collect::<Vec<_>>();

        let relayer_key = Pubkey::new_unique();

        // Distribute for the first epoch.

        test_setup
            .distribute_rewards(
                dz_epoch,
                contributor_key,
                &DOUBLEZERO_MINT_KEY,
                &relayer_key,
                &recipient_keys,
                share.unit_share,
                share.economic_burn_rate(),
                proof,
            )
            .await
            .unwrap();
    }

    //
}

#[cfg(not(feature = "development"))]
async fn test_distribute_rewards_mainnet() {
    todo!()
}
