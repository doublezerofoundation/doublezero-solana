use std::io;

use borsh::{BorshDeserialize, BorshSerialize};
use doublezero_program_tools::{Discriminator, DISCRIMINATOR_LEN};
use solana_hash::Hash;
use solana_pubkey::Pubkey;

#[derive(Debug, BorshDeserialize, BorshSerialize, Clone, PartialEq)]
pub enum ConfigureProgramSetting {
    Flag(ConfigureFlag),
    Accountant(Pubkey),
    Sol2zSwapProgram(Pubkey),
    SolanaValidatorFee(u16),
    CalculationGracePeriodSeconds(u32),
    CommunityBurnRateParameters {
        limit: u32,
        dz_epochs_to_increasing: u32,
        dz_epochs_to_limit: u32,
        initial_rate: Option<u32>,
    },
}

#[derive(Debug, BorshDeserialize, BorshSerialize, Clone, PartialEq)]
pub enum ConfigureFlag {
    IsPaused(bool),
}

#[derive(Debug, BorshDeserialize, BorshSerialize, Clone, PartialEq)]
pub enum ConfigureDistributionData {
    SolanaValidatorPayments {
        total_owed: u64,
        merkle_root: Hash,
    },
    ContributorRewards {
        total_contributors: u32,
        merkle_root: Hash,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum RevenueDistributionInstructionData {
    InitializeProgram,
    SetAdmin(Pubkey),
    ConfigureProgram(ConfigureProgramSetting),
    InitializeJournal,
    InitializeDistribution,
    ConfigureDistribution(ConfigureDistributionData),
    InitializePrepaidUser(Pubkey),
}

impl TryFrom<RevenueDistributionInstructionData> for Vec<u8> {
    type Error = io::Error;

    fn try_from(ix_data: RevenueDistributionInstructionData) -> Result<Self, Self::Error> {
        borsh::to_vec(&ix_data)
    }
}

impl RevenueDistributionInstructionData {
    pub const INITIALIZE_PROGRAM: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::initialize_program");
    pub const SET_ADMIN: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::set_admin");
    pub const CONFIGURE_PROGRAM: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::configure_program");
    pub const INITIALIZE_JOURNAL: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::initialize_journal");
    pub const INITIALIZE_DISTRIBUTION: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::initialize_distribution");
    pub const CONFIGURE_DISTRIBUTION: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::configure_distribution");
    pub const INITIALIZE_PREPAID_USER: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new_sha2(b"dz::ix::initialize_prepaid_user");
}

impl BorshDeserialize for RevenueDistributionInstructionData {
    fn deserialize_reader<R: io::Read>(reader: &mut R) -> std::io::Result<Self> {
        match Discriminator::deserialize_reader(reader)? {
            Self::INITIALIZE_PROGRAM => Ok(Self::InitializeProgram),
            Self::SET_ADMIN => Pubkey::deserialize_reader(reader).map(Self::SetAdmin),
            Self::CONFIGURE_PROGRAM => {
                ConfigureProgramSetting::deserialize_reader(reader).map(Self::ConfigureProgram)
            }
            Self::INITIALIZE_JOURNAL => Ok(Self::InitializeJournal),
            Self::INITIALIZE_DISTRIBUTION => Ok(Self::InitializeDistribution),
            Self::CONFIGURE_DISTRIBUTION => ConfigureDistributionData::deserialize_reader(reader)
                .map(Self::ConfigureDistribution),
            Self::INITIALIZE_PREPAID_USER => {
                Pubkey::deserialize_reader(reader).map(Self::InitializePrepaidUser)
            }
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
            Self::InitializeJournal => Self::INITIALIZE_JOURNAL.serialize(writer),
            Self::InitializeDistribution => Self::INITIALIZE_DISTRIBUTION.serialize(writer),
            Self::ConfigureDistribution(data) => {
                Self::CONFIGURE_DISTRIBUTION.serialize(writer)?;
                data.serialize(writer)
            }
            Self::InitializePrepaidUser(key) => {
                Self::INITIALIZE_PREPAID_USER.serialize(writer)?;
                key.serialize(writer)
            }
        }
    }
}
