use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};
use doublezero_passport::{
    instruction::{self as passport_instruction, PassportInstructionData},
    state as passport_state, ID as PASSPORT_PROGRAM_ID,
};
use doublezero_program_tools::{get_program_data_address, instruction::try_build_instruction};
use doublezero_revenue_distribution::{
    instruction::{self as revenue_distribution_instruction, RevenueDistributionInstructionData},
    state as revenue_distribution_state, ID as REVENUE_DISTRIBUTION_PROGRAM_ID,
};
use solana_sdk::{compute_budget::ComputeBudgetInstruction, pubkey::Pubkey};

use crate::payer::{SolanaPayerOptions, Wallet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Program {
    Passport,
    RevenueDistribution,
}

#[derive(Debug, Args)]
pub struct AdminCliCommand {
    #[command(subcommand)]
    pub command: AdminSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum AdminSubCommand {
    /// Configure the journal account. Only the administrator can execute this command.
    ConfigureJournal {
        /// Activation cost for a prepaid connection.
        #[arg(long)]
        activation_cost: Option<u32>,

        /// Cost per DoubleZero epoch for a prepaid connection.
        #[arg(long)]
        cost_per_epoch: Option<u32>,

        #[command(flatten)]
        payer_options: SolanaPayerOptions,
    },

    /// Configure the program. Only the administrator can execute this command.
    ConfigureProgram {
        // Flags.
        //
        /// Whether to pause the program. Cannot be used with --unpause.
        #[arg(long)]
        pause: Option<bool>,

        /// Whether to unpause the program. Cannot be used with --pause.
        #[arg(long)]
        unpause: Option<bool>,

        // Other configuration.
        //
        /// Set the accountant key.
        #[arg(long)]
        accountant_key: Option<Pubkey>,

        /// Set the SOL/2Z Swap program ID.
        #[arg(long)]
        sol_2z_swap_program_id: Option<Pubkey>,

        /// Solana validator fee percentage (max: 100%).
        #[arg(long)]
        solana_validator_fee_percentage: Option<String>,

        /// How long the accountant must wait to fetch telemetry data for reward calculations.
        #[arg(long)]
        calculation_grace_period_seconds: Option<u32>,

        /// Amount to pay relayer to terminate a prepaid connection.
        #[arg(long)]
        prepaid_connection_termination_relay_lamports: Option<u32>,

        //
        #[command(flatten)]
        payer_options: SolanaPayerOptions,
    },

    /// Initialize program config and journal. Also set admin to yourself.
    Initialize {
        /// Relevant program.
        #[arg(long, short = 'p', value_enum)]
        program: Program,

        #[command(flatten)]
        solana_payer_options: SolanaPayerOptions,
    },

    /// Migrate program accounts.
    MigrateProgramAccounts {
        /// Relevant program.
        #[arg(long, short = 'p', value_enum)]
        program: Program,

        #[command(flatten)]
        solana_payer_options: SolanaPayerOptions,
    },

    /// Set the admin key. Only the upgrade authority can execute this command.
    SetAdmin {
        /// Relevant program.
        #[arg(long, short = 'p', value_enum)]
        program: Program,

        admin_key: Pubkey,

        #[command(flatten)]
        solana_payer_options: SolanaPayerOptions,
    },
}

impl AdminSubCommand {
    pub async fn try_into_execute(self) -> Result<()> {
        match self {
            AdminSubCommand::ConfigureJournal {
                activation_cost: _,
                cost_per_epoch: _,
                payer_options: _,
            } => {
                todo!()
            }
            AdminSubCommand::ConfigureProgram {
                pause: _,
                unpause: _,
                accountant_key: _,
                sol_2z_swap_program_id: _,
                solana_validator_fee_percentage: _,
                calculation_grace_period_seconds: _,
                prepaid_connection_termination_relay_lamports: _,
                payer_options: _,
            } => {
                todo!()
            }
            AdminSubCommand::Initialize {
                program,
                solana_payer_options,
            } => match program {
                Program::Passport => {
                    execute_initialize_passport_program(solana_payer_options).await
                }
                Program::RevenueDistribution => {
                    execute_initialize_revenue_distribution_program(solana_payer_options).await
                }
            },
            AdminSubCommand::MigrateProgramAccounts {
                program,
                solana_payer_options,
            } => match program {
                Program::RevenueDistribution => {
                    execute_migrate_revenue_distribution_accounts(solana_payer_options).await
                }
                _ => {
                    anyhow::bail!("Migrate program accounts is only supported for Revenue Distribution program")
                }
            },
            AdminSubCommand::SetAdmin {
                program,
                admin_key,
                solana_payer_options,
            } => match program {
                Program::Passport => {
                    execute_passport_set_admin(admin_key, solana_payer_options).await
                }
                Program::RevenueDistribution => {
                    execute_revenue_distribution_set_admin(admin_key, solana_payer_options).await
                }
            },
        }
    }
}

//
// AdminSubCommand::Initialize.
//

async fn execute_initialize_passport_program(
    solana_payer_options: SolanaPayerOptions,
) -> Result<()> {
    let wallet = Wallet::try_from(solana_payer_options)?;

    let wallet_key = wallet.pubkey();

    let initialize_program_ix = try_build_instruction(
        &PASSPORT_PROGRAM_ID,
        passport_instruction::account::InitializeProgramAccounts::new(&wallet_key),
        &PassportInstructionData::InitializeProgram,
    )?;

    let set_admin_ix = try_build_instruction(
        &PASSPORT_PROGRAM_ID,
        passport_instruction::account::SetAdminAccounts::new(&PASSPORT_PROGRAM_ID, &wallet_key),
        &PassportInstructionData::SetAdmin(wallet_key),
    )?;

    // Precisely calculate the amount of compute units needed for the instructions.
    // There should be ~5k CU buffer with this base.
    let mut compute_unit_limit = 16_000;

    let (_, bump) = passport_state::ProgramConfig::find_address();
    compute_unit_limit += Wallet::compute_units_for_bump_seed(bump);

    let (_, bump) = get_program_data_address(&PASSPORT_PROGRAM_ID);
    compute_unit_limit += Wallet::compute_units_for_bump_seed(bump);

    let mut instructions = vec![
        initialize_program_ix,
        set_admin_ix,
        ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit),
    ];

    if let Some(ref compute_unit_price_ix) = wallet.compute_unit_price_ix {
        instructions.push(compute_unit_price_ix.clone());
    }

    let transaction = wallet.new_transaction(&instructions).await?;

    let tx_sig = wallet
        .send_and_confirm_transaction_with_spinner(&transaction)
        .await?;
    println!("Initialized Passport program: {tx_sig}");

    wallet.print_verbose_output(&[tx_sig]).await?;

    Ok(())
}

async fn execute_initialize_revenue_distribution_program(
    solana_payer_options: SolanaPayerOptions,
) -> Result<()> {
    let mut wallet = Wallet::try_from(solana_payer_options)?;
    let wallet_key = wallet.pubkey();

    wallet.connection.cache_if_mainnet().await?;

    let dz_mint_key = if wallet.connection.is_mainnet {
        doublezero_revenue_distribution::env::mainnet::DOUBLEZERO_MINT_KEY
    } else {
        doublezero_revenue_distribution::env::development::DOUBLEZERO_MINT_KEY
    };

    let initialize_program_ix = try_build_instruction(
        &REVENUE_DISTRIBUTION_PROGRAM_ID,
        revenue_distribution_instruction::account::InitializeProgramAccounts::new(
            &wallet_key,
            &dz_mint_key,
        ),
        &RevenueDistributionInstructionData::InitializeProgram,
    )?;

    let initialize_journal_ix = try_build_instruction(
        &REVENUE_DISTRIBUTION_PROGRAM_ID,
        revenue_distribution_instruction::account::InitializeJournalAccounts::new(
            &wallet_key,
            &dz_mint_key,
        ),
        &RevenueDistributionInstructionData::InitializeJournal,
    )?;

    let set_admin_ix = try_build_instruction(
        &REVENUE_DISTRIBUTION_PROGRAM_ID,
        revenue_distribution_instruction::account::SetAdminAccounts::new(
            &REVENUE_DISTRIBUTION_PROGRAM_ID,
            &wallet_key,
        ),
        &RevenueDistributionInstructionData::SetAdmin(wallet_key),
    )?;

    // Precisely calculate the amount of compute units needed for the instructions.
    // There should be ~5k CU buffer with this base.
    let mut compute_unit_limit = 42_000;

    let (program_config_key, bump) = revenue_distribution_state::ProgramConfig::find_address();
    compute_unit_limit += Wallet::compute_units_for_bump_seed(bump);

    let (_, bump) = revenue_distribution_state::find_2z_token_pda_address(&program_config_key);
    compute_unit_limit += Wallet::compute_units_for_bump_seed(bump);

    let (journal_key, bump) = revenue_distribution_state::Journal::find_address();
    compute_unit_limit += Wallet::compute_units_for_bump_seed(bump);

    let (_, bump) = revenue_distribution_state::find_2z_token_pda_address(&journal_key);
    compute_unit_limit += Wallet::compute_units_for_bump_seed(bump);

    let (_, bump) = get_program_data_address(&REVENUE_DISTRIBUTION_PROGRAM_ID);
    compute_unit_limit += Wallet::compute_units_for_bump_seed(bump);

    let mut instructions = vec![
        initialize_program_ix,
        initialize_journal_ix,
        set_admin_ix,
        ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit),
    ];

    if let Some(ref compute_unit_price_ix) = wallet.compute_unit_price_ix {
        instructions.push(compute_unit_price_ix.clone());
    }

    let transaction = wallet.new_transaction(&instructions).await?;

    let tx_sig = wallet
        .send_and_confirm_transaction_with_spinner(&transaction)
        .await?;
    println!("Initialized Revenue Distribution program: {tx_sig}");

    wallet.print_verbose_output(&[tx_sig]).await?;

    Ok(())
}

