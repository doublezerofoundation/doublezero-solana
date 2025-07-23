use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};
use doublezero_program_tools::{Discriminator, PrecomputedDiscriminator};
use solana_pubkey::Pubkey;

use crate::{
    state::StorageGap,
    types::{DoubleZeroEpoch, EpochDuration},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct Journal {
    /// This seed will be used to sign for token transfers.
    pub bump_seed: u8,

    /// Cache this seed to validate token PDA address.
    pub token_2z_pda_bump_seed: u8,
    _bump_seed_padding: [u8; 2],

    pub minimum_prepaid_dz_epochs: u16,
    pub maximum_entries: u16,

    pub total_sol_balance: u64,

    /// Based on interactions with the program to deposit 2Z, this is our expected balance. This
    /// balance may deviate from the actual balance in the 2Z Token account because folks may
    /// transfer tokens directly to that account (not intended). So if we wanted any recourse to
    /// do something with the excess amount in this token account, we can simply compute the
    /// difference between the token account balance and this.
    pub total_2z_balance: u64,

    pub activation_cost: u32,
    pub cost_per_dz_epoch: u32,

    /// 8 * 32 bytes of a storage gap in case we need more fields.
    _storage_gap: StorageGap<8>,
}

impl PrecomputedDiscriminator for Journal {
    const DISCRIMINATOR: Discriminator<8> = Discriminator::new_sha2(b"dz::account::journal");
}

impl Journal {
    pub const SEED_PREFIX: &'static [u8] = b"journal";

    pub fn find_address() -> (Pubkey, u8) {
        Pubkey::find_program_address(&[Self::SEED_PREFIX], &crate::ID)
    }

    pub fn checked_journal_entries(data: &[u8]) -> Option<JournalEntries> {
        BorshDeserialize::try_from_slice(data).ok()
    }

    pub fn checked_activation_cost(&self, decimals: u8) -> Option<u64> {
        checked_pow_10(decimals)?.checked_mul(self.activation_cost.into())
    }

    pub fn checked_cost_per_dz_epoch(&self, decimals: u8) -> Option<u64> {
        checked_pow_10(decimals)?.checked_mul(self.cost_per_dz_epoch.into())
    }

    pub fn checked_duration_cost(&self, num_entries: u16, decimals: u8) -> Option<u64> {
        self.checked_cost_per_dz_epoch(decimals)?
            .checked_mul(num_entries.into())
    }
}

#[derive(Debug, BorshDeserialize, BorshSerialize, Clone, Copy, PartialEq, Eq)]
pub struct JournalEntry {
    pub dz_epoch: DoubleZeroEpoch,
    pub amount: u64,
}

#[derive(Debug, BorshDeserialize, BorshSerialize, Clone, Default, PartialEq, Eq)]
pub struct JournalEntries(pub Vec<JournalEntry>);

impl JournalEntries {
    pub fn last_dz_epoch(&self) -> Option<DoubleZeroEpoch> {
        self.0.last().map(|entry| entry.dz_epoch)
    }

    pub fn extend(
        &mut self,
        next_dz_epoch: DoubleZeroEpoch,
        maximum_epoch_duration_to_load: EpochDuration,
    ) {
        let end_dz_epoch = self
            .last_dz_epoch()
            .unwrap_or(next_dz_epoch)
            .saturating_add_duration(1);

        let new_last_dz_epoch =
            end_dz_epoch.saturating_add_duration(maximum_epoch_duration_to_load);

        if end_dz_epoch < new_last_dz_epoch {
            for epoch_value in end_dz_epoch.value()..=new_last_dz_epoch.value() {
                self.0.push(JournalEntry {
                    dz_epoch: DoubleZeroEpoch::new(epoch_value),
                    amount: 0,
                });
            }
        }
    }
    pub fn update(
        &mut self,
        next_dz_epoch: DoubleZeroEpoch,
        valid_through_dz_epoch: DoubleZeroEpoch,
        cost_per_epoch: u64,
    ) {
        let entries = &mut self.0;

        // First, add amounts to existing entries where we need to allocate 2Z to specific DZ epochs.
        entries
            .iter_mut()
            .filter(|entry| {
                entry.dz_epoch >= next_dz_epoch && entry.dz_epoch <= valid_through_dz_epoch
            })
            .for_each(|entry| entry.amount = entry.amount.saturating_add(cost_per_epoch));

        // Find the last epoch so we can push the cost-per-epoch as new entries.
        let last_dz_epoch = entries
            .last()
            .map(|entry| entry.dz_epoch)
            .unwrap_or(next_dz_epoch)
            .saturating_add_duration(1);

        if last_dz_epoch <= valid_through_dz_epoch {
            for epoch_value in last_dz_epoch.value()..=valid_through_dz_epoch.value() {
                entries.push(JournalEntry {
                    dz_epoch: DoubleZeroEpoch::new(epoch_value),
                    amount: cost_per_epoch,
                });
            }
        }
    }
}

#[inline(always)]
fn checked_pow_10(decimals: u8) -> Option<u64> {
    u64::checked_pow(10, decimals.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_journal_entries_update_full_overlap() {
        let next_dz_epoch = DoubleZeroEpoch::new(0);
        let valid_through_dz_epoch = DoubleZeroEpoch::new(5);

        let cost_per_epoch = 69;

        let mut journal_entries = JournalEntries(vec![
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(0),
                amount: 100,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(1),
                amount: 200,
            },
        ]);

        journal_entries.update(next_dz_epoch, valid_through_dz_epoch, cost_per_epoch);

        let expected_journal_entries = JournalEntries(vec![
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(0),
                amount: 169,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(1),
                amount: 269,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(2),
                amount: 69,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(3),
                amount: 69,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(4),
                amount: 69,
            },
            JournalEntry {
                dz_epoch: DoubleZeroEpoch::new(5),
                amount: 69,
            },
        ]);
        assert_eq!(journal_entries, expected_journal_entries);
    }
}
