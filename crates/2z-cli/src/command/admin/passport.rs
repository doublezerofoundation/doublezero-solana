use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use doublezero_passport::{
    ID,
    instruction::{
        PassportInstructionData, ProgramConfiguration, ProgramFlagConfiguration,
        account::{ConfigureProgramAccounts, InitializeProgramAccounts, SetAdminAccounts},
    },
    state::ProgramConfig,
};
use doublezero_program_tools::{get_program_data_address, instruction::try_build_instruction};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, instruction::Instruction, pubkey::Pubkey,
};

use crate::payer::{SolanaPayerOptions, Wallet};

#[derive(Debug, Args)]
pub struct PassportAdminCliCommand {
    #[command(subcommand)]
    pub command: PassportAdminSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum PassportAdminSubCommand {
    /// Initialize and set admin to upgrade authority.
    Initialize {
        #[command(flatten)]
        solana_payer_options: SolanaPayerOptions,
    },

    /// Set admin to a specified key.
    SetAdmin {
        admin_key: Pubkey,

        #[command(flatten)]
        solana_payer_options: SolanaPayerOptions,
    },

    /// Configure the program.
    Configure {
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
}

impl PassportAdminSubCommand {
    pub async fn try_into_execute(self) -> Result<()> {
        match self {
            PassportAdminSubCommand::Initialize {
                solana_payer_options,
            } => execute_initialize_program(solana_payer_options).await,
            PassportAdminSubCommand::SetAdmin {
                admin_key,
                solana_payer_options,
            } => execute_set_admin(admin_key, solana_payer_options).await,
            PassportAdminSubCommand::Configure {
                pause,
                unpause,
                solana_payer_options,
            } => execute_configure_program(pause, unpause, solana_payer_options).await,
        }
    }
}

//
// AdminSubCommand::Initialize.
//

pub async fn execute_initialize_program(solana_payer_options: SolanaPayerOptions) -> Result<()> {
    let wallet = Wallet::try_from(solana_payer_options)?;

    let wallet_key = wallet.pubkey();

    let initialize_program_ix = try_build_instruction(
        &ID,
        InitializeProgramAccounts::new(&wallet_key),
        &PassportInstructionData::InitializeProgram,
    )?;

    let set_admin_ix = try_build_instruction(
        &ID,
        SetAdminAccounts::new(&ID, &wallet_key),
        &PassportInstructionData::SetAdmin(wallet_key),
    )?;

    // Precisely calculate the amount of compute units needed for the instructions.
    // There should be ~5k CU buffer with this base.
    let mut compute_unit_limit = 16_000;

    let (_, bump) = ProgramConfig::find_address();
    compute_unit_limit += Wallet::compute_units_for_bump_seed(bump);

    let (_, bump) = get_program_data_address(&ID);
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
    let tx_sig = wallet.send_or_simulate_transaction(&transaction).await?;

    if let Some(tx_sig) = tx_sig {
        println!("Initialized Passport program: {tx_sig}");

        wallet.print_verbose_output(&[tx_sig]).await?;
    }

    Ok(())
}

//
// AdminSubCommand::SetAdmin.
//

pub async fn execute_set_admin(
    admin_key: Pubkey,
    solana_payer_options: SolanaPayerOptions,
) -> Result<()> {
    let wallet = Wallet::try_from(solana_payer_options)?;

    let wallet_key = wallet.pubkey();

    let set_admin_ix = try_build_instruction(
        &ID,
        SetAdminAccounts::new(&ID, &wallet_key),
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
    let tx_sig = wallet.send_or_simulate_transaction(&transaction).await?;

    if let Some(tx_sig) = tx_sig {
        println!("Set Passport program admin: {tx_sig}");

        wallet.print_verbose_output(&[tx_sig]).await?;
    }

    Ok(())
}

//
// AdminConfigureSubCommand::Passport.
//

pub async fn execute_configure_program(
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
            bail!("Cannot use both --pause and --unpause at the same time");
        }
        (true, false) => {
            let configure_program_ix = try_build_configure_program_instruction(
                &wallet_key,
                ProgramConfiguration::Flag(ProgramFlagConfiguration::IsPaused(true)),
            )?;
            instructions.push(configure_program_ix);
            compute_unit_limit += 2_000;
        }
        (false, true) => {
            let configure_program_ix = try_build_configure_program_instruction(
                &wallet_key,
                ProgramConfiguration::Flag(ProgramFlagConfiguration::IsPaused(false)),
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
    let tx_sig = wallet.send_or_simulate_transaction(&transaction).await?;

    if let Some(tx_sig) = tx_sig {
        println!("Configured Passport program: {tx_sig}");

        wallet.print_verbose_output(&[tx_sig]).await?;
    }

    Ok(())
}

//

fn try_build_configure_program_instruction(
    admin_key: &Pubkey,
    setting: ProgramConfiguration,
) -> Result<Instruction> {
    try_build_instruction(
        &ID,
        ConfigureProgramAccounts::new(admin_key),
        &PassportInstructionData::ConfigureProgram(setting),
    )
    .map_err(Into::into)
}