//
// AdminSubCommand::MigrateProgramAccounts.
//

async fn execute_migrate_revenue_distribution_accounts(
    solana_payer_options: SolanaPayerOptions,
) -> Result<()> {
    let wallet = Wallet::try_from(solana_payer_options)?;
    let wallet_key = wallet.pubkey();

    let migrate_program_accounts_ix = try_build_instruction(
        &REVENUE_DISTRIBUTION_PROGRAM_ID,
        revenue_distribution_instruction::account::MigrateProgramAccounts::new(
            &REVENUE_DISTRIBUTION_PROGRAM_ID,
            &wallet_key,
        ),
        &RevenueDistributionInstructionData::MigrateProgramAccounts,
    )?;

    let compute_unit_limit = 100_000;

    let mut instructions = vec![
        migrate_program_accounts_ix,
        ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit),
    ];

    if let Some(ref compute_unit_price_ix) = wallet.compute_unit_price_ix {
        instructions.push(compute_unit_price_ix.clone());
    }

    let transaction = wallet.new_transaction(&instructions).await?;

    let tx_sig = wallet
        .send_and_confirm_transaction_with_spinner(&transaction)
        .await?;
    println!("Migrate program accounts: {tx_sig}");

    wallet.print_verbose_output(&[tx_sig]).await?;

    Ok(())
}

