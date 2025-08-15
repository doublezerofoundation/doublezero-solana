use anyhow::{anyhow, Result};
use clap::{Args, Subcommand};
use doublezero_program_tools::zero_copy;
use doublezero_revenue_distribution::state::{Journal, ProgramConfig};

use crate::rpc::{Connection, SolanaConnectionOptions};

#[derive(Debug, Args)]
pub struct RevenueDistributionCliCommand {
    #[command(subcommand)]
    pub command: RevenueDistributionSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum RevenueDistributionSubCommand {
    Fetch {
        #[arg(long)]
        program_config: bool,

        #[arg(long)]
        journal: bool,

        // TODO: --distribution with Option<u64>.
        // TODO: --contributor-rewards with Option<Pubkey>.
        // TODO: --prepaid-connection with Option<Pubkey>.
        //
        #[command(flatten)]
        solana_connection_options: SolanaConnectionOptions,
    },
}

impl RevenueDistributionSubCommand {
    pub async fn try_into_execute(self) -> Result<()> {
        match self {
            RevenueDistributionSubCommand::Fetch {
                program_config,
                journal,
                solana_connection_options,
            } => {
                let connection = Connection::try_from(solana_connection_options)?;

                if program_config {
                    let program_config_key = ProgramConfig::find_address().0;
                    let program_config_info = connection.get_account(&program_config_key).await?;

                    let (program_config, _) = zero_copy::checked_from_bytes_with_discriminator::<
                        ProgramConfig,
                    >(&program_config_info.data)
                    .ok_or(anyhow!("Failed to deserialize program config"))?;

                    // TODO: Pretty print.
                    println!("Program config: {:?}", program_config);
                }

                if journal {
                    let journal_key = Journal::find_address().0;
                    let journal_info = connection.get_account(&journal_key).await?;

                    let (journal, _) = zero_copy::checked_from_bytes_with_discriminator::<Journal>(
                        &journal_info.data,
                    )
                    .ok_or(anyhow!("Failed to deserialize journal"))?;

                    // TODO: Pretty print.
                    println!("Journal: {:?}", journal);
                }

                Ok(())
            }
        }
    }
}
