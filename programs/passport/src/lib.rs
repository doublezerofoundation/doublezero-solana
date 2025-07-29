pub mod instruction;
#[cfg(not(feature = "no-entrypoint"))]
mod processor;
pub mod state;

//

solana_pubkey::declare_id!("E2tRaDuoom5nUg24H9bGNETBHkVokzTgLLPKa2oeuoqH");
