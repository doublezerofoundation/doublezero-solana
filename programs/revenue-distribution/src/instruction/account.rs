use doublezero_program_tools::get_program_data_address;
use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;
use solana_system_interface::program as system_program;

use crate::{
    state::{
        find_2z_token_pda_address, find_swap_authority_address, ContributorRewards, Distribution,
        Journal, PrepaidConnection, ProgramConfig, SolanaValidatorDeposit,
    },
    types::DoubleZeroEpoch,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializeProgramAccounts {
    pub payer_key: Pubkey,
    pub new_program_config_key: Pubkey,
    pub reserve_2z_key: Pubkey,
    pub dz_mint_key: Pubkey,
}

impl InitializeProgramAccounts {
    pub fn new(payer_key: &Pubkey, dz_mint_key: &Pubkey) -> Self {
        let new_program_config_key = ProgramConfig::find_address().0;

        Self {
            payer_key: *payer_key,
            new_program_config_key,
            reserve_2z_key: find_2z_token_pda_address(&new_program_config_key).0,
            dz_mint_key: *dz_mint_key,
        }
    }
}

impl From<InitializeProgramAccounts> for Vec<AccountMeta> {
    fn from(accounts: InitializeProgramAccounts) -> Self {
        let InitializeProgramAccounts {
            payer_key,
            new_program_config_key,
            reserve_2z_key,
            dz_mint_key,
        } = accounts;

        vec![
            AccountMeta::new(payer_key, true),
            AccountMeta::new(new_program_config_key, false),
            AccountMeta::new(reserve_2z_key, false),
            AccountMeta::new_readonly(dz_mint_key, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrateProgramAccounts {
    pub program_data_key: Pubkey,
    pub owner_key: Pubkey,
    pub program_config_key: Pubkey,
}

impl MigrateProgramAccounts {
    pub fn new(program_id: &Pubkey, owner_key: &Pubkey) -> Self {
        Self {
            program_data_key: get_program_data_address(program_id).0,
            owner_key: *owner_key,
            program_config_key: ProgramConfig::find_address().0,
        }
    }
}

impl From<MigrateProgramAccounts> for Vec<AccountMeta> {
    fn from(accounts: MigrateProgramAccounts) -> Self {
        let MigrateProgramAccounts {
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
pub struct SetAdminAccounts {
    pub program_data_key: Pubkey,
    pub owner_key: Pubkey,
    pub program_config_key: Pubkey,
}

impl SetAdminAccounts {
    pub fn new(program_id: &Pubkey, owner_key: &Pubkey) -> Self {
        Self {
            program_data_key: get_program_data_address(program_id).0,
            owner_key: *owner_key,
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
    pub fn new(admin_key: &Pubkey) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            admin_key: *admin_key,
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
    pub journal_2z_token_pda_key: Pubkey,
    pub dz_mint_key: Pubkey,
}

impl InitializeJournalAccounts {
    pub fn new(payer_key: &Pubkey, dz_mint_key: &Pubkey) -> Self {
        let new_journal_key = Journal::find_address().0;

        Self {
            payer_key: *payer_key,
            new_journal_key,
            journal_2z_token_pda_key: find_2z_token_pda_address(&new_journal_key).0,
            dz_mint_key: *dz_mint_key,
        }
    }
}

impl From<InitializeJournalAccounts> for Vec<AccountMeta> {
    fn from(accounts: InitializeJournalAccounts) -> Self {
        let InitializeJournalAccounts {
            payer_key,
            new_journal_key,
            journal_2z_token_pda_key,
            dz_mint_key,
        } = accounts;

        vec![
            AccountMeta::new(payer_key, true),
            AccountMeta::new(new_journal_key, false),
            AccountMeta::new(journal_2z_token_pda_key, false),
            AccountMeta::new_readonly(dz_mint_key, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigureJournalAccounts {
    pub program_config_key: Pubkey,
    pub admin_key: Pubkey,
    pub journal_key: Pubkey,
}

impl ConfigureJournalAccounts {
    pub fn new(admin_key: &Pubkey) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            admin_key: *admin_key,
            journal_key: Journal::find_address().0,
        }
    }
}

impl From<ConfigureJournalAccounts> for Vec<AccountMeta> {
    fn from(accounts: ConfigureJournalAccounts) -> Self {
        let ConfigureJournalAccounts {
            program_config_key,
            admin_key,
            journal_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new_readonly(admin_key, true),
            AccountMeta::new(journal_key, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializeDistributionAccounts {
    pub program_config_key: Pubkey,
    pub accountant_key: Pubkey,
    pub payer_key: Pubkey,
    pub new_distribution_key: Pubkey,
    pub distribution_2z_token_pda_key: Pubkey,
    pub dz_mint_key: Pubkey,
    pub journal_key: Pubkey,
    pub journal_2z_token_pda_key: Pubkey,
}

impl InitializeDistributionAccounts {
    pub fn new(
        accountant_key: &Pubkey,
        payer_key: &Pubkey,
        dz_epoch: DoubleZeroEpoch,
        dz_mint_key: &Pubkey,
    ) -> Self {
        let new_distribution_key = Distribution::find_address(dz_epoch).0;
        let journal_key = Journal::find_address().0;

        Self {
            program_config_key: ProgramConfig::find_address().0,
            accountant_key: *accountant_key,
            payer_key: *payer_key,
            new_distribution_key,
            distribution_2z_token_pda_key: find_2z_token_pda_address(&new_distribution_key).0,
            dz_mint_key: *dz_mint_key,
            journal_key,
            journal_2z_token_pda_key: find_2z_token_pda_address(&journal_key).0,
        }
    }
}

impl From<InitializeDistributionAccounts> for Vec<AccountMeta> {
    fn from(accounts: InitializeDistributionAccounts) -> Self {
        let InitializeDistributionAccounts {
            program_config_key,
            accountant_key,
            payer_key,
            new_distribution_key,
            distribution_2z_token_pda_key,
            dz_mint_key,
            journal_key,
            journal_2z_token_pda_key,
        } = accounts;

        vec![
            AccountMeta::new(program_config_key, false),
            AccountMeta::new_readonly(accountant_key, true),
            AccountMeta::new(payer_key, true),
            AccountMeta::new(new_distribution_key, false),
            AccountMeta::new(distribution_2z_token_pda_key, false),
            AccountMeta::new_readonly(dz_mint_key, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new(journal_key, false),
            AccountMeta::new(journal_2z_token_pda_key, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigureDistributionDebtAccounts {
    pub program_config_key: Pubkey,
    pub payments_accountant_key: Pubkey,
    pub distribution_key: Pubkey,
}

impl ConfigureDistributionDebtAccounts {
    pub fn new(payments_accountant_key: &Pubkey, dz_epoch: DoubleZeroEpoch) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            payments_accountant_key: *payments_accountant_key,
            distribution_key: Distribution::find_address(dz_epoch).0,
        }
    }
}

impl From<ConfigureDistributionDebtAccounts> for Vec<AccountMeta> {
    fn from(accounts: ConfigureDistributionDebtAccounts) -> Self {
        let ConfigureDistributionDebtAccounts {
            program_config_key,
            payments_accountant_key,
            distribution_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new_readonly(payments_accountant_key, true),
            AccountMeta::new(distribution_key, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizeDistributionDebtAccounts {
    pub program_config_key: Pubkey,
    pub payments_accountant_key: Pubkey,
    pub distribution_key: Pubkey,
    pub payer_key: Pubkey,
}

impl FinalizeDistributionDebtAccounts {
    pub fn new(
        payments_accountant_key: &Pubkey,
        dz_epoch: DoubleZeroEpoch,
        payer_key: &Pubkey,
    ) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            payments_accountant_key: *payments_accountant_key,
            distribution_key: Distribution::find_address(dz_epoch).0,
            payer_key: *payer_key,
        }
    }
}

impl From<FinalizeDistributionDebtAccounts> for Vec<AccountMeta> {
    fn from(accounts: FinalizeDistributionDebtAccounts) -> Self {
        let FinalizeDistributionDebtAccounts {
            program_config_key,
            payments_accountant_key,
            distribution_key,
            payer_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new_readonly(payments_accountant_key, true),
            AccountMeta::new(distribution_key, false),
            AccountMeta::new(payer_key, true),
            AccountMeta::new_readonly(system_program::ID, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigureDistributionRewardsAccounts {
    pub program_config_key: Pubkey,
    pub accountant_key: Pubkey,
    pub distribution_key: Pubkey,
}

impl ConfigureDistributionRewardsAccounts {
    pub fn new(accountant_key: &Pubkey, dz_epoch: DoubleZeroEpoch) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            accountant_key: *accountant_key,
            distribution_key: Distribution::find_address(dz_epoch).0,
        }
    }
}

impl From<ConfigureDistributionRewardsAccounts> for Vec<AccountMeta> {
    fn from(accounts: ConfigureDistributionRewardsAccounts) -> Self {
        let ConfigureDistributionRewardsAccounts {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizeDistributionRewardsAccounts {
    pub program_config_key: Pubkey,
    pub distribution_key: Pubkey,
    pub payer_key: Pubkey,
}

impl FinalizeDistributionRewardsAccounts {
    pub fn new(payer_key: &Pubkey, dz_epoch: DoubleZeroEpoch) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            distribution_key: Distribution::find_address(dz_epoch).0,
            payer_key: *payer_key,
        }
    }
}

impl From<FinalizeDistributionRewardsAccounts> for Vec<AccountMeta> {
    fn from(accounts: FinalizeDistributionRewardsAccounts) -> Self {
        let FinalizeDistributionRewardsAccounts {
            program_config_key,
            distribution_key,
            payer_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new(distribution_key, false),
            AccountMeta::new(payer_key, true),
            AccountMeta::new_readonly(system_program::ID, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializePrepaidConnectionAccounts {
    pub program_config_key: Pubkey,
    pub journal_key: Pubkey,
    pub source_2z_token_account_key: Pubkey,
    pub dz_mint_key: Pubkey,
    pub token_transfer_authority_key: Pubkey,
    pub reserve_2z_key: Pubkey,
    pub payer_key: Pubkey,
    pub new_prepaid_connection_key: Pubkey,
}

impl InitializePrepaidConnectionAccounts {
    pub fn new(
        source_2z_token_account_key: &Pubkey,
        dz_mint_key: &Pubkey,
        token_transfer_authority_key: &Pubkey,
        payer_key: &Pubkey,
        user_key: &Pubkey,
    ) -> Self {
        let program_config_key = ProgramConfig::find_address().0;

        Self {
            program_config_key,
            journal_key: Journal::find_address().0,
            source_2z_token_account_key: *source_2z_token_account_key,
            dz_mint_key: *dz_mint_key,
            token_transfer_authority_key: *token_transfer_authority_key,
            reserve_2z_key: find_2z_token_pda_address(&program_config_key).0,
            payer_key: *payer_key,
            new_prepaid_connection_key: PrepaidConnection::find_address(user_key).0,
        }
    }
}

impl From<InitializePrepaidConnectionAccounts> for Vec<AccountMeta> {
    fn from(accounts: InitializePrepaidConnectionAccounts) -> Self {
        let InitializePrepaidConnectionAccounts {
            program_config_key,
            journal_key,
            source_2z_token_account_key,
            dz_mint_key,
            token_transfer_authority_key,
            reserve_2z_key,
            payer_key,
            new_prepaid_connection_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new_readonly(journal_key, false),
            AccountMeta::new(source_2z_token_account_key, false),
            AccountMeta::new_readonly(dz_mint_key, false),
            AccountMeta::new(reserve_2z_key, false),
            AccountMeta::new_readonly(token_transfer_authority_key, true),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new(payer_key, true),
            AccountMeta::new(new_prepaid_connection_key, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantPrepaidConnectionAccessAccounts {
    pub program_config_key: Pubkey,
    pub dz_ledger_sentinel_key: Pubkey,
    pub prepaid_connection_key: Pubkey,
}

impl GrantPrepaidConnectionAccessAccounts {
    pub fn new(dz_ledger_sentinel_key: &Pubkey, user_key: &Pubkey) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            dz_ledger_sentinel_key: *dz_ledger_sentinel_key,
            prepaid_connection_key: PrepaidConnection::find_address(user_key).0,
        }
    }
}

impl From<GrantPrepaidConnectionAccessAccounts> for Vec<AccountMeta> {
    fn from(accounts: GrantPrepaidConnectionAccessAccounts) -> Self {
        let GrantPrepaidConnectionAccessAccounts {
            program_config_key,
            dz_ledger_sentinel_key,
            prepaid_connection_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new_readonly(dz_ledger_sentinel_key, true),
            AccountMeta::new(prepaid_connection_key, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenyPrepaidConnectionAccessAccounts {
    pub program_config_key: Pubkey,
    pub dz_ledger_sentinel_key: Pubkey,
    pub prepaid_connection_key: Pubkey,
    pub reserve_2z_key: Pubkey,
    pub activation_funder_key: Pubkey,
    pub termination_beneficiary_key: Pubkey,
}

impl DenyPrepaidConnectionAccessAccounts {
    pub fn new(
        dz_ledger_sentinel_key: &Pubkey,
        activation_funder_key: &Pubkey,
        termination_beneficiary_key: &Pubkey,
        user_key: &Pubkey,
    ) -> Self {
        let program_config_key = ProgramConfig::find_address().0;

        Self {
            program_config_key,
            dz_ledger_sentinel_key: *dz_ledger_sentinel_key,
            prepaid_connection_key: PrepaidConnection::find_address(user_key).0,
            reserve_2z_key: find_2z_token_pda_address(&program_config_key).0,
            activation_funder_key: *activation_funder_key,
            termination_beneficiary_key: *termination_beneficiary_key,
        }
    }
}

impl From<DenyPrepaidConnectionAccessAccounts> for Vec<AccountMeta> {
    fn from(accounts: DenyPrepaidConnectionAccessAccounts) -> Self {
        let DenyPrepaidConnectionAccessAccounts {
            program_config_key,
            dz_ledger_sentinel_key,
            prepaid_connection_key,
            reserve_2z_key,
            activation_funder_key,
            termination_beneficiary_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new(dz_ledger_sentinel_key, true),
            AccountMeta::new(prepaid_connection_key, false),
            AccountMeta::new(reserve_2z_key, false),
            AccountMeta::new(activation_funder_key, false),
            AccountMeta::new(termination_beneficiary_key, false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadPrepaidConnectionAccounts {
    pub program_config_key: Pubkey,
    pub journal_key: Pubkey,
    pub prepaid_connection_key: Pubkey,
    pub source_2z_token_account_key: Pubkey,
    pub dz_mint_key: Pubkey,
    pub journal_2z_token_pda_key: Pubkey,
    pub token_transfer_authority_key: Pubkey,
}

impl LoadPrepaidConnectionAccounts {
    pub fn new(
        source_2z_token_account_key: &Pubkey,
        dz_mint_key: &Pubkey,
        token_transfer_authority_key: &Pubkey,
        user_key: &Pubkey,
    ) -> Self {
        let program_config_key = ProgramConfig::find_address().0;
        let journal_key = Journal::find_address().0;

        Self {
            program_config_key,
            journal_key,
            prepaid_connection_key: PrepaidConnection::find_address(user_key).0,
            source_2z_token_account_key: *source_2z_token_account_key,
            dz_mint_key: *dz_mint_key,
            journal_2z_token_pda_key: find_2z_token_pda_address(&journal_key).0,
            token_transfer_authority_key: *token_transfer_authority_key,
        }
    }
}

impl From<LoadPrepaidConnectionAccounts> for Vec<AccountMeta> {
    fn from(accounts: LoadPrepaidConnectionAccounts) -> Self {
        let LoadPrepaidConnectionAccounts {
            program_config_key,
            journal_key,
            prepaid_connection_key,
            source_2z_token_account_key,
            dz_mint_key,
            journal_2z_token_pda_key,
            token_transfer_authority_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new(journal_key, false),
            AccountMeta::new(prepaid_connection_key, false),
            AccountMeta::new(source_2z_token_account_key, false),
            AccountMeta::new_readonly(dz_mint_key, false),
            AccountMeta::new(journal_2z_token_pda_key, false),
            AccountMeta::new_readonly(token_transfer_authority_key, true),
            AccountMeta::new_readonly(spl_token::ID, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminatePrepaidConnectionAccounts {
    pub program_config_key: Pubkey,
    pub prepaid_connection_key: Pubkey,
    pub termination_relayer_key: Pubkey,
    pub termination_beneficiary_key: Pubkey,
}

impl TerminatePrepaidConnectionAccounts {
    pub fn new(
        user_key: &Pubkey,
        termination_beneficiary_key: &Pubkey,
        termination_relayer_key: Option<&Pubkey>,
    ) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            prepaid_connection_key: PrepaidConnection::find_address(user_key).0,
            termination_relayer_key: *termination_relayer_key
                .unwrap_or(termination_beneficiary_key),
            termination_beneficiary_key: *termination_beneficiary_key,
        }
    }
}

impl From<TerminatePrepaidConnectionAccounts> for Vec<AccountMeta> {
    fn from(accounts: TerminatePrepaidConnectionAccounts) -> Self {
        let TerminatePrepaidConnectionAccounts {
            program_config_key,
            prepaid_connection_key,
            termination_relayer_key,
            termination_beneficiary_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new(prepaid_connection_key, false),
            AccountMeta::new(termination_relayer_key, false),
            AccountMeta::new(termination_beneficiary_key, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializeContributorRewardsAccounts {
    pub payer_key: Pubkey,
    pub new_contributor_rewards_key: Pubkey,
}

impl InitializeContributorRewardsAccounts {
    pub fn new(payer_key: &Pubkey, service_key: &Pubkey) -> Self {
        Self {
            payer_key: *payer_key,
            new_contributor_rewards_key: ContributorRewards::find_address(service_key).0,
        }
    }
}

impl From<InitializeContributorRewardsAccounts> for Vec<AccountMeta> {
    fn from(accounts: InitializeContributorRewardsAccounts) -> Self {
        let InitializeContributorRewardsAccounts {
            payer_key,
            new_contributor_rewards_key,
        } = accounts;

        vec![
            AccountMeta::new(payer_key, true),
            AccountMeta::new(new_contributor_rewards_key, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetRewardsManagerAccounts {
    pub program_config_key: Pubkey,
    pub contributor_manager_key: Pubkey,
    pub contributor_rewards_key: Pubkey,
}

impl SetRewardsManagerAccounts {
    pub fn new(contributor_manager_key: &Pubkey, service_key: &Pubkey) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            contributor_manager_key: *contributor_manager_key,
            contributor_rewards_key: ContributorRewards::find_address(service_key).0,
        }
    }
}

impl From<SetRewardsManagerAccounts> for Vec<AccountMeta> {
    fn from(accounts: SetRewardsManagerAccounts) -> Self {
        let SetRewardsManagerAccounts {
            program_config_key,
            contributor_manager_key,
            contributor_rewards_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new_readonly(contributor_manager_key, true),
            AccountMeta::new(contributor_rewards_key, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigureContributorRewardsAccounts {
    pub program_config_key: Pubkey,
    pub contributor_rewards_key: Pubkey,
    pub rewards_manager_key: Pubkey,
}

impl ConfigureContributorRewardsAccounts {
    pub fn new(rewards_manager_key: &Pubkey, service_key: &Pubkey) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            contributor_rewards_key: ContributorRewards::find_address(service_key).0,
            rewards_manager_key: *rewards_manager_key,
        }
    }
}

impl From<ConfigureContributorRewardsAccounts> for Vec<AccountMeta> {
    fn from(accounts: ConfigureContributorRewardsAccounts) -> Self {
        let ConfigureContributorRewardsAccounts {
            program_config_key,
            contributor_rewards_key,
            rewards_manager_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new(contributor_rewards_key, false),
            AccountMeta::new_readonly(rewards_manager_key, true),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyDistributionMerkleRootAccounts {
    pub distribution_key: Pubkey,
}

impl VerifyDistributionMerkleRootAccounts {
    pub fn new(dz_epoch: DoubleZeroEpoch) -> Self {
        Self {
            distribution_key: Distribution::find_address(dz_epoch).0,
        }
    }
}

impl From<VerifyDistributionMerkleRootAccounts> for Vec<AccountMeta> {
    fn from(accounts: VerifyDistributionMerkleRootAccounts) -> Self {
        let VerifyDistributionMerkleRootAccounts { distribution_key } = accounts;

        vec![AccountMeta::new_readonly(distribution_key, false)]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializeSolanaValidatorDepositAccounts {
    pub new_solana_validator_deposit_key: Pubkey,
    pub payer_key: Pubkey,
}

impl InitializeSolanaValidatorDepositAccounts {
    pub fn new(payer_key: &Pubkey, node_id: &Pubkey) -> Self {
        Self {
            new_solana_validator_deposit_key: SolanaValidatorDeposit::find_address(node_id).0,
            payer_key: *payer_key,
        }
    }
}

impl From<InitializeSolanaValidatorDepositAccounts> for Vec<AccountMeta> {
    fn from(accounts: InitializeSolanaValidatorDepositAccounts) -> Self {
        let InitializeSolanaValidatorDepositAccounts {
            new_solana_validator_deposit_key,
            payer_key,
        } = accounts;

        vec![
            AccountMeta::new(new_solana_validator_deposit_key, false),
            AccountMeta::new(payer_key, true),
            AccountMeta::new_readonly(system_program::ID, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaySolanaValidatorDebtAccounts {
    pub program_config_key: Pubkey,
    pub distribution_key: Pubkey,
    pub solana_validator_deposit_key: Pubkey,
    pub journal_key: Pubkey,
}

impl PaySolanaValidatorDebtAccounts {
    pub fn new(dz_epoch: DoubleZeroEpoch, node_id: &Pubkey) -> Self {
        Self {
            program_config_key: ProgramConfig::find_address().0,
            distribution_key: Distribution::find_address(dz_epoch).0,
            solana_validator_deposit_key: SolanaValidatorDeposit::find_address(node_id).0,
            journal_key: Journal::find_address().0,
        }
    }
}

impl From<PaySolanaValidatorDebtAccounts> for Vec<AccountMeta> {
    fn from(accounts: PaySolanaValidatorDebtAccounts) -> Self {
        let PaySolanaValidatorDebtAccounts {
            program_config_key,
            distribution_key,
            solana_validator_deposit_key,
            journal_key,
        } = accounts;

        vec![
            AccountMeta::new_readonly(program_config_key, false),
            AccountMeta::new(distribution_key, false),
            AccountMeta::new(solana_validator_deposit_key, false),
            AccountMeta::new(journal_key, false),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializeSwapDestinationAccounts {
    pub payer_key: Pubkey,
    pub swap_authority_key: Pubkey,
    pub new_swap_destination_key: Pubkey,
    pub mint_key: Pubkey,
}

impl InitializeSwapDestinationAccounts {
    pub fn new(payer_key: &Pubkey, mint_key: &Pubkey) -> Self {
        let swap_authority_key = find_swap_authority_address().0;

        Self {
            payer_key: *payer_key,
            swap_authority_key,
            new_swap_destination_key: find_2z_token_pda_address(&swap_authority_key).0,
            mint_key: *mint_key,
        }
    }
}

impl From<InitializeSwapDestinationAccounts> for Vec<AccountMeta> {
    fn from(accounts: InitializeSwapDestinationAccounts) -> Self {
        let InitializeSwapDestinationAccounts {
            payer_key,
            swap_authority_key,
            new_swap_destination_key,
            mint_key,
        } = accounts;

        vec![
            AccountMeta::new(payer_key, true),
            AccountMeta::new_readonly(swap_authority_key, false),
            AccountMeta::new(new_swap_destination_key, false),
            AccountMeta::new_readonly(mint_key, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ]
    }
}

#[cfg(not(feature = "development"))]
mod sweep_distribution_tokens {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SweepDistributionTokensAccounts;

    impl SweepDistributionTokensAccounts {
        pub fn new(_dz_epoch: DoubleZeroEpoch) -> Self {
            Self
        }
    }

    impl From<SweepDistributionTokensAccounts> for Vec<AccountMeta> {
        fn from(_accounts: SweepDistributionTokensAccounts) -> Self {
            vec![]
        }
    }
}

#[cfg(feature = "development")]
mod sweep_distribution_tokens {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SweepDistributionTokensAccounts {
        pub program_config_key: Pubkey,
        pub distribution_key: Pubkey,
        pub journal_key: Pubkey,
        pub distribution_2z_token_pda_key: Pubkey,
        pub swap_authority_key: Pubkey,
        pub swap_2z_token_pda_key: Pubkey,
    }

    impl SweepDistributionTokensAccounts {
        pub fn new(dz_epoch: DoubleZeroEpoch) -> Self {
            let distribution_key = Distribution::find_address(dz_epoch).0;
            let swap_authority_key = find_swap_authority_address().0;

            Self {
                program_config_key: ProgramConfig::find_address().0,
                distribution_key,
                journal_key: Journal::find_address().0,
                distribution_2z_token_pda_key: find_2z_token_pda_address(&distribution_key).0,
                swap_authority_key,
                swap_2z_token_pda_key: find_2z_token_pda_address(&swap_authority_key).0,
            }
        }
    }

    impl From<SweepDistributionTokensAccounts> for Vec<AccountMeta> {
        fn from(accounts: SweepDistributionTokensAccounts) -> Self {
            let SweepDistributionTokensAccounts {
                program_config_key,
                distribution_key,
                journal_key,
                distribution_2z_token_pda_key,
                swap_authority_key,
                swap_2z_token_pda_key,
            } = accounts;

            vec![
                AccountMeta::new_readonly(program_config_key, false),
                AccountMeta::new(distribution_key, false),
                AccountMeta::new(journal_key, false),
                AccountMeta::new(distribution_2z_token_pda_key, false),
                AccountMeta::new_readonly(swap_authority_key, false),
                AccountMeta::new(swap_2z_token_pda_key, false),
                AccountMeta::new_readonly(spl_token::ID, false),
            ]
        }
    }
}

pub use sweep_distribution_tokens::*;
