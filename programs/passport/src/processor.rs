use borsh::BorshDeserialize;
use doublezero_program_tools::{
    account_info::{
        try_next_enumerated_account, EnumeratedAccountInfoIter, NextAccountOptions,
        TryNextAccounts, UpgradeAuthority,
    },
    recipe::{create_account::try_create_account, Invoker},
    zero_copy::{self, ZeroCopyAccount, ZeroCopyMutAccount},
    LAMPORTS_PER_SOL,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed_unchecked;
use solana_msg::msg;
use solana_program::program::invoke;
use solana_program_error::{ProgramError, ProgramResult};
use solana_pubkey::Pubkey;
use solana_system_interface::instruction as system_instruction;
use solana_sysvar::{rent::Rent, Sysvar};

use crate::{
    instruction::{
        AccessMode, PassportInstructionData, ProgramConfiguration, ProgramFlagConfiguration,
    },
    state::{AccessRequest, ProgramConfig},
    ID,
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

    // NOTE: Instruction data that happens to deserialize to any of the enum variants and has
    // trailing data constitutes invalid instruction data.
    let ix_data =
        BorshDeserialize::try_from_slice(data).map_err(|_| ProgramError::InvalidInstructionData)?;

    match ix_data {
        PassportInstructionData::InitializeProgram => try_initialize_program(accounts),
        PassportInstructionData::SetAdmin(admin_key) => try_set_admin(accounts, admin_key),
        PassportInstructionData::ConfigureProgram(setting) => {
            try_configure_program(accounts, setting)
        }
        PassportInstructionData::RequestAccess(access_mode) => {
            try_request_access(accounts, access_mode)
        }
        PassportInstructionData::GrantAccess => try_grant_access(accounts),
        PassportInstructionData::DenyAccess => try_deny_access(accounts),
    }
}

fn try_initialize_program(accounts: &[AccountInfo]) -> ProgramResult {
    msg!("Initialize program");

    // We expect the following accounts for this instruction:
    // - 0: Payer (funder for new accounts).
    // - 1: New program config.
    // - 5: System program.
    let mut accounts_iter = accounts.iter().enumerate();

    // Account 0 must be a signer and writable (i.e., payer) because it will be sending lamports
    // to the new config account when the system program allocates data to it. But because the
    // create-program instruction requires that this account is a signer and is writable, we do
    // not need to explicitly check these fields in its account info.
    let (_, payer_info) = try_next_enumerated_account(&mut accounts_iter, Default::default())?;

    // Account 1 must be the new program config account. This account should not exist yet.
    let (account_index, new_program_config_info) =
        try_next_enumerated_account(&mut accounts_iter, Default::default())?;

    let (expected_program_config_key, program_config_bump) = ProgramConfig::find_address();

    // Enforce this account location.
    if new_program_config_info.key != &expected_program_config_key {
        msg!(
            "Invalid seeds for program config (account {})",
            account_index
        );
        return Err(ProgramError::InvalidSeeds);
    }

    let rent_sysvar = Rent::get().unwrap();

    try_create_account(
        Invoker::Signer(payer_info.key),
        Invoker::Pda {
            key: &expected_program_config_key,
            signer_seeds: &[ProgramConfig::SEED_PREFIX, &[program_config_bump]],
        },
        new_program_config_info.lamports(),
        zero_copy::data_end::<ProgramConfig>(),
        &ID,
        accounts,
        Some(&rent_sysvar),
    )?;

    // Establish the discriminator. Set other fields using the configure program instruction.
    zero_copy::try_initialize::<ProgramConfig>(new_program_config_info, None)?;

    Ok(())
}

fn try_set_admin(accounts: &[AccountInfo], admin_key: Pubkey) -> ProgramResult {
    msg!("Set admin");

    // We expect the following accounts for this instruction:
    // - 0: This program's program data account (BPF Loader Upgradeable program).
    // - 1: The program's owner (i.e., upgrade authority).
    // - 2: Program config.
    let mut accounts_iter = accounts.iter().enumerate();

    // Account 0 must be the program data belonging to this program.
    // Account 1 must be the owner of the program data (i.e., the upgrade authority).
    UpgradeAuthority::try_next_accounts(&mut accounts_iter, &ID)?;

    // Account 2 must be the program config account.
    let mut program_config =
        ZeroCopyMutAccount::<ProgramConfig>::try_next_accounts(&mut accounts_iter, Some(&ID))?;

    msg!("admin_key: {}", admin_key);
    program_config.admin_key = admin_key;

    Ok(())
}

fn try_configure_program(accounts: &[AccountInfo], setting: ProgramConfiguration) -> ProgramResult {
    msg!("Configure program");

    // We expect the following accounts for this instruction:
    // - 0: Program config.
    // - 1: Admin.
    let mut accounts_iter = accounts.iter().enumerate();

    let authorized_use =
        VerifiedProgramAuthorityMut::try_next_accounts(&mut accounts_iter, Authority::Admin)?;
    let mut program_config = authorized_use.program_config;

    match setting {
        ProgramConfiguration::Flag(configure_flag) => {
            msg!("Set flag");
            match configure_flag {
                ProgramFlagConfiguration::IsPaused(should_pause) => {
                    msg!("is_paused: {}", should_pause);
                    program_config.set_is_paused(should_pause);
                }
            };
        }
        ProgramConfiguration::Sentinel(sentinel_key) => {
            msg!("Set sentinel_key: {}", sentinel_key);
            program_config.sentinel_key = sentinel_key;
        }
    }

    Ok(())
}

fn try_request_access(accounts: &[AccountInfo], access_mode: AccessMode) -> ProgramResult {
    msg!("Initiate access request");

    let AccessMode::SolanaValidator {
        validator_id,
        service_key,
        ed25519_signature,
    } = access_mode;

    if service_key == Pubkey::default() {
        msg!("User service key cannot be zero address");
        return Err(ProgramError::InvalidInstructionData);
    }

    // Instruction accounts are expected in the following order:
    // - 0: Program config
    // - 1: Validator ID
    // - 2: Payer (funder and rent beneficiary)
    // - 3: New access request account
    // - 4: System program

    let mut accounts_iter = accounts.iter().enumerate();

    // Account 0 must be the program config.
    let program_config = ZeroCopyAccount::<ProgramConfig>::try_next_accounts(&mut accounts_iter, Some(&ID))?;

    // Make sure program is not paused and we're accepting new accounts at this time
    try_require_unpaused(&program_config)?;

    // Account 1 should be the validator for which access is being requested.
    // Make sure the validator in the access request matches the validator ID account
    let (_, validator_info) = try_next_enumerated_account(&mut accounts_iter, Default::default())?;
    if validator_info.key != &validator_id {
        msg!("Validator access request must match the validator account");
        return Err(ProgramError::InvalidInstructionData);
    }

    // Account 2 must be the payer. The system program will automatically ensure this account is a signer and writable
    // in order to transfer the lamports to create the new account.
    let (_, payer_info) = try_next_enumerated_account(&mut accounts_iter, Default::default())?;
    let (account_index, new_access_request_info) = try_next_enumerated_account(&mut accounts_iter, Default::default())?;

    let (expected_access_request_key, access_request_bump) = AccessRequest::find_address(&service_key);

    // Enforce the account location and seed validity
    if new_access_request_info.key != &expected_access_request_key {
        msg!("Invalid seeds for access request (account {})", account_index);
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate the signature of the requesting validator
    let message: [u8; 48] = {
        let mut buf = [0u8; 48];
        buf[..16].copy_from_slice(b"solana_validator");
        buf[16..].copy_from_slice(service_key.as_ref());
        buf
    };
    let sig_verify_ix = solana_ed25519_program::new_ed25519_instruction_with_signature(
        &message,
        &ed25519_signature,
        validator_id.as_ref().try_into().unwrap(),
    );
    invoke(&sig_verify_ix, &[])?;

    // manually create the account instruction to override the usual minimum rent exemption
    // balance transfer and instead put down the refundable 1 SOL deposit
    let create_account_ix = system_instruction::create_account(
        payer_info.key,
        &expected_access_request_key,
        LAMPORTS_PER_SOL,
        zero_copy::data_end::<AccessRequest>() as u64,
        &ID,
    );
    invoke_signed_unchecked(&create_account_ix, accounts, &[&[AccessRequest::SEED_PREFIX, service_key.as_ref(), &[access_request_bump]]])?;

    // Finalize init the access request with the user service and beneficiary keys
    let (mut access_request, _) = zero_copy::try_initialize::<AccessRequest>(new_access_request_info, None)?;
    access_request.service_key = service_key;
    access_request.rent_beneficiary_key = *payer_info.key;

    msg!("Initialized user access request {}", service_key);

    Ok(())
}

fn try_grant_access(accounts: &[AccountInfo]) -> ProgramResult {
    let mut accounts_iter = accounts.iter().enumerate();

    VerifiedProgramAuthority::try_next_accounts(&mut accounts_iter, Authority::Sentinel)?;

    // Send the sentinel 10_000 lamports from access request rent for executing this transaction.
    // Send remaining rent lamports to rent beneficiary.
    //
    // Only the sentinel can call this instruction.
    todo!();
}

fn try_deny_access(accounts: &[AccountInfo]) -> ProgramResult {
    let mut accounts_iter = accounts.iter().enumerate();

    VerifiedProgramAuthority::try_next_accounts(&mut accounts_iter, Authority::Sentinel)?;

    // Send the sentinel full rent when closing the access request account.
    //
    // Only the sentinel can call this instruction.
    todo!();
}

//
// Account info handling.
//

enum Authority {
    Admin,
    Sentinel,
}

impl Authority {
    fn try_next_as_authorized_account<'b, 'c>(
        &self,
        accounts_iter: &mut EnumeratedAccountInfoIter<'b, 'c>,
        program_config: &ProgramConfig,
    ) -> Result<(usize, &'b AccountInfo<'c>), ProgramError> {
        let (index, authority_info) = try_next_enumerated_account(
            accounts_iter,
            NextAccountOptions {
                must_be_signer: true,
                ..Default::default()
            },
        )?;

        match self {
            Authority::Admin => {
                if authority_info.key != &program_config.admin_key {
                    msg!("Unauthorized admin (account {})", index);
                    return Err(ProgramError::InvalidAccountData);
                }
            }
            Authority::Sentinel => {
                if authority_info.key != &program_config.sentinel_key {
                    msg!("Unauthorized sentinel (account {})", index);
                    return Err(ProgramError::InvalidAccountData);
                }
            }
        }

        Ok((index, authority_info))
    }
}

struct VerifiedProgramAuthority<'a, 'b> {
    _program_config: ZeroCopyAccount<'a, 'b, ProgramConfig>,
    _authority: (usize, &'a AccountInfo<'b>),
}

impl<'a, 'b> TryNextAccounts<'a, 'b, Authority> for VerifiedProgramAuthority<'a, 'b> {
    fn try_next_accounts(
        accounts_iter: &mut EnumeratedAccountInfoIter<'a, 'b>,
        authority: Authority,
    ) -> Result<Self, ProgramError> {
        // Index == 0.
        let program_config = ZeroCopyAccount::try_next_accounts(accounts_iter, Some(&ID))?;

        // Index == 1.
        let (index, authority_info) =
            authority.try_next_as_authorized_account(accounts_iter, &program_config.data)?;

        Ok(Self {
            _program_config: program_config,
            _authority: (index, authority_info),
        })
    }
}

struct VerifiedProgramAuthorityMut<'a, 'b> {
    program_config: ZeroCopyMutAccount<'a, 'b, ProgramConfig>,
    _authority: (usize, &'a AccountInfo<'b>),
}

impl<'a, 'b> TryNextAccounts<'a, 'b, Authority> for VerifiedProgramAuthorityMut<'a, 'b> {
    fn try_next_accounts(
        accounts_iter: &mut EnumeratedAccountInfoIter<'a, 'b>,
        authority: Authority,
    ) -> Result<Self, ProgramError> {
        // Index == 0.
        let program_config = ZeroCopyMutAccount::try_next_accounts(accounts_iter, Some(&ID))?;

        // Index == 1.
        let (index, authority_info) =
            authority.try_next_as_authorized_account(accounts_iter, &program_config.data)?;

        Ok(Self {
            program_config,
            _authority: (index, authority_info),
        })
    }
}

fn try_require_unpaused(program_config: &ProgramConfig) -> ProgramResult {
    if program_config.is_paused() {
        msg!("Program is paused");
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}
