mod common;

//

use doublezero_revenue_distribution::{
    instruction::{
        BadSolanaValidatorDebtResolution, ProgramConfiguration, ProgramFeatureConfiguration,
        ProgramFlagConfiguration,
    },
    state::{self, Distribution, SolanaValidatorDeposit},
    types::{BurnRate, DoubleZeroEpoch, SolanaValidatorDebt, ValidatorFee},
    DOUBLEZERO_MINT_KEY,
};
use solana_program_test::tokio;
use solana_pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use svm_hash::{
    merkle::{merkle_root_from_indexed_pod_leaves, MerkleProof},
    sha2::Hash,
};

//
// Resolve bad Solana validator debt.
//

#[tokio::test]
async fn test_resolve_bad_solana_validator_debt() {
    let transfer_authority_signer = Keypair::new();

    let bootstrapped_accounts = common::generate_token_accounts_for_test(
        &DOUBLEZERO_MINT_KEY,
        &[transfer_authority_signer.pubkey()],
    );
    let src_token_account_key = bootstrapped_accounts.first().unwrap().key;

    let mut test_setup = common::start_test_with_accounts(bootstrapped_accounts).await;

    let admin_signer = Keypair::new();
    let debt_accountant_signer = Keypair::new();
    let rewards_accountant_signer = Keypair::new();

    // Community burn rate.
    let initial_cbr = 100_000_000; // 10%.

    // Relay settings.
    let distribute_rewards_relay_lamports = 10_000;

    // Distribution debt - create 8 validators with debt.
    let debt_data = (0..8)
        .map(|i| SolanaValidatorDebt {
            node_id: Pubkey::new_unique(),
            amount: 10_000_000_000 * (i + 1),
        })
        .collect::<Vec<_>>();

    let total_solana_validators = debt_data.len() as u32;
    let total_solana_validator_debt: u64 = debt_data.iter().map(|debt| debt.amount).sum();
    let solana_validator_debt_merkle_root =
        merkle_root_from_indexed_pod_leaves(&debt_data, Some(SolanaValidatorDebt::LEAF_PREFIX))
            .unwrap();

    // Index of the validator whose debt will be written off and reclassified.
    let bad_debt_index = 2;
    let bad_debt = debt_data[bad_debt_index];

    let expected_swept_2z_amount = 420 * u64::pow(10, 8);

    // Distribution rewards.
    let total_contributors = 2;
    let rewards_merkle_root = Hash::new_unique();

    // Target epochs.
    let dz_epoch = DoubleZeroEpoch::new(0);
    let bad_debt_dz_epoch = dz_epoch.saturating_add_duration(1);

    // Initialize program and set up distribution.
    test_setup
        .transfer_2z(&src_token_account_key, expected_swept_2z_amount)
        .await
        .unwrap()
        .initialize_program()
        .await
        .unwrap()
        .initialize_journal()
        .await
        .unwrap()
        .set_admin(&admin_signer.pubkey())
        .await
        .unwrap()
        .configure_program(
            &admin_signer,
            [
                ProgramConfiguration::Sol2zSwapProgram(mock_swap_sol_2z::ID),
                ProgramConfiguration::DebtAccountant(debt_accountant_signer.pubkey()),
                ProgramConfiguration::RewardsAccountant(rewards_accountant_signer.pubkey()),
                ProgramConfiguration::SolanaValidatorFeeParameters {
                    base_block_rewards_pct: 500,
                    priority_block_rewards_pct: 0,
                    inflation_rewards_pct: 0,
                    jito_tips_pct: 0,
                    fixed_sol_amount: 0,
                    _unused: Default::default(),
                },
                ProgramConfiguration::CommunityBurnRateParameters {
                    limit: 500_000_000,
                    dz_epochs_to_increasing: 10,
                    dz_epochs_to_limit: 20,
                    initial_rate: Some(initial_cbr),
                },
                ProgramConfiguration::DistributeRewardsRelayLamports(
                    distribute_rewards_relay_lamports,
                ),
                ProgramConfiguration::CalculationGracePeriodMinutes(1),
                ProgramConfiguration::DistributionInitializationGracePeriodMinutes(1),
                ProgramConfiguration::MinimumEpochDurationToFinalizeRewards(1),
                ProgramConfiguration::Flag(ProgramFlagConfiguration::IsPaused(false)),
                ProgramConfiguration::FeatureActivation {
                    feature: ProgramFeatureConfiguration::SolanaValidatorDebtWriteOff,
                    activation_epoch: DoubleZeroEpoch::new(1),
                },
            ],
        )
        .await
        .unwrap()
        // Initialize epoch 0.
        .initialize_distribution(&debt_accountant_signer)
        .await
        .unwrap()
        .warp_timestamp_by(60)
        .await
        .unwrap()
        // Initialize epoch 1 (bad_debt_dz_epoch).
        .initialize_distribution(&debt_accountant_signer)
        .await
        .unwrap()
        .warp_timestamp_by(60)
        .await
        .unwrap()
        .configure_distribution_debt(
            bad_debt_dz_epoch,
            &debt_accountant_signer,
            total_solana_validators,
            total_solana_validator_debt,
            solana_validator_debt_merkle_root,
        )
        .await
        .unwrap()
        .finalize_distribution_debt(bad_debt_dz_epoch, &debt_accountant_signer)
        .await
        .unwrap()
        .enable_solana_validator_debt_write_off(bad_debt_dz_epoch)
        .await
        .unwrap()
        .enable_erroneous_solana_validator_debt(bad_debt_dz_epoch)
        .await
        .unwrap()
        .configure_distribution_rewards(
            bad_debt_dz_epoch,
            &rewards_accountant_signer,
            total_contributors,
            rewards_merkle_root,
        )
        .await
        .unwrap()
        .initialize_swap_destination(&DOUBLEZERO_MINT_KEY)
        .await
        .unwrap();

    // Initialize deposit accounts and pay debt for all validators except the
    // bad debt validator.
    for (i, debt) in debt_data.iter().enumerate() {
        let node_id = &debt.node_id;

        // Just initialize the bad debt validator's deposit account (without
        // paying debt).
        if i == bad_debt_index {
            test_setup
                .initialize_solana_validator_deposit(node_id)
                .await
                .unwrap();
            continue;
        }

        let proof = MerkleProof::from_indexed_pod_leaves(
            &debt_data,
            i.try_into().unwrap(),
            Some(SolanaValidatorDebt::LEAF_PREFIX),
        )
        .unwrap();

        let (deposit_key, _) = SolanaValidatorDeposit::find_address(node_id);

        test_setup
            .initialize_solana_validator_deposit(node_id)
            .await
            .unwrap()
            .transfer_lamports(&deposit_key, debt.amount)
            .await
            .unwrap()
            .pay_solana_validator_debt(bad_debt_dz_epoch, debt, proof)
            .await
            .unwrap();
    }

    // Finalize epoch 0 so we can sweep.
    test_setup
        .finalize_distribution_debt(dz_epoch, &debt_accountant_signer)
        .await
        .unwrap()
        .finalize_distribution_rewards(dz_epoch)
        .await
        .unwrap()
        .finalize_distribution_rewards(bad_debt_dz_epoch)
        .await
        .unwrap();

    // Write off the bad debt before sweeping.
    let bad_debt_proof = MerkleProof::from_indexed_pod_leaves(
        &debt_data,
        bad_debt_index.try_into().unwrap(),
        Some(SolanaValidatorDebt::LEAF_PREFIX),
    )
    .unwrap();

    test_setup
        .write_off_solana_validator_debt(
            bad_debt_dz_epoch,
            bad_debt_dz_epoch,
            &debt_accountant_signer,
            &bad_debt,
            bad_debt_proof.clone(),
        )
        .await
        .unwrap();

    // Swap SOL for 2Z tokens - only need enough for collected debt (excluding
    // written-off).
    let sol_destination_key = Pubkey::new_unique();
    test_setup
        .mock_buy_sol(
            &src_token_account_key,
            &transfer_authority_signer,
            &sol_destination_key,
            expected_swept_2z_amount,
            total_solana_validator_debt - bad_debt.amount,
        )
        .await
        .unwrap();

    // Sweep epoch 0 first, then epoch 1.
    test_setup
        .sweep_distribution_tokens(dz_epoch)
        .await
        .unwrap()
        .sweep_distribution_tokens(bad_debt_dz_epoch)
        .await
        .unwrap();

    // Verify state before reclassification.
    let (distribution_key, distribution_before, remaining_data_before, _, _) =
        test_setup.fetch_distribution(bad_debt_dz_epoch).await;

    let write_off_bitmap = &remaining_data_before
        [distribution_before.written_off_solana_validator_debt_bitmap_range()];
    assert_eq!(write_off_bitmap, [0b00000100]);

    let (_, deposit_before) = test_setup
        .fetch_solana_validator_deposit(&bad_debt.node_id)
        .await;

    let mut expected_deposit = SolanaValidatorDeposit::default();
    expected_deposit.node_id = bad_debt.node_id;
    expected_deposit.written_off_sol_debt = bad_debt.amount;
    assert_eq!(deposit_before, expected_deposit);

    // Reclassify the written-off debt as erroneous.
    test_setup
        .resolve_bad_solana_validator_debt(
            bad_debt_dz_epoch,
            None, // windfall_dz_epoch
            &debt_accountant_signer,
            &bad_debt,
            bad_debt_proof,
            BadSolanaValidatorDebtResolution::ReclassifyErroneous,
        )
        .await
        .unwrap();

    // Verify state after ReclassifyErroneous.
    let (_, distribution_after, remaining_data_after, _, _) =
        test_setup.fetch_distribution(bad_debt_dz_epoch).await;

    let erroneous_bitmap_range = distribution_after
        .checked_erroneous_solana_validator_debt_bitmap_range()
        .expect("Erroneous bitmap should exist");
    let erroneous_bitmap = &remaining_data_after[erroneous_bitmap_range];
    assert_eq!(erroneous_bitmap, [0b00000100]);

    let init = ExpectedDistributionInitializer {
        dz_epoch: bad_debt_dz_epoch,
        distribution_key,
        initial_cbr,
        distribute_rewards_relay_lamports,
        total_solana_validators,
        total_solana_validator_debt,
        solana_validator_debt_merkle_root,
        total_contributors,
        rewards_merkle_root,
    };

    let mut expected_distribution = build_expected_distribution(&distribution_after, &init);
    expected_distribution.uncollectible_sol_debt = bad_debt.amount;
    expected_distribution.solana_validator_write_off_count = 1;
    expected_distribution.collected_2z_converted_from_sol = expected_swept_2z_amount;
    expected_distribution.erroneous_sol_debt = bad_debt.amount;
    assert_eq!(distribution_after, expected_distribution);

    let (_, deposit_after) = test_setup
        .fetch_solana_validator_deposit(&bad_debt.node_id)
        .await;

    let mut expected_deposit = SolanaValidatorDeposit::default();
    expected_deposit.node_id = bad_debt.node_id;
    expected_deposit.written_off_sol_debt = bad_debt.amount;
    expected_deposit.erroneous_sol_debt = bad_debt.amount;
    assert_eq!(deposit_after, expected_deposit);

    // Reclassify as unpaid (undo the erroneous classification).
    let bad_debt_proof = MerkleProof::from_indexed_pod_leaves(
        &debt_data,
        bad_debt_index.try_into().unwrap(),
        Some(SolanaValidatorDebt::LEAF_PREFIX),
    )
    .unwrap();

    test_setup
        .resolve_bad_solana_validator_debt(
            bad_debt_dz_epoch,
            None, // windfall_dz_epoch
            &debt_accountant_signer,
            &bad_debt,
            bad_debt_proof,
            BadSolanaValidatorDebtResolution::ReclassifyUnpaid,
        )
        .await
        .unwrap();

    // Verify state after ReclassifyUnpaid.
    let (_, distribution_after_unpaid, remaining_data_after_unpaid, _, _) =
        test_setup.fetch_distribution(bad_debt_dz_epoch).await;

    let erroneous_bitmap_range = distribution_after_unpaid
        .checked_erroneous_solana_validator_debt_bitmap_range()
        .expect("Erroneous bitmap should exist");
    let erroneous_bitmap = &remaining_data_after_unpaid[erroneous_bitmap_range];
    assert_eq!(erroneous_bitmap, [0b00000000]);

    let mut expected_distribution = build_expected_distribution(&distribution_after_unpaid, &init);
    expected_distribution.uncollectible_sol_debt = bad_debt.amount;
    expected_distribution.solana_validator_write_off_count = 1;
    expected_distribution.collected_2z_converted_from_sol = expected_swept_2z_amount;
    expected_distribution.erroneous_sol_debt = 0;
    assert_eq!(distribution_after_unpaid, expected_distribution);

    let (_, deposit_after_unpaid) = test_setup
        .fetch_solana_validator_deposit(&bad_debt.node_id)
        .await;

    let mut expected_deposit = SolanaValidatorDeposit::default();
    expected_deposit.node_id = bad_debt.node_id;
    expected_deposit.written_off_sol_debt = bad_debt.amount;
    expected_deposit.erroneous_sol_debt = 0;
    assert_eq!(deposit_after_unpaid, expected_deposit);

    // Resolve with recover, which requires a windfall distribution.
    let windfall_dz_epoch = bad_debt_dz_epoch.saturating_add_duration(1);

    test_setup
        .initialize_distribution(&debt_accountant_signer)
        .await
        .unwrap()
        .warp_timestamp_by(60)
        .await
        .unwrap()
        .configure_distribution_debt(
            windfall_dz_epoch,
            &debt_accountant_signer,
            0, // No validators needed for windfall
            0,
            Hash::default(),
        )
        .await
        .unwrap()
        .finalize_distribution_debt(windfall_dz_epoch, &debt_accountant_signer)
        .await
        .unwrap();

    // Transfer lamports to the validator deposit so it can pay the recovered
    // debt.
    let (deposit_key, _) = SolanaValidatorDeposit::find_address(&bad_debt.node_id);
    test_setup
        .transfer_lamports(&deposit_key, bad_debt.amount)
        .await
        .unwrap();

    // Get journal state before recovery.
    let (_, journal_before, _) = test_setup.fetch_journal().await;

    // Recover the bad debt.
    let bad_debt_proof = MerkleProof::from_indexed_pod_leaves(
        &debt_data,
        bad_debt_index.try_into().unwrap(),
        Some(SolanaValidatorDebt::LEAF_PREFIX),
    )
    .unwrap();

    test_setup
        .resolve_bad_solana_validator_debt(
            bad_debt_dz_epoch,
            Some(windfall_dz_epoch),
            &debt_accountant_signer,
            &bad_debt,
            bad_debt_proof,
            BadSolanaValidatorDebtResolution::Recover,
        )
        .await
        .unwrap();

    // Verify bad debt distribution state after Recover.
    let (_, distribution_after_recover, remaining_data_after_recover, _, _) =
        test_setup.fetch_distribution(bad_debt_dz_epoch).await;

    let write_off_bitmap = &remaining_data_after_recover
        [distribution_after_recover.written_off_solana_validator_debt_bitmap_range()];
    assert_eq!(write_off_bitmap, [0b00000000]);

    let mut expected_distribution = build_expected_distribution(&distribution_after_recover, &init);
    expected_distribution.uncollectible_sol_debt = bad_debt.amount;
    expected_distribution.solana_validator_write_off_count = 1;
    expected_distribution.collected_2z_converted_from_sol = expected_swept_2z_amount;
    assert_eq!(distribution_after_recover, expected_distribution);

    // Verify deposit state after Recover.
    let (_, deposit_after_recover) = test_setup
        .fetch_solana_validator_deposit(&bad_debt.node_id)
        .await;

    let mut expected_deposit = SolanaValidatorDeposit::default();
    expected_deposit.node_id = bad_debt.node_id;
    expected_deposit.written_off_sol_debt = bad_debt.amount;
    expected_deposit.recovered_sol_debt = bad_debt.amount;
    assert_eq!(deposit_after_recover, expected_deposit);

    // Verify journal received the lamports.
    let (_, journal_after, _) = test_setup.fetch_journal().await;
    assert_eq!(
        journal_after.total_sol_balance,
        journal_before.total_sol_balance + bad_debt.amount
    );

    // Verify windfall distribution's recovered_sol_debt is updated.
    let (_, windfall_after, _, _, _) = test_setup.fetch_distribution(windfall_dz_epoch).await;
    assert_eq!(windfall_after.recovered_sol_debt, bad_debt.amount);
}

