mod common;

//

use doublezero_program_tools::zero_copy::checked_from_bytes_with_discriminator;
use doublezero_revenue_distribution::{
    instruction::{
        DistributionConfiguration, JournalConfiguration, ProgramConfiguration,
        ProgramFlagConfiguration,
    },
    state::{
        self, CommunityBurnRateParameters, Distribution, Journal, JournalEntries, JournalEntry,
        PrepaidConnection, ProgramConfig,
    },
    types::ValidatorFee,
    types::{BurnRate, DoubleZeroEpoch},
    DOUBLEZERO_MINT_KEY,
};
use solana_hash::Hash;
use solana_program_pack::Pack;
use solana_program_test::tokio;
use solana_pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use spl_token::state::{Account as TokenAccount, AccountState as SplTokenAccountState};

//
// Initialize program.
//

#[tokio::test]
async fn test_initialize_program() {
    let mut test_setup = common::start_test().await;

    test_setup.initialize_program().await.unwrap();

    let (program_config_key, program_config_bump) = ProgramConfig::find_address();

    let program_config_account_data = test_setup
        .banks_client
        .get_account(program_config_key)
        .await
        .unwrap()
        .unwrap()
        .data;

    let (program_config, remaining_data) =
        checked_from_bytes_with_discriminator::<ProgramConfig>(&program_config_account_data)
            .unwrap();
    assert!(remaining_data.is_empty());

    let mut expected_program_config = ProgramConfig::default();
    expected_program_config.bump_seed = program_config_bump;
    expected_program_config.reserve_2z_bump_seed =
        state::find_2z_token_pda_address(&program_config_key).1;
    expected_program_config.set_is_paused(true);
    assert_eq!(program_config, &expected_program_config);
}

//
// Set admin.
//

#[tokio::test]
async fn test_set_admin() {
    let mut test_setup = common::start_test().await;

    // Test input.

    let admin_signer = Keypair::new();

    test_setup
        .initialize_program()
        .await
        .unwrap()
        .set_admin(admin_signer.pubkey())
        .await
        .unwrap();

    let (program_config_key, program_config, _) = test_setup.fetch_program_config().await;

    let mut expected_program_config = ProgramConfig::default();
    expected_program_config.bump_seed = ProgramConfig::find_address().1;
    expected_program_config.reserve_2z_bump_seed =
        state::find_2z_token_pda_address(&program_config_key).1;
    expected_program_config.set_is_paused(true);
    expected_program_config.admin_key = admin_signer.pubkey();
    assert_eq!(program_config, expected_program_config);
}

//
// Configure program.
//

#[tokio::test]
async fn test_configure_program() {
    let mut test_setup = common::start_test().await;

    let admin_signer = Keypair::new();

    test_setup
        .initialize_program()
        .await
        .unwrap()
        .set_admin(admin_signer.pubkey())
        .await
        .unwrap();

    // Test inputs.

    // Flags.
    let should_pause = false;

    // Other settings.
    let accountant_key = Pubkey::new_unique();
    let sol_2z_swap_program_id = Pubkey::new_unique();

    // Distribution settings.
    let solana_validator_fee = 500; // 5%
    let calculation_grace_period_seconds = 6 * 60 * 60;

    // -- Community burn rate.
    let initial_cbr = 100_000_000; // 10%
    let cbr_limit = 500_000_000; // 50%
    let dz_epochs_to_increasing_cbr = 10;
    let dz_epochs_to_cbr_limit = 20;

    // Relay settings.
    let prepaid_connection_termination_relay_lamports = 8 * 6_960;

    test_setup
        .configure_program(
            [
                ProgramConfiguration::Flag(ProgramFlagConfiguration::IsPaused(should_pause)),
                ProgramConfiguration::Accountant(accountant_key),
                ProgramConfiguration::CalculationGracePeriodSeconds(
                    calculation_grace_period_seconds,
                ),
                ProgramConfiguration::Sol2zSwapProgram(sol_2z_swap_program_id),
                ProgramConfiguration::SolanaValidatorFee(solana_validator_fee),
                ProgramConfiguration::CommunityBurnRateParameters {
                    limit: cbr_limit,
                    dz_epochs_to_increasing: dz_epochs_to_increasing_cbr,
                    dz_epochs_to_limit: dz_epochs_to_cbr_limit,
                    initial_rate: Some(initial_cbr),
                },
                ProgramConfiguration::PrepaidConnectionTerminationRelayLamports(
                    prepaid_connection_termination_relay_lamports,
                ),
            ],
            &admin_signer,
        )
        .await
        .unwrap();

    let (program_config_key, program_config, _) = test_setup.fetch_program_config().await;

    let mut expected_program_config = ProgramConfig::default();
    expected_program_config.bump_seed = ProgramConfig::find_address().1;
    expected_program_config.reserve_2z_bump_seed =
        state::find_2z_token_pda_address(&program_config_key).1;
    expected_program_config.admin_key = admin_signer.pubkey();
    expected_program_config.set_is_paused(should_pause);
    expected_program_config.accountant_key = accountant_key;
    expected_program_config.sol_2z_swap_program_id = sol_2z_swap_program_id;

    let expected_distribution_params = &mut expected_program_config.distribution_parameters;
    expected_distribution_params.calculation_grace_period_seconds =
        calculation_grace_period_seconds;
    expected_distribution_params.current_solana_validator_fee =
        ValidatorFee::new(solana_validator_fee).unwrap();
    expected_distribution_params.community_burn_rate_parameters = CommunityBurnRateParameters::new(
        BurnRate::new(initial_cbr).unwrap(),
        BurnRate::new(cbr_limit).unwrap(),
        dz_epochs_to_increasing_cbr,
        dz_epochs_to_cbr_limit,
    )
    .unwrap();

    let expected_relay_params = &mut expected_program_config.relay_parameters;
    expected_relay_params.prepaid_connection_termination_lamports =
        prepaid_connection_termination_relay_lamports;
    assert_eq!(program_config, expected_program_config);
}

