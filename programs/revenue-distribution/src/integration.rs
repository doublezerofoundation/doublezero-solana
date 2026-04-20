//! Interface that rewards integration programs implement so rev-distr can
//! CPI into them uniformly.
//!
//! Integration programs should:
//! 1. Add `doublezero-revenue-distribution` as a dependency.
//! 2. In their own `try_process_instruction`, try
//!    [`IntegrationInstructionData::try_from_slice`] before falling through
//!    to their native instruction match:
//!
//!    ```ignore
//!    use borsh::BorshDeserialize;
//!    use doublezero_revenue_distribution::integration::IntegrationInstructionData;
//!
//!    fn try_process_instruction(
//!        program_id: &Pubkey,
//!        accounts: &[AccountInfo],
//!        data: &[u8],
//!    ) -> ProgramResult {
//!        if let Ok(ix) = IntegrationInstructionData::try_from_slice(data) {
//!            return try_handle_integration_ix(accounts, ix);
//!        }
//!        // Fall through to the integration's native instruction set.
//!        let ix = MyNativeInstructionData::try_from_slice(data)?;
//!        match ix { /* ... */ }
//!    }
//!    ```
//! 3. Implement the handler for each variant (only
//!    [`IntegrationInstructionData::WithdrawIntegrationRewards`] for now).

use borsh::{BorshDeserialize, BorshSerialize};
use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;

/// Instruction data that rev-distr CPIs with when harvesting integrations.
///
/// The Borsh discriminator + argument layout is the stable wire format every
/// integration program must honor. The compiler enforces drift: adding a
/// variant here breaks every integration that hasn't been updated.
#[derive(Debug, Clone, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
pub enum IntegrationInstructionData {
    /// Rev-distr asks the integration to transfer the epoch's contributor
    /// share out of its bucket and into a destination token account, and to
    /// flip its `is_collected` flag so subsequent calls are rejected.
    ///
    /// See [`IntegrationAccounts`] for the expected account list.
    WithdrawIntegrationRewards,
}

/// Account list rev-distr uses when CPI-invoking an integration's
/// [`IntegrationInstructionData::WithdrawIntegrationRewards`] handler.
///
/// Slot order is part of the interface contract and must not change without
/// coordinating all implementers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationAccounts {
    /// Slot 0: integration's per-epoch state PDA (writable). Owned by the
    /// integration program. Contains the `is_collected` flag the handler
    /// flips.
    pub integration_distribution_key: Pubkey,

    /// Slot 1: integration's per-epoch 2Z bucket (writable). Owned by
    /// [`integration_distribution_key`][Self::integration_distribution_key].
    /// Source of the transfer.
    pub integration_2z_bucket_key: Pubkey,

    /// Slot 2: destination token account (writable). Rev-distr's
    /// `Distribution` 2Z PDA. Recipient of the transfer.
    pub destination_token_account_key: Pubkey,

    /// Slot 3: rev-distr's `Distribution` PDA (signer, read-only). The
    /// integration uses this as proof the caller is rev-distr (signer check)
    /// and reads the epoch off of it to verify cross-epoch harvesting is not
    /// happening.
    pub rev_distr_distribution_key: Pubkey,
}

impl From<IntegrationAccounts> for Vec<AccountMeta> {
    fn from(accounts: IntegrationAccounts) -> Self {
        let IntegrationAccounts {
            integration_distribution_key,
            integration_2z_bucket_key,
            destination_token_account_key,
            rev_distr_distribution_key,
        } = accounts;

        vec![
            AccountMeta::new(integration_distribution_key, false),
            AccountMeta::new(integration_2z_bucket_key, false),
            AccountMeta::new(destination_token_account_key, false),
            AccountMeta::new_readonly(rev_distr_distribution_key, true),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn withdraw_integration_rewards_borsh_roundtrip() {
        let ix = IntegrationInstructionData::WithdrawIntegrationRewards;

        let serialized = borsh::to_vec(&ix).unwrap();
        let deserialized = IntegrationInstructionData::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized, ix);
    }

    #[test]
    fn withdraw_integration_rewards_discriminator_is_stable() {
        // The enum's discriminator is encoded as the first byte (Borsh enum
        // variant index). Future variants must be appended; existing ones
        // must keep their index for backward compatibility.
        let serialized =
            borsh::to_vec(&IntegrationInstructionData::WithdrawIntegrationRewards).unwrap();
        assert_eq!(serialized, vec![0]);
    }

    #[test]
    fn integration_accounts_into_meta_preserves_order_and_flags() {
        let accounts = IntegrationAccounts {
            integration_distribution_key: Pubkey::new_unique(),
            integration_2z_bucket_key: Pubkey::new_unique(),
            destination_token_account_key: Pubkey::new_unique(),
            rev_distr_distribution_key: Pubkey::new_unique(),
        };
        let keys = [
            accounts.integration_distribution_key,
            accounts.integration_2z_bucket_key,
            accounts.destination_token_account_key,
            accounts.rev_distr_distribution_key,
        ];

        let metas: Vec<AccountMeta> = accounts.into();
        assert_eq!(metas.len(), 4);
        for (i, meta) in metas.iter().enumerate() {
            assert_eq!(meta.pubkey, keys[i]);
        }

        // Slots 0-2 are writable, not signers.
        for meta in &metas[..3] {
            assert!(meta.is_writable);
            assert!(!meta.is_signer);
        }

        // Slot 3 (rev-distr Distribution) is read-only signer.
        assert!(!metas[3].is_writable);
        assert!(metas[3].is_signer);
    }
}
