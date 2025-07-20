use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;

use crate::{
    state::{find_custodied_2z_address, Distribution, Journal, ProgramConfig},
    types::DoubleZeroEpoch,
    DOUBLEZERO_MINT_KEY,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializeProgramAccounts {
    pub payer_key: Pubkey,
    pub new_program_config_key: Pubkey,
}

impl InitializeProgramAccounts {
    pub fn new(payer_key: Pubkey) -> Self {
        Self {
            payer_key,
            new_program_config_key: ProgramConfig::find_address().0,
        }
    }
}

impl From<InitializeProgramAccounts> for Vec<AccountMeta> {
    fn from(accounts: InitializeProgramAccounts) -> Self {
        let InitializeProgramAccounts {
            payer_key,
            new_program_config_key,
        } = accounts;

        vec![
            AccountMeta::new(payer_key, true),
            AccountMeta::new(new_program_config_key, false),
            AccountMeta::new_readonly(solana_system_interface::program::ID, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetAdminAccounts {
    pub program_data_key: Pubkey,
    pub owner_key: Pubkey,
    pub program_config_key: Pubkey,
}

impl SetAdminAccounts {
    pub fn new(program_data_key: Pubkey, owner_key: Pubkey) -> Self {
        Self {
            program_data_key,
            owner_key,
            program_config_key: ProgramConfig::find_address().0,
        }
    }
}

impl From<SetAdminAccounts> for Vec<AccountMeta> {
    fn from(accounts: SetAdminAccounts) -> Self {
        let SetAdminAccounts {
            program_data_key,
            owner_key,
            program_config_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_data_key, false),
            AccountMeta::new_readonly(owner_key, true),
            AccountMeta::new(program_config_key, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigureProgramAccounts {
    pub program_config_key: Pubkey,
    pub admin_key: Pubkey,
}

impl ConfigureProgramAccounts {
    pub fn new(admin_key: Pubkey) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            admin_key,
        }
    }
}

impl From<ConfigureProgramAccounts> for Vec<AccountMeta> {
    fn from(accounts: ConfigureProgramAccounts) -> Self {
        let ConfigureProgramAccounts {
            program_config_key,
            admin_key,
        } = accounts;

        vec![
            AccountMeta::new(program_config_key, false),
            AccountMeta::new_readonly(admin_key, true),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializeJournalAccounts {
    pub payer_key: Pubkey,
    pub new_journal_key: Pubkey,
    pub journal_custodied_2z_key: Pubkey,
}

impl InitializeJournalAccounts {
    pub fn new(payer_key: Pubkey) -> Self {
        let new_journal_key = Journal::find_address().0;

        Self {
            payer_key,
            new_journal_key,
            journal_custodied_2z_key: find_custodied_2z_address(&new_journal_key).0,
        }
    }
}

impl From<InitializeJournalAccounts> for Vec<AccountMeta> {
    fn from(accounts: InitializeJournalAccounts) -> Self {
        let InitializeJournalAccounts {
            payer_key,
            new_journal_key,
            journal_custodied_2z_key,
        } = accounts;

        vec![
            AccountMeta::new(payer_key, true),
            AccountMeta::new(new_journal_key, false),
            AccountMeta::new(journal_custodied_2z_key, false),
            AccountMeta::new_readonly(DOUBLEZERO_MINT_KEY, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_system_interface::program::ID, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializeDistributionAccounts {
    pub program_config_key: Pubkey,
    pub accountant_key: Pubkey,
    pub payer_key: Pubkey,
    pub new_distribution_key: Pubkey,
    pub distribution_custodied_2z_key: Pubkey,
}

impl InitializeDistributionAccounts {
    pub fn new(accountant_key: Pubkey, payer_key: Pubkey, dz_epoch: DoubleZeroEpoch) -> Self {
        let new_distribution_key = Distribution::find_address(dz_epoch).0;

        Self {
            program_config_key: ProgramConfig::find_address().0,
            accountant_key,
            payer_key,
            new_distribution_key,
            distribution_custodied_2z_key: find_custodied_2z_address(&new_distribution_key).0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigureDistributionAccounts {
    pub program_config_key: Pubkey,
    pub accountant_key: Pubkey,
    pub distribution_key: Pubkey,
}

impl ConfigureDistributionAccounts {
    pub fn new(accountant_key: Pubkey, dz_epoch: DoubleZeroEpoch) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            accountant_key,
            distribution_key: Distribution::find_address(dz_epoch).0,
        }
    }
}

impl From<ConfigureDistributionAccounts> for Vec<AccountMeta> {
    fn from(accounts: ConfigureDistributionAccounts) -> Self {
        let ConfigureDistributionAccounts {
            program_config_key,
            accountant_key,
            distribution_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new_readonly(accountant_key, true),
            AccountMeta::new(distribution_key, false),
        ]
    }
}

impl From<InitializeDistributionAccounts> for Vec<AccountMeta> {
    fn from(accounts: InitializeDistributionAccounts) -> Self {
        let InitializeDistributionAccounts {
            program_config_key,
            accountant_key,
            payer_key,
            new_distribution_key,
            distribution_custodied_2z_key,
        } = accounts;

        vec![
            AccountMeta::new(program_config_key, false),
            AccountMeta::new_readonly(accountant_key, true),
            AccountMeta::new(payer_key, true),
            AccountMeta::new(new_distribution_key, false),
            AccountMeta::new(distribution_custodied_2z_key, false),
            AccountMeta::new_readonly(DOUBLEZERO_MINT_KEY, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_system_interface::program::ID, false),
        ]
    }
}
