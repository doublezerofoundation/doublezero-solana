use crate::{
    state::StorageGap,
    types::{DoubleZeroEpoch, Flags},
};
use bytemuck::{Pod, Zeroable};
use doublezero_program_tools::{Discriminator, PrecomputedDiscriminator};
use solana_pubkey::Pubkey;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct PrepaidUser {
    flags: Flags,

    user: Pubkey,
    valid_through_dz_epoch: DoubleZeroEpoch,

    disconnect_beneficiary: Pubkey,
    disconnect_relay_lamports: u64,

    _gap: StorageGap<8>,
}

impl PrecomputedDiscriminator for PrepaidUser {
    const DISCRIMINATOR: Discriminator<8> = Discriminator::new_sha2(b"dz::account::prepaid_user");
}

impl PrepaidUser {
    pub const SEED_PREFIX: &'static [u8] = b"prepaid_user";

    pub fn find_address(prepaid_user_key: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[Self::SEED_PREFIX, &prepaid_user_key.as_ref()], &crate::ID)
    }
}
