use bytemuck::{Pod, Zeroable};
use doublezero_program_tools::{
    types::{Flags, StorageGap},
    Discriminator, PrecomputedDiscriminator,
};
use doublezero_revenue_distribution::types::DoubleZeroEpoch;
use solana_pubkey::Pubkey;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(C, align(8))]
pub struct MockIntegrationDistribution {
    pub dz_epoch: DoubleZeroEpoch,
    pub is_collected: u8,
    pub bump_seed: u8,
    _padding: [u8; 6],

    // Reserved for future flags.
    _flags: Flags,

    _storage_gap: StorageGap<4>,
}

impl PrecomputedDiscriminator for MockIntegrationDistribution {
    const DISCRIMINATOR: Discriminator<8> =
        Discriminator::new_sha2(b"mock::account::integration_distribution");
}

impl MockIntegrationDistribution {
    pub const SEED_PREFIX: &'static [u8] = b"mock_integration_distribution";

    pub fn find_address(dz_epoch: DoubleZeroEpoch) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[Self::SEED_PREFIX, &dz_epoch.as_seed()], &crate::ID)
    }
}