//
// Initialize journal.
//

#[tokio::test]
async fn test_initialize_journal() {
    let mut test_setup = common::start_test().await;

    test_setup.initialize_journal().await.unwrap();

    let journal_key = Journal::find_address().0;
    let journal_account_data = test_setup
        .banks_client
        .get_account(journal_key)
        .await
        .unwrap()
        .unwrap()
        .data;

    let (journal, remaining_data) =
        checked_from_bytes_with_discriminator::<Journal>(&journal_account_data).unwrap();

    let (journal_key, journal_bump) = Journal::find_address();

    let mut expected_journal = Journal::default();
    expected_journal.bump_seed = journal_bump;
    expected_journal.token_2z_pda_bump_seed = state::find_2z_token_pda_address(&journal_key).1;
    assert_eq!(journal, &expected_journal);

    let epoch_payments = Journal::checked_journal_entries(remaining_data).unwrap();
    assert!(epoch_payments.0.is_empty());

    let custodied_2z_token_account_data = test_setup
        .banks_client
        .get_account(state::find_2z_token_pda_address(&journal_key).0)
        .await
        .unwrap()
        .unwrap()
        .data;
    let custodied_2z_token_account =
        TokenAccount::unpack(&custodied_2z_token_account_data).unwrap();
    let expected_custodied_2z_token_account = TokenAccount {
        mint: DOUBLEZERO_MINT_KEY,
        owner: journal_key,
        state: SplTokenAccountState::Initialized,
        ..Default::default()
    };
    assert_eq!(
        custodied_2z_token_account,
        expected_custodied_2z_token_account
    );
}

//
// Configure journal.
//

#[tokio::test]
async fn test_configure_journal() {
    let mut test_setup = common::start_test().await;

    let admin_signer = Keypair::new();

    test_setup
        .initialize_program()
        .await
        .unwrap()
        .initialize_journal()
        .await
        .unwrap()
        .set_admin(admin_signer.pubkey())
        .await
        .unwrap();

    // Test inputs.

    // Prepaid connection settings.
    let prepaid_connection_activation_cost = 10_000;
    let prepaid_connection_cost_per_dz_epoch = 6_969;

    test_setup
        .configure_journal(
            [
                JournalConfiguration::ActivationCost(prepaid_connection_activation_cost),
                JournalConfiguration::CostPerDoubleZeroEpoch(prepaid_connection_cost_per_dz_epoch),
            ],
            &admin_signer,
        )
        .await
        .unwrap();

    let journal_key = Journal::find_address().0;
    let journal_account_data = test_setup
        .banks_client
        .get_account(journal_key)
        .await
        .unwrap()
        .unwrap()
        .data;

    let (journal, remaining_data) =
        checked_from_bytes_with_discriminator::<Journal>(&journal_account_data).unwrap();

    let (journal_key, journal_bump) = Journal::find_address();

    let mut expected_journal = Journal::default();
    expected_journal.bump_seed = journal_bump;
    expected_journal.token_2z_pda_bump_seed = state::find_2z_token_pda_address(&journal_key).1;

    let expected_prepaid_params = &mut expected_journal.prepaid_connection_parameters;
    expected_prepaid_params.activation_cost = prepaid_connection_activation_cost;
    expected_prepaid_params.cost_per_dz_epoch = prepaid_connection_cost_per_dz_epoch;
    assert_eq!(journal, &expected_journal);

    let epoch_payments = Journal::checked_journal_entries(remaining_data).unwrap();
    assert!(epoch_payments.0.is_empty());

    let custodied_2z_token_account_data = test_setup
        .banks_client
        .get_account(state::find_2z_token_pda_address(&journal_key).0)
        .await
        .unwrap()
        .unwrap()
        .data;
    let custodied_2z_token_account =
        TokenAccount::unpack(&custodied_2z_token_account_data).unwrap();
    let expected_custodied_2z_token_account = TokenAccount {
        mint: DOUBLEZERO_MINT_KEY,
        owner: journal_key,
        state: SplTokenAccountState::Initialized,
        ..Default::default()
    };
    assert_eq!(
        custodied_2z_token_account,
        expected_custodied_2z_token_account
    );
}

