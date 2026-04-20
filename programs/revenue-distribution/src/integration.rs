use borsh::{BorshDeserialize, BorshSerialize};
use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;

/// Instructions rev-distr CPIs integration programs with. Integration
/// programs deserialize this before their own instruction enum.
#[derive(Debug, Clone, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
pub enum IntegrationInstructionData {
    /// Transfer the epoch's contributor-share 2Z from the integration's
    /// bucket to the destination and flip `is_collected = true`. See
    /// [`IntegrationAccounts`] for the account list.
    WithdrawIntegrationRewards,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationAccounts {
    pub integration_distribution_key: Pubkey,
    pub integration_2z_bucket_key: Pubkey,
    pub destination_token_account_key: Pubkey,
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