//
// AdminSubCommand::SetAdmin.
//

async fn execute_passport_set_admin(
    admin_key: Pubkey,
    solana_payer_options: SolanaPayerOptions,
) -> Result<()> {
    let wallet = Wallet::try_from(solana_payer_options)?;

    let wallet_key = wallet.pubkey();

    let set_admin_ix = try_build_instruction(
        &PASSPORT_PROGRAM_ID,
        passport_instruction::account::SetAdminAccounts::new(&PASSPORT_PROGRAM_ID, &wallet_key),
        &PassportInstructionData::SetAdmin(admin_key),
    )?;

    // Precisely calculate the amount of compute units needed for the instructions.
    // There should be ~3k CU buffer with this base.
    let compute_unit_limit = 10_000;

    let mut instructions = vec![
        set_admin_ix,
        ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit),
    ];

    if let Some(ref compute_unit_price_ix) = wallet.compute_unit_price_ix {
        instructions.push(compute_unit_price_ix.clone());
    }

    let transaction = wallet.new_transaction(&instructions).await?;

    let tx_sig = wallet
        .send_and_confirm_transaction_with_spinner(&transaction)
        .await?;
    println!("Set Passport program admin: {tx_sig}");

    wallet.print_verbose_output(&[tx_sig]).await?;

    Ok(())
}

async fn execute_revenue_distribution_set_admin(
    admin_key: Pubkey,
    solana_payer_options: SolanaPayerOptions,
) -> Result<()> {
    let wallet = Wallet::try_from(solana_payer_options)?;
    let wallet_key = wallet.pubkey();

    let set_admin_ix = try_build_instruction(
        &REVENUE_DISTRIBUTION_PROGRAM_ID,
        revenue_distribution_instruction::account::SetAdminAccounts::new(
            &REVENUE_DISTRIBUTION_PROGRAM_ID,
            &wallet_key,
        ),
        &RevenueDistributionInstructionData::SetAdmin(admin_key),
    )?;

    // Precisely calculate the amount of compute units needed for the instructions.
    // There should be ~3k CU buffer with this base.
    let compute_unit_limit = 10_000;

    let mut instructions = vec![
        set_admin_ix,
        ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit),
    ];

    if let Some(ref compute_unit_price_ix) = wallet.compute_unit_price_ix {
        instructions.push(compute_unit_price_ix.clone());
    }

    let transaction = wallet.new_transaction(&instructions).await?;

    let tx_sig = wallet
        .send_and_confirm_transaction_with_spinner(&transaction)
        .await?;
    println!("Set Revenue Distribution program admin: {tx_sig}");

    wallet.print_verbose_output(&[tx_sig]).await?;

    Ok(())
}