//
// Initialize distribution.
//

#[tokio::test]
async fn test_initialize_distribution() {
    let mut test_setup = common::start_test().await;

    let admin_signer = Keypair::new();

    let accountant_signer = Keypair::new();
    let solana_validator_fee = 500; // 5%

    // Community burn rate.
    let initial_cbr = 100_000_000; // 10%
    let cbr_limit = 500_000_000; // 50%
    let dz_epochs_to_increasing_cbr = 1;
    let dz_epochs_to_cbr_limit = 20;

    test_setup
        .initialize_program()
        .await
        .unwrap()
        .initialize_journal()
        .await
        .unwrap()
        .set_admin(admin_signer.pubkey())
        .await
        .unwrap()
        .configure_program(
            [
                ProgramConfiguration::Accountant(accountant_signer.pubkey()),
                ProgramConfiguration::SolanaValidatorFee(solana_validator_fee),
                ProgramConfiguration::CommunityBurnRateParameters {
                    limit: cbr_limit,
                    dz_epochs_to_increasing: dz_epochs_to_increasing_cbr,
                    dz_epochs_to_limit: dz_epochs_to_cbr_limit,
                    initial_rate: Some(initial_cbr),
                },
                ProgramConfiguration::Flag(ProgramFlagConfiguration::IsPaused(false)),
            ],
            &admin_signer,
        )
        .await
        .unwrap()
        .initialize_distribution(&accountant_signer)
        .await
        .unwrap();

    let mut cbr_params = CommunityBurnRateParameters::new(
        BurnRate::new(initial_cbr).unwrap(),
        BurnRate::new(cbr_limit).unwrap(),
        dz_epochs_to_increasing_cbr,
        dz_epochs_to_cbr_limit,
    )
    .unwrap();

    // Sync community burn rate.
    let expected_cbr = cbr_params.checked_compute().unwrap();
    assert_eq!(expected_cbr, BurnRate::new(100_000_000).unwrap());
    assert_eq!(
        cbr_params.next_burn_rate().unwrap(),
        BurnRate::new(120_000_000).unwrap()
    );

    let dz_epoch = DoubleZeroEpoch::new(0);
    let (distribution_key, distribution, distribution_custody) =
        test_setup.fetch_distribution(dz_epoch).await;

    let mut expected_distribution = Distribution::default();
    expected_distribution.bump_seed = Distribution::find_address(dz_epoch).1;
    expected_distribution.token_2z_pda_bump_seed =
        state::find_2z_token_pda_address(&distribution_key).1;
    expected_distribution.dz_epoch = dz_epoch;
    expected_distribution.community_burn_rate = expected_cbr;
    assert_eq!(distribution, expected_distribution);
    assert_eq!(distribution_custody.amount, 0);

    let (program_config_key, program_config, _) = test_setup.fetch_program_config().await;

    let mut expected_program_config = ProgramConfig::default();
    expected_program_config.bump_seed = ProgramConfig::find_address().1;
    expected_program_config.reserve_2z_bump_seed =
        state::find_2z_token_pda_address(&program_config_key).1;
    expected_program_config.admin_key = admin_signer.pubkey();
    expected_program_config.next_dz_epoch = DoubleZeroEpoch::new(1);
    expected_program_config.accountant_key = accountant_signer.pubkey();

    let expected_distribution_params = &mut expected_program_config.distribution_parameters;
    expected_distribution_params.current_solana_validator_fee =
        ValidatorFee::new(solana_validator_fee).unwrap();
    expected_distribution_params.community_burn_rate_parameters = cbr_params;
    assert_eq!(program_config, expected_program_config);

    // Create another distribution.

    test_setup
        .initialize_distribution(&accountant_signer)
        .await
        .unwrap();

    // Sync community burn rate.
    let expected_cbr = cbr_params.checked_compute().unwrap();
    assert_eq!(expected_cbr, BurnRate::new(120_000_000).unwrap());
    assert_eq!(
        cbr_params.next_burn_rate().unwrap(),
        BurnRate::new(140_000_000).unwrap()
    );

    let dz_epoch = DoubleZeroEpoch::new(1);
    let (distribution_key, distribution, distribution_custody) =
        test_setup.fetch_distribution(dz_epoch).await;

    let mut expected_distribution = Distribution::default();
    expected_distribution.bump_seed = Distribution::find_address(dz_epoch).1;
    expected_distribution.token_2z_pda_bump_seed =
        state::find_2z_token_pda_address(&distribution_key).1;
    expected_distribution.dz_epoch = dz_epoch;
    expected_distribution.community_burn_rate = expected_cbr;
    assert_eq!(distribution, expected_distribution);
    assert_eq!(distribution_custody.amount, 0);

    let (program_config_key, program_config, _) = test_setup.fetch_program_config().await;

    let mut expected_program_config = ProgramConfig::default();
    expected_program_config.bump_seed = ProgramConfig::find_address().1;
    expected_program_config.reserve_2z_bump_seed =
        state::find_2z_token_pda_address(&program_config_key).1;
    expected_program_config.admin_key = admin_signer.pubkey();
    expected_program_config.next_dz_epoch = DoubleZeroEpoch::new(2);
    expected_program_config.accountant_key = accountant_signer.pubkey();

    let expected_distribution_params = &mut expected_program_config.distribution_parameters;
    expected_distribution_params.current_solana_validator_fee =
        ValidatorFee::new(solana_validator_fee).unwrap();
    expected_distribution_params.community_burn_rate_parameters = cbr_params;
    assert_eq!(program_config, expected_program_config);
}