struct ExpectedDistributionInitializer {
    dz_epoch: DoubleZeroEpoch,
    distribution_key: Pubkey,
    initial_cbr: u32,
    distribute_rewards_relay_lamports: u32,
    total_solana_validators: u32,
    total_solana_validator_debt: u64,
    solana_validator_debt_merkle_root: Hash,
    total_contributors: u32,
    rewards_merkle_root: Hash,
}

fn build_expected_distribution(
    actual: &Distribution,
    init: &ExpectedDistributionInitializer,
) -> Distribution {
    let debt_bitmap_bytes = init.total_solana_validators.div_ceil(8);
    let rewards_bitmap_bytes = init.total_contributors.div_ceil(8);

    let mut expected = Distribution::default();
    expected.set_is_debt_calculation_finalized(true);
    expected.set_is_rewards_calculation_finalized(true);
    expected.set_has_swept_2z_tokens(true);
    expected.set_is_solana_validator_debt_write_off_enabled(true);
    expected.set_is_erroneous_solana_validator_debt_enabled(true);
    expected.bump_seed = Distribution::find_address(init.dz_epoch).1;
    expected.token_2z_pda_bump_seed = state::find_2z_token_pda_address(&init.distribution_key).1;
    expected.dz_epoch = init.dz_epoch;
    expected.community_burn_rate = BurnRate::new(init.initial_cbr).unwrap();
    expected
        .solana_validator_fee_parameters
        .base_block_rewards_pct = ValidatorFee::new(500).unwrap();
    expected.total_solana_validators = init.total_solana_validators;
    expected.solana_validator_payments_count = init.total_solana_validators - 1;
    expected.total_solana_validator_debt = init.total_solana_validator_debt;
    expected.collected_solana_validator_payments =
        init.total_solana_validator_debt - 30_000_000_000; // bad_debt.amount
    expected.solana_validator_debt_merkle_root = init.solana_validator_debt_merkle_root;
    expected.total_contributors = init.total_contributors;
    expected.rewards_merkle_root = init.rewards_merkle_root;
    // Bitmap layout: processed_debt | write_off | erroneous | processed_rewards
    expected.processed_solana_validator_debt_end_index = debt_bitmap_bytes;
    expected.written_off_solana_validator_debt_start_index = debt_bitmap_bytes;
    expected.written_off_solana_validator_debt_end_index = 2 * debt_bitmap_bytes;
    expected.erroneous_solana_validator_debt_start_index = 2 * debt_bitmap_bytes;
    expected.erroneous_solana_validator_debt_end_index = 3 * debt_bitmap_bytes;
    expected.processed_rewards_start_index = 3 * debt_bitmap_bytes;
    expected.processed_rewards_end_index = 3 * debt_bitmap_bytes + rewards_bitmap_bytes;
    expected.distribute_rewards_relay_lamports = init.distribute_rewards_relay_lamports;
    // Copy from actual since this depends on when distribution was initialized.
    expected.calculation_allowed_timestamp = actual.calculation_allowed_timestamp;
    expected
}
