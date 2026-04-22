use std::io;

use borsh::{BorshDeserialize, BorshSerialize};
use doublezero_program_tools::{
    instruction::try_build_instruction, Discriminator, DISCRIMINATOR_LEN,
};
use doublezero_revenue_distribution::types::DoubleZeroEpoch;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_system_interface::program as system_program;

use crate::{state::MockIntegrationDistribution, ID};

/// Mock integration instructions. The mock's processor first checks whether
/// incoming data starts with byte 0 (the shared
/// `IntegrationInstructionData::WithdrawIntegrationRewards` discriminator)
/// and routes to the interface handler. Any other first byte is dispatched
/// here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockRewardsIntegrationInstructionData {
    /// Create the mock's per-epoch integration distribution PDA.
    InitializeIntegrationDistribution { dz_epoch: DoubleZeroEpoch },
}

impl MockRewardsIntegrationInstructionData {
    pub const INITIALIZE_INTEGRATION_DISTRIBUTION: Discriminator<DISCRIMINATOR_LEN> =
        Discriminator::new([1, 0, 0, 0, 0, 0, 0, 0]);
}

impl BorshDeserialize for MockRewardsIntegrationInstructionData {
    fn deserialize_reader<R: io::Read>(reader: &mut R) -> std::io::Result<Self> {
        match Discriminator::deserialize_reader(reader)? {
            Self::INITIALIZE_INTEGRATION_DISTRIBUTION => {
                let dz_epoch = BorshDeserialize::deserialize_reader(reader)?;
                Ok(Self::InitializeIntegrationDistribution { dz_epoch })
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid discriminator",
            )),
        }
    }
}

impl BorshSerialize for MockRewardsIntegrationInstructionData {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::InitializeIntegrationDistribution { dz_epoch } => {
                Self::INITIALIZE_INTEGRATION_DISTRIBUTION.serialize(writer)?;
                dz_epoch.serialize(writer)
            }
        }
    }
}

/// Build the instruction for creating the mock's integration distribution
/// PDA. The bucket (an SPL token account whose authority is the PDA) is
/// expected to be created off-chain by the test harness.
pub fn initialize_integration_distribution(
    payer_key: &Pubkey,
    dz_epoch: DoubleZeroEpoch,
) -> Instruction {
    let (integration_distribution_key, _) = MockIntegrationDistribution::find_address(dz_epoch);

    try_build_instruction(
        &ID,
        vec![
            AccountMeta::new(*payer_key, true),
            AccountMeta::new(integration_distribution_key, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        &MockRewardsIntegrationInstructionData::InitializeIntegrationDistribution { dz_epoch },
    )
    .unwrap()
}