//
// Configure distribution.
//

#[tokio::test]
async fn test_configure_distribution() {
    let mut test_setup = common::start_test().await;

    let admin_signer = Keypair::new();

    let accountant_signer = Keypair::new();
    let solana_validator_fee = 500; // 5%

    // Community burn rate.
    let initial_cbr = 100_000_000; // 10%
    let cbr_limit = 500_000_000; // 50%
    let dz_epochs_to_increasing_cbr = 10;
    let dz_epochs_to_cbr_limit = 20;

    // Test inputs.

    let dz_epoch = DoubleZeroEpoch::new(1);

    let total_solana_validator_payments_owed = 100_000_000_000; // 100 SOL
    let solana_validator_payments_merkle_root = Hash::new_unique();

    let total_contributors = 69;
    let contributor_rewards_merkle_root = Hash::new_unique();

    test_setup
        .initialize_program()
        .await
        .unwrap()
        .initialize_journal()
        .await
        .unwrap()
        .set_admin(admin_signer.pubkey())
        .await
        .unwrap()
        .configure_program(
            [
                ProgramConfiguration::Accountant(accountant_signer.pubkey()),
                ProgramConfiguration::SolanaValidatorFee(solana_validator_fee),
                ProgramConfiguration::CommunityBurnRateParameters {
                    limit: cbr_limit,
                    dz_epochs_to_increasing: dz_epochs_to_increasing_cbr,
                    dz_epochs_to_limit: dz_epochs_to_cbr_limit,
                    initial_rate: Some(initial_cbr),
                },
                ProgramConfiguration::Flag(ProgramFlagConfiguration::IsPaused(false)),
            ],
            &admin_signer,
        )
        .await
        .unwrap()
        .initialize_distribution(&accountant_signer)
        .await
        .unwrap()
        .initialize_distribution(&accountant_signer)
        .await
        .unwrap()
        .configure_distribution(
            dz_epoch,
            [
                DistributionConfiguration::SolanaValidatorPayments {
                    total_lamports_owed: total_solana_validator_payments_owed,
                    merkle_root: solana_validator_payments_merkle_root,
                },
                DistributionConfiguration::ContributorRewards {
                    total_contributors,
                    merkle_root: contributor_rewards_merkle_root,
                },
            ],
            &accountant_signer,
        )
        .await
        .unwrap();

    let (distribution_key, distribution, _) = test_setup.fetch_distribution(dz_epoch).await;

    let mut expected_distribution = Distribution::default();
    expected_distribution.bump_seed = Distribution::find_address(dz_epoch).1;
    expected_distribution.token_2z_pda_bump_seed =
        state::find_2z_token_pda_address(&distribution_key).1;
    expected_distribution.dz_epoch = dz_epoch;
    expected_distribution.community_burn_rate = BurnRate::new(initial_cbr).unwrap();
    expected_distribution.total_solana_validator_payments_owed =
        total_solana_validator_payments_owed;
    expected_distribution.solana_validator_payments_merkle_root =
        solana_validator_payments_merkle_root;
    expected_distribution.total_contributors = total_contributors;
    expected_distribution.contributor_rewards_merkle_root = contributor_rewards_merkle_root;
    assert_eq!(distribution, expected_distribution);
}

