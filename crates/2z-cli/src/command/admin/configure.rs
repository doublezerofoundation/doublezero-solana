use anyhow::Result;
use clap::{Args, Subcommand};
use doublezero_passport::{
    instruction::{self as passport_instruction, PassportInstructionData},
    ID as PASSPORT_PROGRAM_ID,
};
use doublezero_program_tools::instruction::try_build_instruction;
use doublezero_revenue_distribution::{
    instruction::{self as revenue_distribution_instruction, RevenueDistributionInstructionData},
    ID as REVENUE_DISTRIBUTION_PROGRAM_ID,
};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, instruction::Instruction, pubkey::Pubkey,
};

use crate::payer::{SolanaPayerOptions, Wallet};

#[derive(Debug, Args)]
pub struct AdminConfigureCliCommand {
    #[command(subcommand)]
    pub command: AdminConfigureSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum AdminConfigureSubCommand {
    Passport {
        // Flags.
        //
        /// Whether to pause the program. Cannot be used with --unpause.
        #[arg(long)]
        pause: bool,

        /// Whether to unpause the program. Cannot be used with --pause.
        #[arg(long)]
        unpause: bool,

        #[command(flatten)]
        solana_payer_options: SolanaPayerOptions,
    },

    /// Configure the journal account on the Revenue Distribution program.
    RevenueDistribution(ConfigureRevenueDistributionCommand),
}

impl AdminConfigureSubCommand {
    pub async fn try_into_execute(self) -> Result<()> {
        match self {
            AdminConfigureSubCommand::Passport {
                pause,
                unpause,
                solana_payer_options,
            } => execute_configure_passport(pause, unpause, solana_payer_options).await,
            AdminConfigureSubCommand::RevenueDistribution(command) => {
                execute_configure_revenue_distribution(command).await
            }
        }
    }
}

#[derive(Debug, Args)]
pub struct ConfigureRevenueDistributionCommand {
    // Flags.
    //
    /// Whether to pause the program. Cannot be used with --unpause.
    #[arg(long)]
    pub pause: bool,

    /// Whether to unpause the program. Cannot be used with --pause.
    #[arg(long)]
    pub unpause: bool,

    // Other configuration.
    //
    /// Set the payments accountant key.
    #[arg(long)]
    pub payments_accountant: Option<Pubkey>,

    /// Set the rewards accountant key.
    #[arg(long)]
    pub rewards_accountant: Option<Pubkey>,

    /// Set the SOL/2Z Swap program ID.
    #[arg(long)]
    pub sol_2z_swap_program: Option<Pubkey>,

    /// Solana validator fee percentage (max: 100%).
    #[arg(long)]
    pub solana_validator_fee_percentage: Option<String>,

    /// How long the accountant must wait to fetch telemetry data for reward calculations.
    #[arg(long)]
    pub calculation_grace_period_seconds: Option<u32>,

    /// Amount to pay relayer to terminate a prepaid connection.
    #[arg(long)]
    pub prepaid_connection_termination_relay_lamports: Option<u32>,

    /// Activation cost for a prepaid connection.
    #[arg(long)]
    pub activation_cost: Option<u32>,

    /// Cost per DoubleZero epoch for a prepaid connection.
    #[arg(long)]
    pub cost_per_epoch: Option<u32>,

    #[command(flatten)]
    pub solana_payer_options: SolanaPayerOptions,
}

//
// AdminConfigureSubCommand::Passport.
//

async fn execute_configure_passport(
    pause: bool,
    unpause: bool,
    solana_payer_options: SolanaPayerOptions,
) -> Result<()> {
    let wallet = Wallet::try_from(solana_payer_options)?;
    let wallet_key = wallet.pubkey();

    let mut instructions = vec![];
    let mut compute_unit_limit = 5_000;

    match (pause, unpause) {
        (true, true) => {
            return Err(anyhow::anyhow!(
                "Cannot use both --pause and --unpause at the same time"
            ));
        }
        (true, false) => {
            let configure_program_ix = try_build_passport_configure_instruction(
                &wallet_key,
                passport_instruction::ProgramConfiguration::Flag(
                    passport_instruction::ProgramFlagConfiguration::IsPaused(true),
                ),
            )?;
            instructions.push(configure_program_ix);
            compute_unit_limit += 2_000;
        }
        (false, true) => {
            let configure_program_ix = try_build_passport_configure_instruction(
                &wallet_key,
                passport_instruction::ProgramConfiguration::Flag(
                    passport_instruction::ProgramFlagConfiguration::IsPaused(false),
                ),
            )?;
            instructions.push(configure_program_ix);
            compute_unit_limit += 2_000;
        }
        (false, false) => {}
    }

    instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
        compute_unit_limit,
    ));

    if let Some(ref compute_unit_price_ix) = wallet.compute_unit_price_ix {
        instructions.push(compute_unit_price_ix.clone());
    }

    let transaction = wallet.new_transaction(&instructions).await?;

    let tx_sig = wallet
        .send_and_confirm_transaction_with_spinner(&transaction)
        .await?;
    println!("Configured Passport program: {tx_sig}");

    wallet.print_verbose_output(&[tx_sig]).await?;

    Ok(())
}

//
// AdminConfigureSubCommand::RevenueDistribution.
//

