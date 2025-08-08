use std::{ops::Deref, path::PathBuf};

use anyhow::Result;
use clap::Args;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, instruction::Instruction, pubkey::Pubkey,
    signature::Keypair, signer::Signer,
};

use crate::rpc::{Connection, SolanaConnectionOptions};

#[derive(Debug, Args)]
pub struct SolanaPayerOptions {
    #[command(flatten)]
    pub connection_options: SolanaConnectionOptions,

    #[command(flatten)]
    pub keypair_options: SolanaKeypairOptions,
}

// TODO: Add fee-payer like in solana CLI?
#[derive(Debug, Args)]
pub struct SolanaKeypairOptions {
    /// Filepath or URL to a keypair.
    #[arg(long = "keypair", short = 'k', value_name = "KEYPAIR")]
    pub keypair_path: Option<String>,

    /// Set the compute unit price for transaction in increments of 0.000001 lamports per compute
    /// unit.
    #[arg(long, value_name = "COMPUTE_UNIT_PRICE")]
    pub with_compute_unit_price: Option<u64>,
}

pub struct Wallet {
    pub connection: Connection,
    pub keypair: Keypair,
    pub compute_unit_price_ix: Option<Instruction>,
}

impl Wallet {
    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }
}

impl TryFrom<SolanaPayerOptions> for Wallet {
    type Error = anyhow::Error;

    fn try_from(opts: SolanaPayerOptions) -> Result<Wallet> {
        let SolanaPayerOptions {
            connection_options,
            keypair_options:
                SolanaKeypairOptions {
                    keypair_path,
                    with_compute_unit_price,
                },
        } = opts;

        let keypair = try_load_keypair(keypair_path.map(Into::into))?;

        Ok(Wallet {
            connection: Connection::try_from(connection_options)?,
            keypair,
            compute_unit_price_ix: with_compute_unit_price
                .map(ComputeBudgetInstruction::set_compute_unit_price),
        })
    }
}

impl Deref for Wallet {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

/// Taken from a Solana cookbook to load a keypair from a user's Solana config
/// location.
fn try_load_keypair(path: Option<PathBuf>) -> Result<Keypair> {
    let home_path = std::env::var_os("HOME").unwrap();
    let default_keypair_path = ".config/solana/id.json";

    let keypair_path = path.unwrap_or_else(|| PathBuf::from(home_path).join(default_keypair_path));

    let keypair_file = std::fs::read_to_string(keypair_path)?;
    let keypair_bytes = serde_json::from_str::<Vec<u8>>(&keypair_file)?;
    let default_keypair = Keypair::try_from(keypair_bytes.as_slice())?;

    Ok(default_keypair)
}