//
// Initialize prepaid connection.
//

#[tokio::test]
async fn test_initialize_prepaid_connection() {
    let burn_authority_signer = Keypair::new();

    let bootstrapped_accounts = common::generate_token_accounts_for_test(
        &DOUBLEZERO_MINT_KEY,
        &[burn_authority_signer.pubkey()],
    );
    let source_token_account_key = bootstrapped_accounts.first().unwrap().key;

    let mut test_setup = common::start_test_with_accounts(bootstrapped_accounts).await;

    let admin_signer = Keypair::new();

    // Prepaid connection settings.
    let prepaid_connection_activation_cost = 20_000;

    let expected_activation_cost = u64::from(prepaid_connection_activation_cost) * u64::pow(10, 8);

    test_setup
        .transfer_2z(&source_token_account_key, expected_activation_cost)
        .await
        .unwrap()
        .initialize_program()
        .await
        .unwrap()
        .initialize_journal()
        .await
        .unwrap()
        .set_admin(admin_signer.pubkey())
        .await
        .unwrap()
        .configure_journal(
            [JournalConfiguration::ActivationCost(
                prepaid_connection_activation_cost,
            )],
            &admin_signer,
        )
        .await
        .unwrap();

    let src_balance = test_setup
        .fetch_token_account(&source_token_account_key)
        .await
        .unwrap()
        .amount;
    assert_eq!(src_balance, expected_activation_cost);

    // Test inputs.

    let user_key = Pubkey::new_unique();

    test_setup
        .initialize_prepaid_connection(
            &burn_authority_signer,
            &source_token_account_key,
            &user_key,
            8,
        )
        .await
        .unwrap();

    // Activation fee must have been transferred from the source token account.
    let src_balance = test_setup
        .fetch_token_account(&source_token_account_key)
        .await
        .unwrap()
        .amount;
    assert_eq!(src_balance, 0);

    // Did the tokens arrive in the reserve account?
    let (_, _, reserve_2z) = test_setup.fetch_program_config().await;
    assert_eq!(reserve_2z.amount, expected_activation_cost);

    let (_, prepaid_connection) = test_setup.fetch_prepaid_connection(&user_key).await;

    let mut expected_prepaid_connection = PrepaidConnection::default();
    expected_prepaid_connection.user_key = user_key;
    expected_prepaid_connection.termination_beneficiary_key = test_setup.payer_signer.pubkey();
    assert_eq!(prepaid_connection, expected_prepaid_connection);
}

//
// Load prepaid connection.
//