async fn execute_configure_revenue_distribution(
    command: ConfigureRevenueDistributionCommand,
) -> Result<()> {
    let ConfigureRevenueDistributionCommand {
        pause,
        unpause,
        payments_accountant,
        rewards_accountant,
        sol_2z_swap_program,
        solana_validator_fee_percentage: _,
        calculation_grace_period_seconds,
        prepaid_connection_termination_relay_lamports,
        activation_cost: _,
        cost_per_epoch: _,
        solana_payer_options,
    } = command;

    let wallet = Wallet::try_from(solana_payer_options)?;
    let wallet_key = wallet.pubkey();

    let mut instructions = vec![];
    let mut compute_unit_limit = 5_000;

    match (pause, unpause) {
        (true, true) => {
            return Err(anyhow::anyhow!(
                "Cannot use both --pause and --unpause at the same time"
            ));
        }
        (true, false) => {
            let configure_program_ix = try_build_revenue_distribution_configure_instruction(
                &wallet_key,
                revenue_distribution_instruction::ProgramConfiguration::Flag(
                    revenue_distribution_instruction::ProgramFlagConfiguration::IsPaused(true),
                ),
            )?;
            instructions.push(configure_program_ix);
            compute_unit_limit += 2_000;
        }
        (false, true) => {
            let configure_program_ix = try_build_revenue_distribution_configure_instruction(
                &wallet_key,
                revenue_distribution_instruction::ProgramConfiguration::Flag(
                    revenue_distribution_instruction::ProgramFlagConfiguration::IsPaused(false),
                ),
            )?;
            instructions.push(configure_program_ix);
            compute_unit_limit += 2_000;
        }
        (false, false) => {}
    }

    if let Some(payments_accountant_key) = payments_accountant {
        let configure_program_ix = try_build_revenue_distribution_configure_instruction(
            &wallet_key,
            revenue_distribution_instruction::ProgramConfiguration::PaymentsAccountant(
                payments_accountant_key,
            ),
        )?;
        instructions.push(configure_program_ix);
        compute_unit_limit += 2_000;
    }

    if let Some(rewards_accountant_key) = rewards_accountant {
        let configure_program_ix = try_build_revenue_distribution_configure_instruction(
            &wallet_key,
            revenue_distribution_instruction::ProgramConfiguration::RewardsAccountant(
                rewards_accountant_key,
            ),
        )?;
        instructions.push(configure_program_ix);
        compute_unit_limit += 2_000;
    }

    if let Some(sol_2z_swap_program_id) = sol_2z_swap_program {
        let configure_program_ix = try_build_revenue_distribution_configure_instruction(
            &wallet_key,
            revenue_distribution_instruction::ProgramConfiguration::Sol2zSwapProgram(
                sol_2z_swap_program_id,
            ),
        )?;
        instructions.push(configure_program_ix);
        compute_unit_limit += 2_000;
    }

    if let Some(calculation_grace_period_seconds) = calculation_grace_period_seconds {
        let configure_program_ix = try_build_revenue_distribution_configure_instruction(
            &wallet_key,
            revenue_distribution_instruction::ProgramConfiguration::CalculationGracePeriodSeconds(
                calculation_grace_period_seconds,
            ),
        )?;
        instructions.push(configure_program_ix);
        compute_unit_limit += 2_000;
    }

    if let Some(prepaid_connection_termination_relay_lamports) =
        prepaid_connection_termination_relay_lamports
    {
        let configure_program_ix = try_build_revenue_distribution_configure_instruction(
            &wallet_key,
            revenue_distribution_instruction::ProgramConfiguration::PrepaidConnectionTerminationRelayLamports(
                prepaid_connection_termination_relay_lamports,
            ),
        )?;
        instructions.push(configure_program_ix);
        compute_unit_limit += 2_000;
    }

    // NOTE: We may need to chunk these instructions if more configurations are
    // added.

    instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
        compute_unit_limit,
    ));

    if let Some(ref compute_unit_price_ix) = wallet.compute_unit_price_ix {
        instructions.push(compute_unit_price_ix.clone());
    }

    let transaction = wallet.new_transaction(&instructions).await?;

    let tx_sig = wallet
        .send_and_confirm_transaction_with_spinner(&transaction)
        .await?;
    println!("Configured Revenue Distribution program: {tx_sig}");

    wallet.print_verbose_output(&[tx_sig]).await?;

    Ok(())
}

fn try_build_passport_configure_instruction(
    admin_key: &Pubkey,
    setting: passport_instruction::ProgramConfiguration,
) -> Result<Instruction> {
    try_build_instruction(
        &PASSPORT_PROGRAM_ID,
        passport_instruction::account::ConfigureProgramAccounts::new(admin_key),
        &PassportInstructionData::ConfigureProgram(setting),
    )
    .map_err(Into::into)
}

fn try_build_revenue_distribution_configure_instruction(
    admin_key: &Pubkey,
    setting: revenue_distribution_instruction::ProgramConfiguration,
) -> Result<Instruction> {
    try_build_instruction(
        &REVENUE_DISTRIBUTION_PROGRAM_ID,
        revenue_distribution_instruction::account::ConfigureProgramAccounts::new(admin_key),
        &RevenueDistributionInstructionData::ConfigureProgram(setting),
    )
    .map_err(Into::into)
}
