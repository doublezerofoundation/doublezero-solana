use borsh::BorshDeserialize;
use doublezero_program_tools::{
    account_info::{try_next_enumerated_account, TryNextAccounts},
    recipe::{create_account::try_create_account, Invoker},
    zero_copy::{self, ZeroCopyAccount},
};
use doublezero_revenue_distribution::{
    integration::{
        IntegrationInstructionData, WithdrawIntegrationRewardsHandlerAccounts,
        INTEGRATION_DISTRIBUTION_SEED_PREFIX,
    },
    types::DoubleZeroEpoch,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed_unchecked;
use solana_msg::msg;
use solana_program_error::{ProgramError, ProgramResult};
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use spl_token_interface::instruction as token_instruction;

use crate::{
    instruction::MockRewardsIntegrationInstructionData, state::MockIntegrationDistribution, ID,
};

solana_program_entrypoint::entrypoint!(try_process_instruction);

fn try_process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    if program_id != &ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Integrations first attempt to deserialize the shared rev-distr
    // interface, then fall back to their own instruction enum. The shared
    // `WithdrawIntegrationRewards` variant serializes as a single `[0]` byte.
    if matches!(data.first(), Some(0)) {
        let ix = IntegrationInstructionData::try_from_slice(data)
            .map_err(|_| ProgramError::InvalidInstructionData)?;
        match ix {
            IntegrationInstructionData::WithdrawIntegrationRewards => {
                return try_withdraw_integration_rewards(accounts);
            }
        }
    }

    let ix =
        BorshDeserialize::try_from_slice(data).map_err(|_| ProgramError::InvalidInstructionData)?;
    match ix {
        MockRewardsIntegrationInstructionData::InitializeIntegrationDistribution { dz_epoch } => {
            try_initialize_integration_distribution(accounts, dz_epoch)
        }
    }
}

fn try_initialize_integration_distribution(
    accounts: &[AccountInfo],
    dz_epoch: DoubleZeroEpoch,
) -> ProgramResult {
    msg!("Initialize integration distribution");

    // We expect the following accounts:
    // - 0: Payer (funder for the new PDA).
    // - 1: New integration distribution PDA.
    // - 2: System program.
    let mut accounts_iter = accounts.iter().enumerate();

    let (_, payer_info) = try_next_enumerated_account(&mut accounts_iter, Default::default())?;
    let (_, new_integration_distribution_info) =
        try_next_enumerated_account(&mut accounts_iter, Default::default())?;

    let (expected_key, bump_seed) = MockIntegrationDistribution::find_address(dz_epoch);
    if new_integration_distribution_info.key != &expected_key {
        msg!("Invalid seeds for integration distribution");
        return Err(ProgramError::InvalidSeeds);
    }

    try_create_account(
        Invoker::Signer(payer_info.key),
        Invoker::Pda {
            key: &expected_key,
            signer_seeds: &[
                INTEGRATION_DISTRIBUTION_SEED_PREFIX,
                &dz_epoch.as_seed(),
                &[bump_seed],
            ],
        },
        new_integration_distribution_info.lamports(),
        zero_copy::data_end::<MockIntegrationDistribution>(),
        &ID,
        accounts,
        Default::default(),
    )?;

    let (mut integration_distribution, _) = zero_copy::try_initialize::<MockIntegrationDistribution>(
        new_integration_distribution_info,
    )?;
    integration_distribution.dz_epoch = dz_epoch;
    integration_distribution.bump_seed = bump_seed;

    Ok(())
}

fn try_withdraw_integration_rewards(accounts: &[AccountInfo]) -> ProgramResult {
    msg!("Withdraw integration rewards");

    // The shared interface helper validates slot ordering, writable flags,
    // and that the parent `Distribution` is a rev-distr-owned signer.
    // Integration-specific state (the mock's `MockIntegrationDistribution`)
    // is deserialized from the returned `&AccountInfo` below.
    let mut accounts_iter = accounts.iter().enumerate();
    let interface =
        WithdrawIntegrationRewardsHandlerAccounts::try_next_accounts(&mut accounts_iter, ())?;

    let (integration_distribution_index, integration_distribution_info) =
        interface.integration_distribution_info;
    let (_, integration_2z_bucket_info) = interface.integration_2z_bucket_info;
    let (_, destination_token_account_info) = interface.destination_token_account_info;
    let parent_distribution = interface.parent_distribution;

    let integration_distribution =
        ZeroCopyAccount::<MockIntegrationDistribution>::try_from_account_info(
            integration_distribution_index,
            integration_distribution_info,
            Some(&ID),
        )?;

    if integration_distribution.dz_epoch != parent_distribution.dz_epoch {
        msg!(
            "DZ epoch mismatch: integration={}, parent={}",
            integration_distribution.dz_epoch,
            parent_distribution.dz_epoch,
        );
        return Err(ProgramError::InvalidAccountData);
    }

    // Transfer the entire bucket balance to the destination, signed by the
    // integration distribution PDA (the bucket's authority).
    let bucket_amount = spl_token_interface::state::Account::unpack(
        &integration_2z_bucket_info.try_borrow_data()?[..],
    )
    .map_err(|_| ProgramError::InvalidAccountData)?
    .amount;

    let dz_epoch = integration_distribution.dz_epoch;
    let bump_seed = integration_distribution.bump_seed;

    let token_transfer_ix = token_instruction::transfer(
        &spl_token_interface::ID,
        integration_2z_bucket_info.key,
        destination_token_account_info.key,
        integration_distribution.info.key,
        &[],
        bucket_amount,
    )
    .unwrap();

    invoke_signed_unchecked(
        &token_transfer_ix,
        accounts,
        &[&[
            INTEGRATION_DISTRIBUTION_SEED_PREFIX,
            &dz_epoch.as_seed(),
            &[bump_seed],
        ]],
    )?;

    msg!(
        "Integration transferred {} 2Z for DZ epoch {}",
        bucket_amount,
        dz_epoch,
    );

    Ok(())
}