#[tokio::test]
async fn test_load_prepaid_connection() {
    let burn_authority_signer = Keypair::new();

    let bootstrapped_accounts = common::generate_token_accounts_for_test(
        &DOUBLEZERO_MINT_KEY,
        &[burn_authority_signer.pubkey()],
    );
    let source_token_account_key = bootstrapped_accounts.first().unwrap().key;

    let mut test_setup = common::start_test_with_accounts(bootstrapped_accounts).await;

    let admin_signer = Keypair::new();

    // Prepaid connection settings.
    let prepaid_activation_cost = 20_000;
    let prepaid_cost_per_dz_epoch = 10_000;

    let prepaid_minimum_prepaid_dz_epochs = 1;
    let prepaid_maximum_entries = 10;

    let user_1_key = Pubkey::new_unique();
    let user_2_key = Pubkey::new_unique();
    let user_3_key = Pubkey::new_unique();

    test_setup
        .transfer_2z(&source_token_account_key, 1_000_000 * u64::pow(10, 8))
        .await
        .unwrap()
        .initialize_program()
        .await
        .unwrap()
        .initialize_journal()
        .await
        .unwrap()
        .set_admin(admin_signer.pubkey())
        .await
        .unwrap()
        .configure_journal(
            [
                JournalConfiguration::ActivationCost(prepaid_activation_cost),
                JournalConfiguration::CostPerDoubleZeroEpoch(prepaid_cost_per_dz_epoch),
                JournalConfiguration::EntryBoundaries {
                    minimum_prepaid_dz_epochs: prepaid_minimum_prepaid_dz_epochs,
                    maximum_entries: prepaid_maximum_entries,
                },
            ],
            &admin_signer,
        )
        .await
        .unwrap();

    for user_key in &[user_1_key, user_2_key, user_3_key] {
        test_setup
            .initialize_prepaid_connection(
                &burn_authority_signer,
                &source_token_account_key,
                user_key,
                8,
            )
            .await
            .unwrap();
    }

    // Test input
    let starting_src_balance = test_setup
        .fetch_token_account(&source_token_account_key)
        .await
        .unwrap()
        .amount;

    // Test inputs.

    let valid_through_dz_epoch = DoubleZeroEpoch::new(5);

    test_setup
        .load_prepaid_connection(
            &burn_authority_signer,
            &source_token_account_key,
            &user_1_key,
            valid_through_dz_epoch,
            8,
        )
        .await
        .unwrap();

    let ending_src_balance = test_setup
        .fetch_token_account(&source_token_account_key)
        .await
        .unwrap()
        .amount;

    // Compute the total cost. Because global DZ epoch is 0, we needed to have paid for 6 epochs.
    let expected_total_payment = 6 * u64::from(prepaid_cost_per_dz_epoch) * u64::pow(10, 8);
    assert_eq!(
        starting_src_balance - ending_src_balance,
        expected_total_payment
    );

    let (_, prepaid_connection) = test_setup.fetch_prepaid_connection(&user_1_key).await;

    let mut expected_prepaid_connection_1 = PrepaidConnection::default();
    expected_prepaid_connection_1.user_key = user_1_key;
    expected_prepaid_connection_1.set_has_paid(true);
    expected_prepaid_connection_1.valid_through_dz_epoch = valid_through_dz_epoch;
    expected_prepaid_connection_1.termination_beneficiary_key = test_setup.payer_signer.pubkey();
    assert_eq!(prepaid_connection, expected_prepaid_connection_1);

    let (_, journal, journal_entries, journal_2z_pda) = test_setup.fetch_journal().await;

    let total_journal_balance = expected_total_payment;
    assert_eq!(journal.total_2z_balance, total_journal_balance);
    assert_eq!(journal_2z_pda.amount, total_journal_balance);

    let expected_journal_entries = JournalEntries(
        vec![
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(0),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(1),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(2),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(3),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(4),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(5),
                amount: prepaid_cost_per_dz_epoch,
            },
        ]
        .into(),
    );
    assert_eq!(journal_entries, expected_journal_entries);

    // Load again.

    let starting_src_balance = ending_src_balance;
    let last_journal_balance = total_journal_balance;

    let valid_through_dz_epoch = DoubleZeroEpoch::new(7);

    test_setup
        .load_prepaid_connection(
            &burn_authority_signer,
            &source_token_account_key,
            &user_1_key,
            valid_through_dz_epoch,
            8,
        )
        .await
        .unwrap();

    let ending_src_balance = test_setup
        .fetch_token_account(&source_token_account_key)
        .await
        .unwrap()
        .amount;

    // Compute the total cost. Because we have already paid through DZ epoch 5, we needed to have
    // paid for 2 more epochs.
    let expected_total_payment = 2 * u64::from(prepaid_cost_per_dz_epoch) * u64::pow(10, 8);
    assert_eq!(
        starting_src_balance - ending_src_balance,
        expected_total_payment
    );

    let (_, prepaid_connection) = test_setup.fetch_prepaid_connection(&user_1_key).await;
    expected_prepaid_connection_1.valid_through_dz_epoch = valid_through_dz_epoch;
    assert_eq!(prepaid_connection, expected_prepaid_connection_1);

    let (_, journal, journal_entries, journal_2z_pda) = test_setup.fetch_journal().await;

    let total_journal_balance = last_journal_balance + expected_total_payment;
    assert_eq!(journal.total_2z_balance, total_journal_balance);
    assert_eq!(journal_2z_pda.amount, total_journal_balance);

    let expected_journal_entries = JournalEntries(
        vec![
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(0),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(1),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(2),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(3),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(4),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(5),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(6),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(7),
                amount: prepaid_cost_per_dz_epoch,
            },
        ]
        .into(),
    );
    assert_eq!(journal_entries, expected_journal_entries);

    // Load another user.

    let starting_src_balance = ending_src_balance;
    let last_journal_balance = total_journal_balance;

    let valid_through_dz_epoch = DoubleZeroEpoch::new(3);

    test_setup
        .load_prepaid_connection(
            &burn_authority_signer,
            &source_token_account_key,
            &user_2_key,
            valid_through_dz_epoch,
            8,
        )
        .await
        .unwrap();

    let ending_src_balance = test_setup
        .fetch_token_account(&source_token_account_key)
        .await
        .unwrap()
        .amount;

    // Compute the total cost. Because global DZ epoch is 0, we needed to have paid for 4 epochs.
    let expected_total_payment = 4 * u64::from(prepaid_cost_per_dz_epoch) * u64::pow(10, 8);
    assert_eq!(
        starting_src_balance - ending_src_balance,
        expected_total_payment
    );

    let (_, prepaid_connection) = test_setup.fetch_prepaid_connection(&user_2_key).await;

    let mut expected_prepaid_connection_2 = PrepaidConnection::default();
    expected_prepaid_connection_2.user_key = user_2_key;
    expected_prepaid_connection_2.set_has_paid(true);
    expected_prepaid_connection_2.valid_through_dz_epoch = valid_through_dz_epoch;
    expected_prepaid_connection_2.termination_beneficiary_key = test_setup.payer_signer.pubkey();
    assert_eq!(prepaid_connection, expected_prepaid_connection_2);

    let (_, journal, journal_entries, journal_2z_pda) = test_setup.fetch_journal().await;

    let total_journal_balance = last_journal_balance + expected_total_payment;
    assert_eq!(journal.total_2z_balance, total_journal_balance);
    assert_eq!(journal_2z_pda.amount, total_journal_balance);

    let expected_journal_entries = JournalEntries(
        vec![
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(0),
                amount: 2 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(1),
                amount: 2 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(2),
                amount: 2 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(3),
                amount: 2 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(4),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(5),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(6),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(7),
                amount: prepaid_cost_per_dz_epoch,
            },
        ]
        .into(),
    );
    assert_eq!(journal_entries, expected_journal_entries);

    // Initialize new distribution.

    let last_journal_balance = total_journal_balance;

    let accountant_signer = Keypair::new();
    let solana_validator_fee = 500; // 5%

    // Community burn rate.
    let initial_cbr = 100_000_000; // 10%
    let cbr_limit = 500_000_000; // 50%
    let dz_epochs_to_increasing_cbr = 10;
    let dz_epochs_to_cbr_limit = 20;

    test_setup
        .configure_program(
            [
                ProgramConfiguration::Accountant(accountant_signer.pubkey()),
                ProgramConfiguration::SolanaValidatorFee(solana_validator_fee),
                ProgramConfiguration::CommunityBurnRateParameters {
                    limit: cbr_limit,
                    dz_epochs_to_increasing: dz_epochs_to_increasing_cbr,
                    dz_epochs_to_limit: dz_epochs_to_cbr_limit,
                    initial_rate: Some(initial_cbr),
                },
                ProgramConfiguration::Flag(ProgramFlagConfiguration::IsPaused(false)),
            ],
            &admin_signer,
        )
        .await
        .unwrap()
        .initialize_distribution(&accountant_signer)
        .await
        .unwrap();

    let expected_journal_entry_amount = 2 * prepaid_cost_per_dz_epoch;
    assert_eq!(
        journal_entries.front_entry().unwrap().amount,
        expected_journal_entry_amount
    );

    let expected_transfer_amount = u64::from(expected_journal_entry_amount) * u64::pow(10, 8);

    let (_, _, distribution_2z_token_pda) =
        test_setup.fetch_distribution(DoubleZeroEpoch::new(0)).await;
    assert_eq!(distribution_2z_token_pda.amount, expected_transfer_amount);

    let (_, journal, journal_entries, journal_2z_pda) = test_setup.fetch_journal().await;

    // The balance on the journal should change by the first entry's amount.

    let total_journal_balance = last_journal_balance - expected_transfer_amount;
    assert_eq!(journal.total_2z_balance, total_journal_balance);
    assert_eq!(journal_2z_pda.amount, total_journal_balance);

    let expected_journal_entries = JournalEntries(
        vec![
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(1),
                amount: 2 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(2),
                amount: 2 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(3),
                amount: 2 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(4),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(5),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(6),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(7),
                amount: prepaid_cost_per_dz_epoch,
            },
        ]
        .into(),
    );
    assert_eq!(journal_entries, expected_journal_entries);

    // Load another user and initialize another distribution.

    let starting_src_balance = ending_src_balance;
    let last_journal_balance = total_journal_balance;

    // NOTE: This should be the maximum a user can load.
    let valid_through_dz_epoch = DoubleZeroEpoch::new(11);

    test_setup
        .load_prepaid_connection(
            &burn_authority_signer,
            &source_token_account_key,
            &user_3_key,
            valid_through_dz_epoch,
            8,
        )
        .await
        .unwrap()
        .initialize_distribution(&accountant_signer)
        .await
        .unwrap();

    let ending_src_balance = test_setup
        .fetch_token_account(&source_token_account_key)
        .await
        .unwrap()
        .amount;

    // Compute the total cost. Because global DZ epoch is 1, we needed to have paid for 11 epochs.
    let expected_total_payment = 11 * u64::from(prepaid_cost_per_dz_epoch) * u64::pow(10, 8);
    assert_eq!(
        starting_src_balance - ending_src_balance,
        expected_total_payment
    );

    let (_, prepaid_connection) = test_setup.fetch_prepaid_connection(&user_3_key).await;

    let mut expected_prepaid_connection_2 = PrepaidConnection::default();
    expected_prepaid_connection_2.user_key = user_3_key;
    expected_prepaid_connection_2.set_has_paid(true);
    expected_prepaid_connection_2.valid_through_dz_epoch = valid_through_dz_epoch;
    expected_prepaid_connection_2.termination_beneficiary_key = test_setup.payer_signer.pubkey();
    assert_eq!(prepaid_connection, expected_prepaid_connection_2);

    let expected_journal_entry_amount = 3 * prepaid_cost_per_dz_epoch;
    assert_eq!(
        journal_entries.front_entry().unwrap().amount + prepaid_cost_per_dz_epoch,
        expected_journal_entry_amount
    );

    let expected_transfer_amount = u64::from(expected_journal_entry_amount) * u64::pow(10, 8);

    let (_, _, distribution_2z_token_pda) =
        test_setup.fetch_distribution(DoubleZeroEpoch::new(1)).await;
    assert_eq!(distribution_2z_token_pda.amount, expected_transfer_amount);

    let (_, journal, journal_entries, journal_2z_pda) = test_setup.fetch_journal().await;

    // The balance on the journal should change by the first entry's amount.

    let total_journal_balance =
        last_journal_balance + expected_total_payment - expected_transfer_amount;
    assert_eq!(journal.total_2z_balance, total_journal_balance);
    assert_eq!(journal_2z_pda.amount, total_journal_balance);

    let expected_journal_entries = JournalEntries(
        vec![
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(2),
                amount: 3 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(3),
                amount: 3 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(4),
                amount: 2 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(5),
                amount: 2 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(6),
                amount: 2 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(7),
                amount: 2 * prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(8),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(9),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(10),
                amount: prepaid_cost_per_dz_epoch,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(11),
                amount: prepaid_cost_per_dz_epoch,
            },
        ]
        .into(),
    );
    assert_eq!(journal_entries, expected_journal_entries);
}

