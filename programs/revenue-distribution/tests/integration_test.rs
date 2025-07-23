mod common;

use doublezero_program_tools::zero_copy::checked_from_bytes_with_discriminator;
use doublezero_revenue_distribution::instruction::JournalConfiguration;
use doublezero_revenue_distribution::state::PrepaidConnection;
use doublezero_revenue_distribution::{
    instruction::{DistributionConfiguration, ProgramConfiguration, ProgramFlagConfiguration},
    state::{self, CommunityBurnRateParameters, Distribution, Journal, ProgramConfig},
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

    let (program_config_key, program_config) = test_setup.fetch_program_config().await;

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

    let (program_config_key, program_config) = test_setup.fetch_program_config().await;

    let mut expected_program_config = ProgramConfig::default();
    expected_program_config.bump_seed = ProgramConfig::find_address().1;
    expected_program_config.reserve_2z_bump_seed =
        state::find_2z_token_pda_address(&program_config_key).1;
    expected_program_config.admin_key = admin_signer.pubkey();
    expected_program_config.set_is_paused(should_pause);
    expected_program_config.accountant_key = accountant_key;
    expected_program_config.sol_2z_swap_program_id = sol_2z_swap_program_id;

    let distribution_parameters = &mut expected_program_config.distribution_parameters;
    distribution_parameters.calculation_grace_period_seconds = calculation_grace_period_seconds;
    distribution_parameters.current_solana_validator_fee =
        ValidatorFee::new(solana_validator_fee).unwrap();
    distribution_parameters.community_burn_rate_parameters = CommunityBurnRateParameters::new(
        BurnRate::new(initial_cbr).unwrap(),
        BurnRate::new(cbr_limit).unwrap(),
        dz_epochs_to_increasing_cbr,
        dz_epochs_to_cbr_limit,
    )
    .unwrap();

    let relay_parameters = &mut expected_program_config.relay_parameters;
    relay_parameters.prepaid_connection_termination_lamports =
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
            false, // use_payer
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
    expected_journal.activation_cost = prepaid_connection_activation_cost;
    expected_journal.cost_per_dz_epoch = prepaid_connection_cost_per_dz_epoch;
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

    let (program_config_key, program_config) = test_setup.fetch_program_config().await;

    let mut expected_program_config = ProgramConfig::default();
    expected_program_config.bump_seed = ProgramConfig::find_address().1;
    expected_program_config.reserve_2z_bump_seed =
        state::find_2z_token_pda_address(&program_config_key).1;
    expected_program_config.admin_key = admin_signer.pubkey();
    expected_program_config.next_dz_epoch = DoubleZeroEpoch::new(1);
    expected_program_config.accountant_key = accountant_signer.pubkey();

    let distribution_parameters = &mut expected_program_config.distribution_parameters;
    distribution_parameters.current_solana_validator_fee =
        ValidatorFee::new(solana_validator_fee).unwrap();
    distribution_parameters.community_burn_rate_parameters = cbr_params;
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

    let (program_config_key, program_config) = test_setup.fetch_program_config().await;

    let mut expected_program_config = ProgramConfig::default();
    expected_program_config.bump_seed = ProgramConfig::find_address().1;
    expected_program_config.reserve_2z_bump_seed =
        state::find_2z_token_pda_address(&program_config_key).1;
    expected_program_config.admin_key = admin_signer.pubkey();
    expected_program_config.next_dz_epoch = DoubleZeroEpoch::new(2);
    expected_program_config.accountant_key = accountant_signer.pubkey();

    let distribution_parameters = &mut expected_program_config.distribution_parameters;
    distribution_parameters.current_solana_validator_fee =
        ValidatorFee::new(solana_validator_fee).unwrap();
    distribution_parameters.community_burn_rate_parameters = cbr_params;
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
                    total_owed: total_solana_validator_payments_owed,
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
            false, // use_payer
        )
        .await
        .unwrap();

    let balance_before = test_setup
        .fetch_token_account(&source_token_account_key)
        .await
        .unwrap()
        .amount;
    assert_eq!(balance_before, expected_activation_cost);

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

    // Activation fee must have been burned from the source token account.
    let balance_after = test_setup
        .fetch_token_account(&source_token_account_key)
        .await
        .unwrap()
        .amount;
    assert_eq!(balance_after, 0);

    let prepaid_connection_key = PrepaidConnection::find_address(&user_key).0;
    let prepaid_connection_account_data = test_setup
        .banks_client
        .get_account(prepaid_connection_key)
        .await
        .unwrap()
        .unwrap()
        .data;

    let (prepaid_connection, remaining_data) = checked_from_bytes_with_discriminator::<
        PrepaidConnection,
    >(&prepaid_connection_account_data)
    .unwrap();

    let mut expected_prepaid_connection = PrepaidConnection::default();
    expected_prepaid_connection.user_key = user_key;
    expected_prepaid_connection.termination_beneficiary_key = test_setup.payer_signer.pubkey();
    assert_eq!(prepaid_connection, &expected_prepaid_connection);
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
