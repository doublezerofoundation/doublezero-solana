pub mod account;

//

use std::io;

use borsh::{BorshDeserialize, BorshSerialize};
use doublezero_program_tools::{Discriminator, DISCRIMINATOR_LEN};
use solana_hash::Hash;
use solana_pubkey::Pubkey;

use crate::types::EpochDuration;

pub trait ConfigureProgramInstructionData {
    fn into_instruction_data(self) -> RevenueDistributionInstructionData;
}

#[derive(Debug, BorshDeserialize, BorshSerialize, Clone, PartialEq, Eq)]
pub enum ProgramConfiguration {
    Flag(ConfigureFlag),
    Accountant(Pubkey),
    Sol2zSwapProgram(Pubkey),
    SolanaValidatorFee(u16),
    CalculationGracePeriodSeconds(u32),
    CommunityBurnRateParameters {
        limit: u32,
        dz_epochs_to_increasing: EpochDuration,
        dz_epochs_to_limit: EpochDuration,
        initial_rate: Option<u32>,
    },
}

impl ConfigureProgramInstructionData for ProgramConfiguration {
    fn into_instruction_data(self) -> RevenueDistributionInstructionData {
        RevenueDistributionInstructionData::ConfigureProgram(self)
    }
}

#[derive(Debug, BorshDeserialize, BorshSerialize, Clone, PartialEq, Eq)]
pub enum PrepaidConnectionProgramSetting {
    ActivationCost(u32),
    CostPerDoubleZeroEpoch(u32),
    TerminationRelayLamports(u32),
}

impl ConfigureProgramInstructionData for PrepaidConnectionProgramSetting {
    fn into_instruction_data(self) -> RevenueDistributionInstructionData {
        RevenueDistributionInstructionData::ConfigurePrepaidConnectionProgramSetting(self)
    }
}

#[derive(Debug, BorshDeserialize, BorshSerialize, Clone, PartialEq, Eq)]
pub enum ConfigureFlag {
    IsPaused(bool),
}

#[derive(Debug, BorshDeserialize, BorshSerialize, Clone, PartialEq, Eq)]
pub enum DistributionConfiguration {
    SolanaValidatorPayments {
        total_owed: u64,
        merkle_root: Hash,
    },
    ContributorRewards {
        total_contributors: u32,
        merkle_root: Hash,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevenueDistributionInstructionData {
    InitializeProgram,
    SetAdmin(Pubkey),
    ConfigureProgram(ProgramConfiguration),
    ConfigurePrepaidConnectionProgramSetting(PrepaidConnectionProgramSetting),
    InitializeJournal,
    InitializeDistribution,
    ConfigureDistribution(DistributionConfiguration),
    InitializePrepaidConnection {
        user_key: Pubkey,
        decimals: u8,
    },
    LoadPrepaidConnection {
        dz_epoch_duration: EpochDuration,
        decimals: u8,
    },
    TerminatePrepaidConnection,
}

impl RevenueDistributionInstructionData {
    pub const INITIALIZE_PROGRAM: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::initialize_program");
    pub const SET_ADMIN: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::set_admin");
    pub const CONFIGURE_PROGRAM: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::configure_program");
    pub const CONFIGURE_PREPAID_CONNECTION_PROGRAM_SETTING: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::configure_prepaid_connection_program_setting");
    pub const INITIALIZE_JOURNAL: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::initialize_journal");
    pub const INITIALIZE_DISTRIBUTION: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::initialize_distribution");
    pub const CONFIGURE_DISTRIBUTION: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::configure_distribution");
    pub const INITIALIZE_PREPAID_CONNECTION: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::initialize_prepaid_connection");
    pub const LOAD_PREPAID_CONNECTION: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::load_prepaid_connection");
    pub const TERMINATE_PREPAID_CONNECTION: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::terminate_prepaid_connection");
}

impl BorshDeserialize for RevenueDistributionInstructionData {
    fn deserialize_reader<R: io::Read>(reader: &mut R) -> std::io::Result<Self> {
        match Discriminator::deserialize_reader(reader)? {
            Self::INITIALIZE_PROGRAM => Ok(Self::InitializeProgram),
            Self::SET_ADMIN => BorshDeserialize::deserialize_reader(reader).map(Self::SetAdmin),
            Self::CONFIGURE_PROGRAM => {
                BorshDeserialize::deserialize_reader(reader).map(Self::ConfigureProgram)
            }
            Self::CONFIGURE_PREPAID_CONNECTION_PROGRAM_SETTING => {
                BorshDeserialize::deserialize_reader(reader)
                    .map(Self::ConfigurePrepaidConnectionProgramSetting)
            }
            Self::INITIALIZE_JOURNAL => Ok(Self::InitializeJournal),
            Self::INITIALIZE_DISTRIBUTION => Ok(Self::InitializeDistribution),
            Self::CONFIGURE_DISTRIBUTION => DistributionConfiguration::deserialize_reader(reader)
                .map(Self::ConfigureDistribution),
            Self::INITIALIZE_PREPAID_CONNECTION => {
                let user_key = BorshDeserialize::deserialize_reader(reader)?;
                let decimals = BorshDeserialize::deserialize_reader(reader)?;

                Ok(Self::InitializePrepaidConnection { user_key, decimals })
            }
            Self::LOAD_PREPAID_CONNECTION => {
                let dz_epoch_duration = BorshDeserialize::deserialize_reader(reader)?;
                let decimals = BorshDeserialize::deserialize_reader(reader)?;

                Ok(Self::LoadPrepaidConnection {
                    dz_epoch_duration,
                    decimals,
                })
            }
            Self::TERMINATE_PREPAID_CONNECTION => Ok(Self::TerminatePrepaidConnection),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid discriminator",
            )),
        }
    }
}

impl BorshSerialize for RevenueDistributionInstructionData {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::InitializeProgram => Self::INITIALIZE_PROGRAM.serialize(writer),
            Self::SetAdmin(key) => {
                Self::SET_ADMIN.serialize(writer)?;
                key.serialize(writer)
            }
            Self::ConfigureProgram(setting) => {
                Self::CONFIGURE_PROGRAM.serialize(writer)?;
                setting.serialize(writer)
            }
            Self::ConfigurePrepaidConnectionProgramSetting(setting) => {
                Self::CONFIGURE_PREPAID_CONNECTION_PROGRAM_SETTING.serialize(writer)?;
                setting.serialize(writer)
            }
            Self::InitializeJournal => Self::INITIALIZE_JOURNAL.serialize(writer),
            Self::InitializeDistribution => Self::INITIALIZE_DISTRIBUTION.serialize(writer),
            Self::ConfigureDistribution(setting) => {
                Self::CONFIGURE_DISTRIBUTION.serialize(writer)?;
                setting.serialize(writer)
            }
            Self::InitializePrepaidConnection { user_key, decimals } => {
                Self::INITIALIZE_PREPAID_CONNECTION.serialize(writer)?;
                user_key.serialize(writer)?;
                decimals.serialize(writer)
            }
            Self::LoadPrepaidConnection {
                dz_epoch_duration,
                decimals,
            } => {
                Self::LOAD_PREPAID_CONNECTION.serialize(writer)?;
                dz_epoch_duration.serialize(writer)?;
                decimals.serialize(writer)
            }
            Self::TerminatePrepaidConnection => {
                Self::TERMINATE_PREPAID_CONNECTION.serialize(writer)
            }
        }
    }
}
