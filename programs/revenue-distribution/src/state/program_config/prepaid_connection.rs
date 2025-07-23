use bytemuck::{Pod, Zeroable};

use crate::{state::StorageGap, types::EpochDuration};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct PrepaidConnectionParameters {
    pub minimum_activation_dz_epochs: EpochDuration,
    pub maximum_activation_dz_epochs: EpochDuration,

    pub activation_cost: u32,
    pub cost_per_dz_epoch: u32,

    _storage_gap: StorageGap<2>,
}

impl PrepaidConnectionParameters {
    pub fn is_within_activation_boundaries(&self, epoch_duration: EpochDuration) -> bool {
        epoch_duration >= self.minimum_activation_dz_epochs
            && epoch_duration <= self.maximum_activation_dz_epochs
    }

    pub fn checked_activation_cost(&self, decimals: u8) -> Option<u64> {
        checked_pow_10(decimals)?.checked_mul(self.activation_cost.into())
    }

    pub fn checked_cost_per_dz_epoch(&self, decimals: u8) -> Option<u64> {
        checked_pow_10(decimals)?.checked_mul(self.cost_per_dz_epoch.into())
    }

    pub fn checked_duration_cost(
        &self,
        epoch_duration: EpochDuration,
        decimals: u8,
    ) -> Option<u64> {
        self.checked_cost_per_dz_epoch(decimals)?
            .checked_mul(epoch_duration.into())
    }
}

#[inline(always)]
fn checked_pow_10(decimals: u8) -> Option<u64> {
    u64::checked_pow(10, decimals.into())
}
