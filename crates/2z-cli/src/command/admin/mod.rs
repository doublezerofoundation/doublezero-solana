mod passport;
mod revenue_distribution;

//

use anyhow::{bail, Result};
use clap::{Args, Subcommand, ValueEnum};
use solana_sdk::pubkey::Pubkey;

use crate::payer::SolanaPayerOptions;

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
    /// Configure a specified program.
    Configure(AdminConfigureCliCommand),

    /// Initialize specified program.
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

    /// Set the admin key for specified program.
    SetAdmin {
        admin_key: Pubkey,

        /// Relevant program.
        #[arg(long, short = 'p', value_enum)]
        program: Program,

        #[command(flatten)]
        solana_payer_options: SolanaPayerOptions,
    },
}

impl AdminSubCommand {
    pub async fn try_into_execute(self) -> Result<()> {
        match self {
            AdminSubCommand::Configure(configure) => configure.command.try_into_execute().await,
            AdminSubCommand::Initialize {
                program,
                solana_payer_options,
            } => match program {
                Program::Passport => {
                    passport::execute_initialize_program(solana_payer_options).await
                }
                Program::RevenueDistribution => {
                    revenue_distribution::execute_initialize_program(solana_payer_options).await
                }
            },
            AdminSubCommand::MigrateProgramAccounts {
                program,
                solana_payer_options,
            } => match program {
                Program::RevenueDistribution => {
                    revenue_distribution::execute_migrate_program_accounts(solana_payer_options)
                        .await
                }
                _ => {
                    bail!("Migrate program accounts not supported for Passport program");
                }
            },
            AdminSubCommand::SetAdmin {
                program,
                admin_key,
                solana_payer_options,
            } => match program {
                Program::Passport => {
                    passport::execute_set_admin(admin_key, solana_payer_options).await
                }
                Program::RevenueDistribution => {
                    revenue_distribution::execute_set_admin(admin_key, solana_payer_options).await
                }
            },
        }
    }
}

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
    RevenueDistribution {
        #[command(flatten)]
        configure_options: Box<ConfigureRevenueDistributionOptions>,

        #[command(flatten)]
        solana_payer_options: SolanaPayerOptions,
    },
}

impl AdminConfigureSubCommand {
    pub async fn try_into_execute(self) -> Result<()> {
        match self {
            AdminConfigureSubCommand::Passport {
                pause,
                unpause,
                solana_payer_options,
            } => passport::execute_configure_program(pause, unpause, solana_payer_options).await,
            AdminConfigureSubCommand::RevenueDistribution {
                configure_options,
                solana_payer_options,
            } => {
                revenue_distribution::execute_configure_program(
                    configure_options,
                    solana_payer_options,
                )
                .await
            }
        }
    }
}

#[derive(Debug, Args)]
pub struct ConfigureRevenueDistributionOptions {
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

    /// Solana validator base block rewards fee percentage (max: 100%).
    #[arg(long)]
    pub solana_validator_base_block_rewards_fee: Option<String>,

    /// Solana validator priority block rewards fee percentage (max: 100%).
    #[arg(long)]
    pub solana_validator_priority_block_rewards_fee: Option<String>,

    /// Solana validator inflation rewards fee percentage (max: 100%).
    #[arg(long)]
    pub solana_validator_inflation_rewards_fee: Option<String>,

    /// Solana validator Jito tips fee percentage (max: 100%).
    #[arg(long)]
    pub solana_validator_jito_tips_fee: Option<String>,

    /// How long the accountant must wait to fetch telemetry data for reward calculations.
    #[arg(long)]
    pub calculation_grace_period_seconds: Option<u32>,

    /// Amount to pay relayer to terminate a prepaid connection.
    #[arg(long)]
    pub prepaid_connection_termination_relay_lamports: Option<u32>,

    /// Community burn rate limit percentage (max: 100%, precision: 7 decimals).
    #[arg(long)]
    pub community_burn_rate_limit: Option<String>,

    #[arg(long)]
    pub epochs_to_increasing_community_burn_rate: Option<u32>,

    #[arg(long)]
    pub epochs_to_community_burn_rate_limit: Option<u32>,

    /// Initial community burn rate percentage (max: 100%, precision: 7 decimals).
    #[arg(long)]
    pub initial_community_burn_rate: Option<String>,

    /// Activation cost for a prepaid connection.
    #[arg(long)]
    pub activation_cost: Option<u32>,

    /// Cost per DoubleZero epoch for a prepaid connection.
    #[arg(long)]
    pub cost_per_epoch: Option<u32>,
}
