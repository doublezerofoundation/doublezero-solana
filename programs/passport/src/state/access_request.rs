use bytemuck::{Pod, Zeroable};
use doublezero_program_tools::{Discriminator, PrecomputedDiscriminator};
use solana_pubkey::Pubkey;

#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
#[repr(C, align(8))]
pub struct AccessRequest {
    pub service_key: Pubkey,
    pub validator_id: Pubkey,
    pub rent_beneficiary_key: Pubkey,
}

impl Default for AccessRequest {
    fn default() -> Self {
        Self {
            service_key: Pubkey::default(),
            validator_id: Pubkey::default(),
            rent_beneficiary_key: Pubkey::default(),
        }
    }
}

impl PrecomputedDiscriminator for AccessRequest {
    const DISCRIMINATOR: Discriminator<8> = Discriminator::new_sha2(b"dz::account::access_request");
}

impl AccessRequest {
    pub const SEED_PREFIX: &'static [u8] = b"access_request";

    pub fn find_address(service_key: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[Self::SEED_PREFIX, service_key.as_ref()], &crate::ID)
    }

    pub fn access_request_message(service_key: &Pubkey) -> [u8; 48] {
        let mut buf = [0u8; 48];
        buf[..16].copy_from_slice(b"solana_validator");
        buf[16..].copy_from_slice(service_key.as_ref());
        buf
    }
}

const _: () = assert!(
    size_of::<AccessRequest>() == 96,
    "`AccessRequest` size changed"
);
