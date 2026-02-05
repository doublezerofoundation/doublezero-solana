mod common;

//

use doublezero_program_tools::instruction::try_build_instruction;
use doublezero_revenue_distribution::{
    instruction::{
        account::{
            ReclassifyBadSolanaValidatorDebtAccounts, RecoverBadSolanaValidatorDebtAccounts,
        },
        ProgramConfiguration, ProgramFeatureConfiguration, ProgramFlagConfiguration,
        RevenueDistributionInstructionData,
    },
    state::SolanaValidatorDeposit,
    types::{DoubleZeroEpoch, SolanaValidatorDebt},
    DOUBLEZERO_MINT_KEY, ID,
};
use solana_program_test::tokio;
use solana_pubkey::Pubkey;
use solana_sdk::{
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::TransactionError,
};
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
    // debt_1 (index 2) and debt_2 (index 5) will be written off.
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

    // Indices of the validators whose debt will be written off and resolved.
    let debt_1_index = 2;
    let debt_2_index = 5;
    let debt_1 = debt_data[debt_1_index];
    let debt_2 = debt_data[debt_2_index];

    let expected_swept_2z_amount = 420 * u64::pow(10, 8);

    // Distribution rewards.
    let total_contributors = 2;
    let rewards_merkle_root = Hash::new_unique();

    // Target epochs.
    // Epoch 0 is skipped (feature cannot be activated at epoch 0).
    // bad_debt_dz_epoch = 1, windfall_dz_epoch = 2.
    let bad_debt_dz_epoch = DoubleZeroEpoch::new(1);
    let windfall_dz_epoch = bad_debt_dz_epoch.saturating_add_duration(1);

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
                // Low value so recovery can succeed immediately.
                ProgramConfiguration::MinimumEpochDurationToRecoverDebt(1),
            ],
        )
        .await
        .unwrap()
        // Initialize epoch 0 (skipped epoch, just for feature activation).
        .initialize_distribution(&debt_accountant_signer)
        .await
        .unwrap()
        .warp_timestamp_by(60)
        .await
        .unwrap()
        .finalize_distribution_debt(DoubleZeroEpoch::new(0), &debt_accountant_signer)
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
    // bad debt validators.
    for (i, debt) in debt_data.iter().enumerate() {
        let node_id = &debt.node_id;

        // Just initialize the bad debt validators' deposit accounts (without
        // paying debt).
        if i == debt_1_index || i == debt_2_index {
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

    // Write off first and second debts.
    let debt_1_proof = MerkleProof::from_indexed_pod_leaves(
        &debt_data,
        debt_1_index.try_into().unwrap(),
        Some(SolanaValidatorDebt::LEAF_PREFIX),
    )
    .unwrap();
    let debt_2_proof = MerkleProof::from_indexed_pod_leaves(
        &debt_data,
        debt_2_index.try_into().unwrap(),
        Some(SolanaValidatorDebt::LEAF_PREFIX),
    )
    .unwrap();

    // Attempt to resolve with unauthorized signer (should fail).
    let unauthorized_signer = Keypair::new();
    let resolve_unauthorized_ix = try_build_instruction(
        &ID,
        ReclassifyBadSolanaValidatorDebtAccounts::new(
            &unauthorized_signer.pubkey(),
            &debt_1.node_id,
            bad_debt_dz_epoch,
        ),
        &RevenueDistributionInstructionData::ReclassifyBadSolanaValidatorDebt {
            amount: debt_1.amount,
            proof: debt_1_proof.clone(),
            classify_erroneous: true,
        },
    )
    .unwrap();

    let (tx_err, program_logs) = test_setup
        .unwrap_simulation_error(
            std::slice::from_ref(&resolve_unauthorized_ix),
            &[&unauthorized_signer],
        )
        .await;
    assert_eq!(
        tx_err,
        TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
    );
    assert_eq!(
        program_logs.get(2).unwrap(),
        "Program log: Unauthorized debt accountant (account 1)"
    );

    // Attempt to resolve before write-offs are enabled (should fail).
    let resolve_before_writeoff_enabled_ix = try_build_instruction(
        &ID,
        ReclassifyBadSolanaValidatorDebtAccounts::new(
            &debt_accountant_signer.pubkey(),
            &debt_1.node_id,
            bad_debt_dz_epoch,
        ),
        &RevenueDistributionInstructionData::ReclassifyBadSolanaValidatorDebt {
            amount: debt_1.amount,
            proof: debt_1_proof.clone(),
            classify_erroneous: true,
        },
    )
    .unwrap();

    let (tx_err, program_logs) = test_setup
        .unwrap_simulation_error(
            std::slice::from_ref(&resolve_before_writeoff_enabled_ix),
            &[&debt_accountant_signer],
        )
        .await;
    assert_eq!(
        tx_err,
        TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
    );
    assert_eq!(
        program_logs.get(3).unwrap(),
        "Program log: Solana validator debt write off is not enabled yet"
    );

    // Enable write-offs.
    test_setup
        .enable_solana_validator_debt_write_off(bad_debt_dz_epoch)
        .await
        .unwrap();

    // Attempt to resolve debt that hasn't been written off yet (should fail).
    let resolve_before_writeoff_ix = try_build_instruction(
        &ID,
        ReclassifyBadSolanaValidatorDebtAccounts::new(
            &debt_accountant_signer.pubkey(),
            &debt_1.node_id,
            bad_debt_dz_epoch,
        ),
        &RevenueDistributionInstructionData::ReclassifyBadSolanaValidatorDebt {
            amount: debt_1.amount,
            proof: debt_1_proof.clone(),
            classify_erroneous: true,
        },
    )
    .unwrap();

    let (tx_err, program_logs) = test_setup
        .unwrap_simulation_error(
            std::slice::from_ref(&resolve_before_writeoff_ix),
            &[&debt_accountant_signer],
        )
        .await;
    assert_eq!(
        tx_err,
        TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
    );
    assert_eq!(
        program_logs.get(4).unwrap(),
        &format!(
            "Program log: Solana validator debt was not written off for epoch {}",
            bad_debt_dz_epoch
        )
    );

    // Write off first and second debts.
    test_setup
        .write_off_solana_validator_debt(
            bad_debt_dz_epoch,
            bad_debt_dz_epoch,
            &debt_accountant_signer,
            &debt_1,
            debt_1_proof.clone(),
        )
        .await
        .unwrap()
        .write_off_solana_validator_debt(
            bad_debt_dz_epoch,
            bad_debt_dz_epoch,
            &debt_accountant_signer,
            &debt_2,
            debt_2_proof.clone(),
        )
        .await
        .unwrap()
        // Must sweep epoch 0 first (sequential sweeping required).
        .finalize_distribution_rewards(DoubleZeroEpoch::new(0))
        .await
        .unwrap()
        .sweep_distribution_tokens(DoubleZeroEpoch::new(0))
        .await
        .unwrap();

    // Verify write-off bitmap after both write-offs.
    let (_, distribution, remaining_data, _, _) =
        test_setup.fetch_distribution(bad_debt_dz_epoch).await;

    let write_off_bitmap =
        &remaining_data[distribution.written_off_solana_validator_debt_bitmap_range()];
    assert_eq!(write_off_bitmap, [0b00100100]);

    // Attempt to resolve debt with wrong merkle proof (use debt_2_proof for
    // debt_1).
    let resolve_wrong_proof_ix = try_build_instruction(
        &ID,
        ReclassifyBadSolanaValidatorDebtAccounts::new(
            &debt_accountant_signer.pubkey(),
            &debt_1.node_id,
            bad_debt_dz_epoch,
        ),
        &RevenueDistributionInstructionData::ReclassifyBadSolanaValidatorDebt {
            amount: debt_1.amount,
            proof: debt_2_proof.clone(),
            classify_erroneous: true,
        },
    )
    .unwrap();

    let (tx_err, program_logs) = test_setup
        .unwrap_simulation_error(
            std::slice::from_ref(&resolve_wrong_proof_ix),
            &[&debt_accountant_signer],
        )
        .await;
    assert_eq!(
        tx_err,
        TransactionError::InstructionError(0, InstructionError::InvalidInstructionData)
    );
    // The computed merkle root will be wrong because we used debt_2's proof with
    // debt_1's node_id and amount.
    let computed_merkle_root = debt_2_proof.root_from_pod_leaf(
        &SolanaValidatorDebt {
            node_id: debt_1.node_id,
            amount: debt_1.amount,
        },
        Some(SolanaValidatorDebt::LEAF_PREFIX),
    );
    assert_eq!(
        program_logs.get(4).unwrap(),
        &format!(
            "Program log: Invalid computed merkle root: {}",
            computed_merkle_root
        )
    );

    // Attempt to reclassify as erroneous before erroneous debt is enabled (should
    // fail).
    let reclassify_before_erroneous_enabled_ix = try_build_instruction(
        &ID,
        ReclassifyBadSolanaValidatorDebtAccounts::new(
            &debt_accountant_signer.pubkey(),
            &debt_1.node_id,
            bad_debt_dz_epoch,
        ),
        &RevenueDistributionInstructionData::ReclassifyBadSolanaValidatorDebt {
            amount: debt_1.amount,
            proof: debt_1_proof.clone(),
            classify_erroneous: true,
        },
    )
    .unwrap();

    let (tx_err, program_logs) = test_setup
        .unwrap_simulation_error(
            std::slice::from_ref(&reclassify_before_erroneous_enabled_ix),
            &[&debt_accountant_signer],
        )
        .await;
    assert_eq!(
        tx_err,
        TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
    );
    assert_eq!(
        program_logs.get(4).unwrap(),
        "Program log: Erroneous Solana validator debt is not enabled yet"
    );

    // Enable erroneous debt.
    test_setup
        .enable_erroneous_solana_validator_debt(bad_debt_dz_epoch)
        .await
        .unwrap();

    // Reclassify first debt as erroneous.
    test_setup
        .reclassify_bad_solana_validator_debt(
            bad_debt_dz_epoch,
            &debt_accountant_signer,
            &debt_1,
            debt_1_proof.clone(),
            true,
        )
        .await
        .unwrap();

    // Verify state after erroneous reclassification.
    let (_, distribution_after, remaining_data_after, _, _) =
        test_setup.fetch_distribution(bad_debt_dz_epoch).await;

    // Write-off bitmap is unchanged.
    let write_off_bitmap =
        &remaining_data_after[distribution_after.written_off_solana_validator_debt_bitmap_range()];
    assert_eq!(write_off_bitmap, [0b00100100]);

    let erroneous_bitmap_range = distribution_after
        .checked_erroneous_solana_validator_debt_bitmap_range()
        .unwrap();
    let erroneous_bitmap = &remaining_data_after[erroneous_bitmap_range];
    assert_eq!(erroneous_bitmap, [0b00000100]);
    assert_eq!(distribution_after.erroneous_sol_debt, debt_1.amount);

    let (_, deposit_after) = test_setup
        .fetch_solana_validator_deposit(&debt_1.node_id)
        .await;
    assert_eq!(deposit_after.erroneous_sol_debt, debt_1.amount);

    // Reclassify first debt as unpaid (undo erroneous classification).
    test_setup
        .reclassify_bad_solana_validator_debt(
            bad_debt_dz_epoch,
            &debt_accountant_signer,
            &debt_1,
            debt_1_proof.clone(),
            false,
        )
        .await
        .unwrap();

    // Verify state after unpaid reclassification.
    let (_, distribution_after_unpaid, remaining_data_after_unpaid, _, _) =
        test_setup.fetch_distribution(bad_debt_dz_epoch).await;

    // Write-off bitmap is unchanged.
    let write_off_bitmap = &remaining_data_after_unpaid
        [distribution_after_unpaid.written_off_solana_validator_debt_bitmap_range()];
    assert_eq!(write_off_bitmap, [0b00100100]);

    let erroneous_bitmap_range = distribution_after_unpaid
        .checked_erroneous_solana_validator_debt_bitmap_range()
        .unwrap();
    let erroneous_bitmap = &remaining_data_after_unpaid[erroneous_bitmap_range];
    assert_eq!(erroneous_bitmap, [0b00000000]);
    assert_eq!(distribution_after_unpaid.erroneous_sol_debt, 0);

    let (_, deposit_after_unpaid) = test_setup
        .fetch_solana_validator_deposit(&debt_1.node_id)
        .await;
    assert_eq!(deposit_after_unpaid.erroneous_sol_debt, 0);

    // Setup windfall distribution (do not sweep tokens).
    test_setup
        .initialize_distribution(&debt_accountant_signer)
        .await
        .unwrap()
        .warp_timestamp_by(60)
        .await
        .unwrap()
        .finalize_distribution_debt(windfall_dz_epoch, &debt_accountant_signer)
        .await
        .unwrap();

    // Transfer lamports to the first debt validator deposit so it can pay the
    // recovered debt.
    let (deposit_1_key, _) = SolanaValidatorDeposit::find_address(&debt_1.node_id);
    test_setup
        .transfer_lamports(&deposit_1_key, debt_1.amount)
        .await
        .unwrap();

    // Get journal state before recovery.
    let (_, journal_before, _) = test_setup.fetch_journal().await;

    // Recover first debt (before bad debt epoch is swept).
    test_setup
        .recover_bad_solana_validator_debt(
            bad_debt_dz_epoch,
            windfall_dz_epoch,
            &debt_accountant_signer,
            &debt_1,
            debt_1_proof.clone(),
        )
        .await
        .unwrap();

    // Verify bad debt distribution state after recovery.
    let (_, distribution_after_recover, remaining_data_after_recover, _, _) =
        test_setup.fetch_distribution(bad_debt_dz_epoch).await;

    let write_off_bitmap = &remaining_data_after_recover
        [distribution_after_recover.written_off_solana_validator_debt_bitmap_range()];
    assert_eq!(write_off_bitmap, [0b00100000]);
    assert_eq!(
        distribution_after_recover.solana_validator_debt_recovery_count,
        1
    );

    // Verify deposit state after recovery.
    let (_, deposit_after_recover) = test_setup
        .fetch_solana_validator_deposit(&debt_1.node_id)
        .await;
    assert_eq!(deposit_after_recover.recovered_sol_debt, debt_1.amount);

    // Verify journal received the lamports.
    let (_, journal_after, _) = test_setup.fetch_journal().await;
    assert_eq!(
        journal_after.total_sol_balance,
        journal_before.total_sol_balance + debt_1.amount
    );

    // Verify windfall distribution's recovered_sol_debt is updated.
    let (_, windfall_after, _, _, _) = test_setup.fetch_distribution(windfall_dz_epoch).await;
    assert_eq!(windfall_after.recovered_sol_debt, debt_1.amount);

    // Attempt to recover the same debt again (should fail because already
    // recovered).
    let recover_again_ix = try_build_instruction(
        &ID,
        RecoverBadSolanaValidatorDebtAccounts::new(
            &debt_accountant_signer.pubkey(),
            &debt_1.node_id,
            bad_debt_dz_epoch,
            windfall_dz_epoch,
        ),
        &RevenueDistributionInstructionData::RecoverBadSolanaValidatorDebt {
            amount: debt_1.amount,
            proof: debt_1_proof.clone(),
        },
    )
    .unwrap();

    let (tx_err, program_logs) = test_setup
        .unwrap_simulation_error(
            std::slice::from_ref(&recover_again_ix),
            &[&debt_accountant_signer],
        )
        .await;
    assert_eq!(
        tx_err,
        TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
    );
    assert_eq!(
        program_logs.get(4).unwrap(),
        &format!(
            "Program log: Solana validator debt was not written off for epoch {}",
            bad_debt_dz_epoch
        )
    );

    // Attempt to reclassify recovered debt as erroneous (should fail because no
    // longer written off).
    let reclassify_recovered_ix = try_build_instruction(
        &ID,
        ReclassifyBadSolanaValidatorDebtAccounts::new(
            &debt_accountant_signer.pubkey(),
            &debt_1.node_id,
            bad_debt_dz_epoch,
        ),
        &RevenueDistributionInstructionData::ReclassifyBadSolanaValidatorDebt {
            amount: debt_1.amount,
            proof: debt_1_proof.clone(),
            classify_erroneous: true,
        },
    )
    .unwrap();

    let (tx_err, program_logs) = test_setup
        .unwrap_simulation_error(
            std::slice::from_ref(&reclassify_recovered_ix),
            &[&debt_accountant_signer],
        )
        .await;
    assert_eq!(
        tx_err,
        TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
    );
    assert_eq!(
        program_logs.get(4).unwrap(),
        &format!(
            "Program log: Solana validator debt was not written off for epoch {}",
            bad_debt_dz_epoch
        )
    );

    // Sweep skipped epoch 0 and bad debt epoch 1.
    let sol_destination_key = Pubkey::new_unique();
    let paid_debt = total_solana_validator_debt - debt_1.amount - debt_2.amount;

    test_setup
        .mock_buy_sol(
            &src_token_account_key,
            &transfer_authority_signer,
            &sol_destination_key,
            expected_swept_2z_amount,
            paid_debt,
        )
        .await
        .unwrap()
        // Now sweep epoch 1.
        .finalize_distribution_rewards(bad_debt_dz_epoch)
        .await
        .unwrap()
        .sweep_distribution_tokens(bad_debt_dz_epoch)
        .await
        .unwrap();

    // Reclassify second debt as erroneous (after bad debt epoch is swept).
    test_setup
        .reclassify_bad_solana_validator_debt(
            bad_debt_dz_epoch,
            &debt_accountant_signer,
            &debt_2,
            debt_2_proof.clone(),
            true,
        )
        .await
        .unwrap();

    let (_, distribution_after_err, remaining_data_after_err, _, _) =
        test_setup.fetch_distribution(bad_debt_dz_epoch).await;

    // Write-off bitmap is unchanged.
    let write_off_bitmap = &remaining_data_after_err
        [distribution_after_err.written_off_solana_validator_debt_bitmap_range()];
    assert_eq!(write_off_bitmap, [0b00100000]);

    // Verify second debt is marked erroneous.
    let erroneous_bitmap_range = distribution_after_err
        .checked_erroneous_solana_validator_debt_bitmap_range()
        .unwrap();
    let erroneous_bitmap = &remaining_data_after_err[erroneous_bitmap_range];
    assert_eq!(erroneous_bitmap, [0b00100000]);
    assert_eq!(distribution_after_err.erroneous_sol_debt, debt_2.amount);

    // Attempt to recover second debt (should fail because it is erroneous).
    let (deposit_2_key, _) = SolanaValidatorDeposit::find_address(&debt_2.node_id);
    test_setup
        .transfer_lamports(&deposit_2_key, debt_2.amount)
        .await
        .unwrap();

    let resolve_bad_debt_ix = try_build_instruction(
        &ID,
        RecoverBadSolanaValidatorDebtAccounts::new(
            &debt_accountant_signer.pubkey(),
            &debt_2.node_id,
            bad_debt_dz_epoch,
            windfall_dz_epoch,
        ),
        &RevenueDistributionInstructionData::RecoverBadSolanaValidatorDebt {
            amount: debt_2.amount,
            proof: debt_2_proof.clone(),
        },
    )
    .unwrap();

    let (tx_err, program_logs) = test_setup
        .unwrap_simulation_error(
            std::slice::from_ref(&resolve_bad_debt_ix),
            &[&debt_accountant_signer],
        )
        .await;
    assert_eq!(
        tx_err,
        TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
    );
    assert_eq!(
        program_logs.get(4).unwrap(),
        &format!(
            "Program log: Cannot recover erroneous debt for epoch {}",
            bad_debt_dz_epoch
        )
    );

    // Reclassify second debt as unpaid.
    test_setup
        .reclassify_bad_solana_validator_debt(
            bad_debt_dz_epoch,
            &debt_accountant_signer,
            &debt_2,
            debt_2_proof.clone(),
            false,
        )
        .await
        .unwrap();

    let (_, distribution_after_unpaid_2, remaining_data_after_unpaid_2, _, _) =
        test_setup.fetch_distribution(bad_debt_dz_epoch).await;

    // Write-off bitmap is unchanged.
    let write_off_bitmap = &remaining_data_after_unpaid_2
        [distribution_after_unpaid_2.written_off_solana_validator_debt_bitmap_range()];
    assert_eq!(write_off_bitmap, [0b00100000]);

    // Verify erroneous flag is cleared.
    let erroneous_bitmap_range = distribution_after_unpaid_2
        .checked_erroneous_solana_validator_debt_bitmap_range()
        .unwrap();
    let erroneous_bitmap = &remaining_data_after_unpaid_2[erroneous_bitmap_range];
    assert_eq!(erroneous_bitmap, [0b00000000]);
    assert_eq!(distribution_after_unpaid_2.erroneous_sol_debt, 0);

    // Recover second debt (after bad debt epoch is swept).
    let (_, journal_before_2, _) = test_setup.fetch_journal().await;

    test_setup
        .recover_bad_solana_validator_debt(
            bad_debt_dz_epoch,
            windfall_dz_epoch,
            &debt_accountant_signer,
            &debt_2,
            debt_2_proof.clone(),
        )
        .await
        .unwrap();

    // Verify bad debt distribution state after recovery.
    let (_, distribution_after_recover_2, remaining_data_after_recover_2, _, _) =
        test_setup.fetch_distribution(bad_debt_dz_epoch).await;

    let write_off_bitmap = &remaining_data_after_recover_2
        [distribution_after_recover_2.written_off_solana_validator_debt_bitmap_range()];
    assert_eq!(write_off_bitmap, [0b00000000]);
    assert_eq!(
        distribution_after_recover_2.solana_validator_debt_recovery_count,
        2
    );

    // Verify deposit state after recovery.
    let (_, deposit_after_recover_2) = test_setup
        .fetch_solana_validator_deposit(&debt_2.node_id)
        .await;
    assert_eq!(deposit_after_recover_2.recovered_sol_debt, debt_2.amount);

    // Verify journal received the lamports.
    let (_, journal_after_2, _) = test_setup.fetch_journal().await;
    assert_eq!(
        journal_after_2.total_sol_balance,
        journal_before_2.total_sol_balance + debt_2.amount
    );

    // Verify windfall distribution's recovered_sol_debt is updated.
    let (_, windfall_after_2, _, _, _) = test_setup.fetch_distribution(windfall_dz_epoch).await;
    assert_eq!(
        windfall_after_2.recovered_sol_debt,
        debt_1.amount + debt_2.amount
    );
}