//
// Terminate prepaid connection.
//

#[tokio::test]
#[ignore]
async fn test_terminate_prepaid_connection() {
    let burn_authority_signer = Keypair::new();

    let bootstrapped_accounts = common::generate_token_accounts_for_test(
        &DOUBLEZERO_MINT_KEY,
        &[burn_authority_signer.pubkey()],
    );
    let source_token_account_key = bootstrapped_accounts.first().unwrap().key;

    let mut test_setup = common::start_test_with_accounts(bootstrapped_accounts).await;

    let admin_signer = Keypair::new();

    let user_key = Pubkey::new_unique();
    let prepaid_connection_termination_relay_lamports = 20_000;

    test_setup
        .initialize_program()
        .await
        .unwrap()
        .initialize_prepaid_connection(
            &burn_authority_signer,
            &source_token_account_key,
            &user_key,
            8,
        )
        .await
        .unwrap()
        .set_admin(admin_signer.pubkey())
        .await
        .unwrap()
        .configure_program(
            [
                ProgramConfiguration::PrepaidConnectionTerminationRelayLamports(
                    prepaid_connection_termination_relay_lamports,
                ),
            ],
            &admin_signer,
        )
        .await
        .unwrap()
        .configure_program(
            [ProgramConfiguration::Flag(
                ProgramFlagConfiguration::IsPaused(false),
            )],
            &admin_signer,
        )
        .await
        .unwrap();

    // Test inputs.

    let termination_relayer_key = Pubkey::new_unique();

    test_setup
        .terminate_prepaid_connection(
            &user_key,
            &test_setup.payer_signer.pubkey(),
            Some(&termination_relayer_key),
        )
        .await
        .unwrap();

    // let prepaid_connection_key = PrepaidConnection::find_address(&user_key).0;
    // let prepaid_connection_account_data = test_setup
    //     .banks_client
    //     .get_account(prepaid_connection_key)
    //     .await
    //     .unwrap()
    //     .unwrap()
    //     .data;
    //
    // let (prepaid_connection, remaining_data) = checked_from_bytes_with_discriminator::<
    //     PrepaidConnection,
    // >(&prepaid_connection_account_data)
    // .unwrap();
    //
    // let mut expected_prepaid_connection = PrepaidConnection::default();
    // expected_prepaid_connection.user_key = user_key;
    // expected_prepaid_connection.termination_beneficiary_key = test_setup.payer_signer.pubkey();
    // assert_eq!(prepaid_connection, &expected_prepaid_connection);
}
