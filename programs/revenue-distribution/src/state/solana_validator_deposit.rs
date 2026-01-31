use bytemuck::{Pod, Zeroable};
use doublezero_program_tools::{types::StorageGap, Discriminator, PrecomputedDiscriminator};
use solana_pubkey::Pubkey;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(C, align(8))]
pub struct SolanaValidatorDeposit {
    pub node_id: Pubkey,

    /// The amount of SOL that was owed in past distributions but was never
    /// paid towards a distribution before the distribution's rewards were
    /// finalized.
    pub written_off_sol_debt: u64,

    /// The amount of SOL that was accrued from a past distribution, but was
    /// written off. This amount was paid towards a future distribution.
    pub recovered_sol_debt: u64,

    /// The amount of SOL that was erroneously calculated by the protocol.
    pub erroneous_sol_debt: u64,

    _padding: [u8; 8],

    _storage_gap: StorageGap<1>,
}

impl PrecomputedDiscriminator for SolanaValidatorDeposit {
    const DISCRIMINATOR: Discriminator<8> =
        Discriminator::new_sha2(b"dz::account::solana_validator_deposit");
}

impl SolanaValidatorDeposit {
    pub const SEED_PREFIX: &'static [u8] = b"solana_validator_deposit";

    pub fn find_address(node_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[Self::SEED_PREFIX, node_id.as_ref()], &crate::ID)
    }

    /// Returns the net bad SOL debt for this validator.
    ///
    /// This is the written-off debt minus any amounts that have been
    /// recovered or reclassified as erroneous.
    ///
    /// # Invariants
    ///
    /// The protocol guarantees:
    /// `recovered_sol_debt + erroneous_sol_debt <= written_off_sol_debt`
    ///
    /// Debt can only be recovered or reclassified after being written off,
    /// and bitmap tracking prevents double-counting.
    pub fn bad_sol_debt(&self) -> u64 {
        self.written_off_sol_debt
            .saturating_sub(self.recovered_sol_debt)
            .saturating_sub(self.erroneous_sol_debt)
    }
}

//

const _: () = assert!(
    size_of::<SolanaValidatorDeposit>() == 96,
    "`SolanaValidatorDeposit` size changed"
);
